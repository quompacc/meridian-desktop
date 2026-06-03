use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::task;
use tracing::{debug, info, warn};
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

use meridian_ipc::{
    decode_screenshot_bridge_message, encode_screenshot_bridge_message, socket_path,
    ScreenshotBridgeError, ScreenshotBridgeMessage, ScreenshotBridgeRequest,
    ScreenshotBridgeResponse, ScreenshotBridgeResult, ScreenshotKind, ScreenshotRequestMetadata,
    ScreenshotRequestOrigin,
};

type Asv = HashMap<String, OwnedValue>;

// Idle-timeout for a single read from the compositor socket. The compositor
// also broadcasts shell events on the same connection, so any liveness keeps
// the timer fresh; only a fully silent (dead/stuck) compositor will trip it.
const READ_IDLE_TIMEOUT: Duration = Duration::from_secs(120);

pub struct ScreenshotImpl;

#[zbus::interface(name = "org.freedesktop.impl.portal.Screenshot")]
impl ScreenshotImpl {
    #[zbus(property)]
    fn version(&self) -> u32 {
        2
    }

    async fn screenshot(
        &self,
        _handle: ObjectPath<'_>,
        app_id: &str,
        _parent_window: &str,
        _options: Asv,
    ) -> (u32, Asv) {
        let app_id = app_id.to_string();
        let request_id = next_request_id();
        debug!("Screenshot request id={request_id} app_id={app_id:?}");

        let request = ScreenshotBridgeRequest {
            request_id: request_id.clone(),
            kind: ScreenshotKind::FullOutput,
            output: None,
            include_cursor: false,
            region: None,
            metadata: ScreenshotRequestMetadata {
                requester: if app_id.is_empty() {
                    None
                } else {
                    Some(app_id.clone())
                },
                origin: ScreenshotRequestOrigin::PortalDbus,
                request_marker: None,
                identity_trusted: false,
            },
        };

        let joined = task::spawn_blocking(move || run_request(request)).await;

        match joined {
            Ok(Ok(path)) => {
                let uri = path_to_file_uri(&path);
                info!("Screenshot: app_id={app_id:?} -> {uri}");
                (0, uri_asv(uri))
            }
            Ok(Err(ScreenshotPortalError::Denied)) => {
                info!("Screenshot: denied (app_id={app_id:?})");
                (1, Asv::new())
            }
            Ok(Err(err)) => {
                warn!("Screenshot: failed (app_id={app_id:?}): {err}");
                (2, Asv::new())
            }
            Err(err) => {
                warn!("Screenshot: portal worker panicked (app_id={app_id:?}): {err}");
                (2, Asv::new())
            }
        }
    }

    async fn pick_color(
        &self,
        _handle: ObjectPath<'_>,
        _app_id: &str,
        _parent_window: &str,
        _options: Asv,
    ) -> (u32, Asv) {
        // PickColor is not implemented in this slice; respond "other" so clients
        // see a clean failure rather than UnknownMethod.
        (2, Asv::new())
    }
}

#[derive(Debug)]
enum ScreenshotPortalError {
    Denied,
    NoSocket(String),
    Io(String),
    Bridge(ScreenshotBridgeError),
    Protocol(String),
}

impl std::fmt::Display for ScreenshotPortalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Denied => write!(f, "permission denied"),
            Self::NoSocket(msg) => write!(f, "compositor socket unavailable: {msg}"),
            Self::Io(msg) => write!(f, "io error: {msg}"),
            Self::Bridge(err) => write!(f, "bridge error: {err:?}"),
            Self::Protocol(msg) => write!(f, "protocol error: {msg}"),
        }
    }
}

fn run_request(request: ScreenshotBridgeRequest) -> Result<String, ScreenshotPortalError> {
    let req_id = request.request_id.clone();
    let path = socket_path();
    let stream = UnixStream::connect(&path)
        .map_err(|e| ScreenshotPortalError::NoSocket(format!("{}: {e}", path.display())))?;
    stream
        .set_read_timeout(Some(READ_IDLE_TIMEOUT))
        .map_err(|e| ScreenshotPortalError::Io(e.to_string()))?;

    let mut writer = stream
        .try_clone()
        .map_err(|e| ScreenshotPortalError::Io(e.to_string()))?;
    let bytes =
        encode_screenshot_bridge_message(&ScreenshotBridgeMessage::ScreenshotRequest { request })
            .map_err(|e| ScreenshotPortalError::Io(e.to_string()))?;
    writer
        .write_all(&bytes)
        .map_err(|e| ScreenshotPortalError::Io(e.to_string()))?;

    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => return Err(ScreenshotPortalError::Io(e.to_string())),
        };
        if line.trim().is_empty() {
            continue;
        }
        // The compositor sends both bridge responses *and* shell-event
        // broadcasts on this socket; only the former concern us.
        let msg = match decode_screenshot_bridge_message(&line) {
            Ok(msg) => msg,
            Err(_) => continue,
        };
        let ScreenshotBridgeMessage::ScreenshotResponse { request_id, result } = msg else {
            continue;
        };
        if request_id != req_id {
            continue;
        }
        return match result {
            ScreenshotBridgeResult::Success {
                response:
                    ScreenshotBridgeResponse {
                        file_descriptor_token: Some(path),
                        ..
                    },
            } => Ok(path),
            ScreenshotBridgeResult::Success { .. } => Err(ScreenshotPortalError::Protocol(
                "success response missing file path token".to_string(),
            )),
            ScreenshotBridgeResult::Error {
                error: ScreenshotBridgeError::PermissionDenied(_),
            } => Err(ScreenshotPortalError::Denied),
            ScreenshotBridgeResult::Error { error } => Err(ScreenshotPortalError::Bridge(error)),
        };
    }
    Err(ScreenshotPortalError::Protocol(
        "compositor closed socket without a response".to_string(),
    ))
}

fn uri_asv(uri: String) -> Asv {
    let mut m = Asv::new();
    if let Ok(owned) = Value::from(uri).try_to_owned() {
        m.insert("uri".into(), owned);
    }
    m
}

fn path_to_file_uri(path: &str) -> String {
    if path.starts_with("file://") {
        path.to_string()
    } else {
        format!("file://{}", percent_encode_path(path))
    }
}

fn percent_encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn next_request_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let pid = std::process::id();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    format!("portal-{pid}-{nanos:x}-{n:x}")
}

#[cfg(test)]
mod tests {
    use super::{next_request_id, path_to_file_uri};

    #[test]
    fn file_uri_preserves_existing_scheme() {
        assert_eq!(
            path_to_file_uri("file:///run/user/1000/already.png"),
            "file:///run/user/1000/already.png"
        );
    }

    #[test]
    fn file_uri_percent_encodes_path_bytes() {
        assert_eq!(
            path_to_file_uri("/run/user/1000/Meridian Test/ä.png"),
            "file:///run/user/1000/Meridian%20Test/%C3%A4.png"
        );
    }

    #[test]
    fn request_ids_are_unique_per_call() {
        let a = next_request_id();
        let b = next_request_id();
        assert_ne!(a, b);
        assert!(a.starts_with("portal-"));
        assert!(b.starts_with("portal-"));
    }
}
