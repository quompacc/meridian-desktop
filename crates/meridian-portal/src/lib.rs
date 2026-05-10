use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use meridian_ipc::{
    ScreenshotBridgeError, ScreenshotBridgeMessage, ScreenshotBridgeRequest,
    ScreenshotBridgeResponse, ScreenshotBridgeResult, ScreenshotKind, ScreenshotRegion,
    ScreenshotRequestMetadata, ScreenshotRequestOrigin,
};
use tracing::{debug, info};
use zbus::blocking::Connection;

pub const DBUS_SERVICE_NAME: &str = "org.meridian.Portal1";
pub const DBUS_OBJECT_PATH: &str = "/org/meridian/portal";
pub const DBUS_INTERFACE_SCREENSHOT: &str = "org.meridian.portal.Screenshot1";
pub const PLANNED_XDG_BACKEND_NAME: &str = "org.freedesktop.impl.portal.desktop.meridian";

const COMPOSITOR_UNAVAILABLE_MSG: &str = "compositor screenshot bridge unavailable";
const COMPOSITOR_TIMEOUT_MSG: &str = "compositor screenshot bridge timeout";
const COMPOSITOR_PROTOCOL_MSG: &str = "compositor screenshot bridge protocol error";
const BRIDGE_TIMEOUT: Duration = Duration::from_secs(2);
static REQUEST_MARKER: AtomicU64 = AtomicU64::new(1);

pub type PortalError = ScreenshotBridgeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalScaffoldState {
    pub ready: bool,
}

impl PortalScaffoldState {
    pub fn new() -> Self {
        Self { ready: true }
    }
}

impl Default for PortalScaffoldState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub enum PortalBootstrapError {
    SessionBusConnect(zbus::Error),
    RequestName(zbus::Error),
    RegisterInterface(zbus::Error),
}

impl std::fmt::Display for PortalBootstrapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SessionBusConnect(err) => {
                write!(f, "failed to connect to session bus: {}", err)
            }
            Self::RequestName(err) => write!(f, "failed to request D-Bus name: {}", err),
            Self::RegisterInterface(err) => {
                write!(f, "failed to register D-Bus interface: {}", err)
            }
        }
    }
}

impl std::error::Error for PortalBootstrapError {}

pub struct PortalDbusService {
    _connection: Connection,
    pub state: PortalScaffoldState,
}

#[derive(Default)]
struct ScreenshotPortalIface;

#[zbus::interface(name = "org.meridian.portal.Screenshot1")]
impl ScreenshotPortalIface {
    fn screenshot(
        &self,
        request_id: String,
        output: String,
        include_cursor: bool,
    ) -> zbus::fdo::Result<String> {
        let output = (!output.trim().is_empty()).then_some(output);
        debug!(
            "screenshot request received: request_id={} output={:?} include_cursor={}",
            request_id, output, include_cursor
        );

        let request = build_full_output_request(request_id, output, include_cursor);
        match handle_screenshot_request(request) {
            Ok(response) => Ok(response.file_descriptor_token.unwrap_or_default()),
            Err(err) => {
                match &err {
                    PortalError::CompositorUnavailable(message) => {
                        info!("compositor unavailable: {}", message);
                    }
                    PortalError::PermissionDenied(message) | PortalError::Unsupported(message) => {
                        info!("screenshot bridge denied/unsupported: {}", message);
                    }
                    _ => {}
                }
                Err(map_portal_error_to_dbus(err))
            }
        }
    }
}

pub fn start_dbus_service() -> Result<PortalDbusService, PortalBootstrapError> {
    let connection = Connection::session().map_err(PortalBootstrapError::SessionBusConnect)?;
    connection
        .request_name(DBUS_SERVICE_NAME)
        .map_err(PortalBootstrapError::RequestName)?;
    connection
        .object_server()
        .at(DBUS_OBJECT_PATH, ScreenshotPortalIface)
        .map_err(PortalBootstrapError::RegisterInterface)?;

    info!(
        "portal D-Bus skeleton ready: service={} path={} interface={} (planned-xdg-backend-name={})",
        DBUS_SERVICE_NAME, DBUS_OBJECT_PATH, DBUS_INTERFACE_SCREENSHOT, PLANNED_XDG_BACKEND_NAME
    );

    Ok(PortalDbusService {
        _connection: connection,
        state: PortalScaffoldState::new(),
    })
}

impl PortalDbusService {
    pub fn run(&self) -> ! {
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}

pub fn build_full_output_request(
    request_id: impl Into<String>,
    output: Option<String>,
    include_cursor: bool,
) -> ScreenshotBridgeRequest {
    ScreenshotBridgeRequest {
        request_id: request_id.into(),
        kind: ScreenshotKind::FullOutput,
        output,
        include_cursor,
        region: None,
        metadata: ScreenshotRequestMetadata {
            requester: None,
            origin: ScreenshotRequestOrigin::PortalDbus,
            request_marker: Some(next_request_marker()),
            identity_trusted: false,
        },
    }
}

pub fn build_full_output_request_with_region(
    request_id: impl Into<String>,
    output: Option<String>,
    include_cursor: bool,
    region: Option<ScreenshotRegion>,
) -> ScreenshotBridgeRequest {
    ScreenshotBridgeRequest {
        request_id: request_id.into(),
        kind: ScreenshotKind::FullOutput,
        output,
        include_cursor,
        region,
        metadata: ScreenshotRequestMetadata {
            requester: None,
            origin: ScreenshotRequestOrigin::PortalDbus,
            request_marker: Some(next_request_marker()),
            identity_trusted: false,
        },
    }
}

fn next_request_marker() -> u64 {
    REQUEST_MARKER.fetch_add(1, Ordering::Relaxed)
}

pub fn handle_screenshot_request(
    request: ScreenshotBridgeRequest,
) -> Result<ScreenshotBridgeResponse, PortalError> {
    request.validate()?;
    request_screenshot_via_bridge(request)
}

pub fn map_portal_error_to_dbus(err: PortalError) -> zbus::fdo::Error {
    match err {
        PortalError::Unsupported(msg) => zbus::fdo::Error::NotSupported(msg),
        PortalError::PermissionDenied(msg) => zbus::fdo::Error::AccessDenied(msg),
        PortalError::CompositorUnavailable(msg) | PortalError::Internal(msg) => {
            zbus::fdo::Error::Failed(msg)
        }
        PortalError::InvalidRequest(msg) => zbus::fdo::Error::InvalidArgs(msg),
    }
}

fn request_screenshot_via_bridge(
    request: ScreenshotBridgeRequest,
) -> Result<ScreenshotBridgeResponse, PortalError> {
    let mut stream = connect_bridge_socket()?;
    stream
        .set_write_timeout(Some(BRIDGE_TIMEOUT))
        .map_err(|_| PortalError::CompositorUnavailable(COMPOSITOR_UNAVAILABLE_MSG.to_string()))?;
    stream
        .set_read_timeout(Some(BRIDGE_TIMEOUT))
        .map_err(|_| PortalError::CompositorUnavailable(COMPOSITOR_UNAVAILABLE_MSG.to_string()))?;

    let bytes = meridian_ipc::encode_screenshot_bridge_message(
        &ScreenshotBridgeMessage::ScreenshotRequest {
            request: request.clone(),
        },
    )
    .map_err(|_| PortalError::Internal(COMPOSITOR_PROTOCOL_MSG.to_string()))?;

    stream
        .write_all(&bytes)
        .map_err(|_| PortalError::CompositorUnavailable(COMPOSITOR_UNAVAILABLE_MSG.to_string()))?;
    info!(
        "portal screenshot bridge request sent: request_id={}",
        request.request_id
    );

    wait_for_bridge_response(&mut stream, &request.request_id)
}

fn connect_bridge_socket() -> Result<UnixStream, PortalError> {
    UnixStream::connect(meridian_ipc::socket_path())
        .map_err(|_| PortalError::CompositorUnavailable(COMPOSITOR_UNAVAILABLE_MSG.to_string()))
}

fn wait_for_bridge_response(
    stream: &mut UnixStream,
    request_id: &str,
) -> Result<ScreenshotBridgeResponse, PortalError> {
    let mut buffer = Vec::new();
    let mut tmp = [0_u8; 4096];
    let deadline = Instant::now() + BRIDGE_TIMEOUT;

    loop {
        if Instant::now() >= deadline {
            return Err(PortalError::CompositorUnavailable(
                COMPOSITOR_TIMEOUT_MSG.to_string(),
            ));
        }

        match stream.read(&mut tmp) {
            Ok(0) => {
                return Err(PortalError::CompositorUnavailable(
                    COMPOSITOR_UNAVAILABLE_MSG.to_string(),
                ))
            }
            Ok(n) => buffer.extend_from_slice(&tmp[..n]),
            Err(err)
                if err.kind() == io::ErrorKind::WouldBlock
                    || err.kind() == io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(_) => {
                return Err(PortalError::CompositorUnavailable(
                    COMPOSITOR_UNAVAILABLE_MSG.to_string(),
                ))
            }
        }

        while let Some(pos) = buffer.iter().position(|byte| *byte == b'\n') {
            let line = buffer.drain(..=pos).collect::<Vec<_>>();
            let line = String::from_utf8_lossy(&line);
            if line.trim().is_empty() {
                continue;
            }

            let Ok(message) = meridian_ipc::decode_screenshot_bridge_message(line.trim()) else {
                continue;
            };

            if let ScreenshotBridgeMessage::ScreenshotResponse {
                request_id: result_request_id,
                result,
            } = message
            {
                if result_request_id == request_id {
                    return map_bridge_result_to_portal_result(request_id, result);
                }
            }
        }
    }
}

fn map_bridge_result_to_portal_result(
    request_id: &str,
    result: ScreenshotBridgeResult,
) -> Result<ScreenshotBridgeResponse, PortalError> {
    match result {
        ScreenshotBridgeResult::Success { response } => {
            if response.request_id != request_id {
                return Err(PortalError::Internal(COMPOSITOR_PROTOCOL_MSG.to_string()));
            }
            Ok(response)
        }
        ScreenshotBridgeResult::Error { error } => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use meridian_ipc::{
        ScreenshotBridgeError, ScreenshotBridgeResponse, ScreenshotBridgeResult, ScreenshotKind,
        ScreenshotRegion, ScreenshotRequestOrigin,
    };

    use super::{
        build_full_output_request, build_full_output_request_with_region,
        map_bridge_result_to_portal_result, map_portal_error_to_dbus, PortalError,
        PortalScaffoldState, DBUS_INTERFACE_SCREENSHOT, DBUS_OBJECT_PATH, DBUS_SERVICE_NAME,
    };

    #[test]
    fn scaffold_state_initializes_ready() {
        let state = PortalScaffoldState::new();
        assert!(state.ready);
    }

    #[test]
    fn dbus_constants_are_stable() {
        assert_eq!(DBUS_SERVICE_NAME, "org.meridian.Portal1");
        assert_eq!(DBUS_OBJECT_PATH, "/org/meridian/portal");
        assert_eq!(DBUS_INTERFACE_SCREENSHOT, "org.meridian.portal.Screenshot1");
    }

    #[test]
    fn full_output_request_maps_contract_fields() {
        let request = build_full_output_request("req-42", Some("HDMI-A-1".to_string()), false);
        assert_eq!(request.request_id, "req-42");
        assert_eq!(request.kind, ScreenshotKind::FullOutput);
        assert_eq!(request.output.as_deref(), Some("HDMI-A-1"));
        assert!(!request.include_cursor);
        assert!(request.region.is_none());
        assert_eq!(request.metadata.origin, ScreenshotRequestOrigin::PortalDbus);
        assert!(!request.metadata.identity_trusted);
        assert!(request.metadata.request_marker.is_some());
    }

    #[test]
    fn region_request_is_rejected_until_supported() {
        let request = build_full_output_request_with_region(
            "req-43",
            None,
            false,
            Some(ScreenshotRegion {
                x: 10,
                y: 20,
                width: 300,
                height: 200,
            }),
        );

        assert_eq!(
            request.validate(),
            Err(ScreenshotBridgeError::Unsupported(
                "region capture is not implemented yet".to_string()
            ))
        );
    }

    #[test]
    fn empty_request_id_is_invalid() {
        let request = build_full_output_request("   ", None, false);
        assert_eq!(
            request.validate(),
            Err(ScreenshotBridgeError::InvalidRequest(
                "request_id must not be empty".to_string()
            ))
        );
    }

    #[test]
    fn portal_error_maps_to_not_supported() {
        let mapped =
            map_portal_error_to_dbus(PortalError::Unsupported("not implemented yet".to_string()));
        match mapped {
            zbus::fdo::Error::NotSupported(message) => assert_eq!(message, "not implemented yet"),
            other => panic!("unexpected error mapping: {other:?}"),
        }
    }

    #[test]
    fn portal_error_maps_to_access_denied() {
        let mapped = map_portal_error_to_dbus(PortalError::PermissionDenied("denied".to_string()));
        match mapped {
            zbus::fdo::Error::AccessDenied(message) => assert_eq!(message, "denied"),
            other => panic!("unexpected error mapping: {other:?}"),
        }
    }

    #[test]
    fn bridge_error_result_maps_to_portal_error() {
        let result = map_bridge_result_to_portal_result(
            "req-7",
            ScreenshotBridgeResult::Error {
                error: ScreenshotBridgeError::PermissionDenied("denied by policy".to_string()),
            },
        );
        assert_eq!(
            result,
            Err(PortalError::PermissionDenied(
                "denied by policy".to_string()
            ))
        );
    }

    #[test]
    fn bridge_success_with_mismatched_id_is_protocol_error() {
        let result = map_bridge_result_to_portal_result(
            "req-8",
            ScreenshotBridgeResult::Success {
                response: ScreenshotBridgeResponse {
                    request_id: "other-id".to_string(),
                    file_descriptor_token: None,
                },
            },
        );
        assert_eq!(
            result,
            Err(PortalError::Internal(
                "compositor screenshot bridge protocol error".to_string()
            ))
        );
    }
}
