//! Typed, capability-scoped document-to-host bridge.
//!
//! The contract exposes only launcher lifecycle and validated app activation.
//! Unknown versions, fields and capabilities fail closed.

use serde::Deserialize;

pub(crate) const HANDLER_NAME: &str = "meridian";
const SCHEMA_VERSION: u32 = 1;
const MAX_MESSAGE_BYTES: usize = 4096;
const MAX_REQUEST_ID_BYTES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    CloseLauncher,
    ToggleLauncher,
    LaunchApp { desktop_id: String },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u32,
    capability: String,
    request_id: String,
    #[serde(default)]
    desktop_id: Option<String>,
}

pub(crate) fn decode(raw: &str) -> Result<Command, String> {
    if raw.len() > MAX_MESSAGE_BYTES {
        return Err("message exceeds bridge limit".to_string());
    }
    let request: Request = serde_json::from_str(raw).map_err(|_| "invalid request schema")?;
    if request.version != SCHEMA_VERSION {
        return Err("unsupported bridge version".to_string());
    }
    if request.request_id.is_empty()
        || request.request_id.len() > MAX_REQUEST_ID_BYTES
        || !request
            .request_id
            .bytes()
            .all(|byte| byte.is_ascii_graphic())
    {
        return Err("invalid request id".to_string());
    }
    match request.capability.as_str() {
        "launcher.close" if request.desktop_id.is_none() => Ok(Command::CloseLauncher),
        "panel.toggle-launcher" if request.desktop_id.is_none() => Ok(Command::ToggleLauncher),
        "launcher.launch" => {
            let desktop_id = request.desktop_id.ok_or("missing desktop id")?;
            if desktop_id.is_empty()
                || desktop_id.len() > 512
                || desktop_id.chars().any(char::is_control)
            {
                return Err("invalid desktop id".to_string());
            }
            Ok(Command::LaunchApp { desktop_id })
        }
        _ => Err("capability denied".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_allows_only_launcher_lifecycle_and_catalog_activation() {
        assert_eq!(
            decode(r#"{"version":1,"capability":"launcher.close","request_id":"request-1"}"#),
            Ok(Command::CloseLauncher)
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"panel.toggle-launcher","request_id":"request-panel"}"#
            ),
            Ok(Command::ToggleLauncher)
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"launcher.launch","request_id":"request-2","desktop_id":"firefox.desktop"}"#
            ),
            Ok(Command::LaunchApp {
                desktop_id: "firefox.desktop".to_string()
            })
        );
        assert!(
            decode(r#"{"version":1,"capability":"process.spawn","request_id":"request-2"}"#)
                .is_err()
        );
    }

    #[test]
    fn malformed_version_id_and_extra_fields_fail_closed() {
        for request in [
            r#"{"version":2,"capability":"launcher.close","request_id":"request-1"}"#,
            r#"{"version":1,"capability":"launcher.close","request_id":""}"#,
            r#"{"version":1,"capability":"launcher.close","request_id":"a b"}"#,
            r#"{"version":1,"capability":"launcher.close","request_id":"ok","extra":true}"#,
            r#"{"version":1,"capability":"launcher.launch","request_id":"ok"}"#,
            r#"{"version":1,"capability":"launcher.launch","request_id":"ok","desktop_id":""}"#,
        ] {
            assert!(decode(request).is_err(), "accepted {request}");
        }
    }

    #[test]
    fn oversized_message_is_rejected_before_parsing() {
        assert!(decode(&"x".repeat(MAX_MESSAGE_BYTES + 1)).is_err());
    }
}
