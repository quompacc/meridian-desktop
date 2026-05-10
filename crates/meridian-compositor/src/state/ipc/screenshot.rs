use meridian_ipc::{ScreenshotBridgeError, ScreenshotBridgeRequest, ScreenshotBridgeResult};

const SCREENSHOT_PERMISSION_DENIED_MSG: &str = "screenshot denied by compositor policy";

use super::screenshot_policy::{
    ScreenshotPolicy, ScreenshotPolicyContext, ScreenshotPolicyDecision,
};

pub(crate) fn handle_screenshot_bridge_request(
    request: ScreenshotBridgeRequest,
) -> ScreenshotBridgeResult {
    let decision = ScreenshotPolicy::evaluate(&request, ScreenshotPolicyContext { client_id: 0 });
    match decision {
        ScreenshotPolicyDecision::Deny => ScreenshotBridgeResult::Error {
            error: ScreenshotBridgeError::PermissionDenied(
                SCREENSHOT_PERMISSION_DENIED_MSG.to_string(),
            ),
        },
        ScreenshotPolicyDecision::Unsupported(message) => ScreenshotBridgeResult::Error {
            error: ScreenshotBridgeError::Unsupported(message),
        },
        ScreenshotPolicyDecision::Invalid(message) => ScreenshotBridgeResult::Error {
            error: ScreenshotBridgeError::InvalidRequest(message),
        },
    }
}

#[cfg(test)]
mod tests {
    use meridian_ipc::{
        ScreenshotBridgeError, ScreenshotBridgeRequest, ScreenshotBridgeResult, ScreenshotKind,
        ScreenshotRegion, ScreenshotRequestMetadata, ScreenshotRequestOrigin,
    };

    use super::handle_screenshot_bridge_request;

    #[test]
    fn invalid_request_is_rejected() {
        let request = ScreenshotBridgeRequest {
            request_id: " ".to_string(),
            kind: ScreenshotKind::FullOutput,
            output: None,
            include_cursor: true,
            region: None,
            metadata: ScreenshotRequestMetadata {
                requester: None,
                origin: ScreenshotRequestOrigin::PortalDbus,
                request_marker: Some(1),
                identity_trusted: false,
            },
        };

        assert_eq!(
            handle_screenshot_bridge_request(request),
            ScreenshotBridgeResult::Error {
                error: ScreenshotBridgeError::InvalidRequest(
                    "request_id must not be empty".to_string(),
                ),
            }
        );
    }

    #[test]
    fn deny_only_response_is_stable_for_valid_request() {
        let request = ScreenshotBridgeRequest {
            request_id: "req-bridge-1".to_string(),
            kind: ScreenshotKind::FullOutput,
            output: Some("eDP-1".to_string()),
            include_cursor: false,
            region: None,
            metadata: ScreenshotRequestMetadata {
                requester: None,
                origin: ScreenshotRequestOrigin::PortalDbus,
                request_marker: Some(2),
                identity_trusted: false,
            },
        };

        assert_eq!(
            handle_screenshot_bridge_request(request),
            ScreenshotBridgeResult::Error {
                error: ScreenshotBridgeError::PermissionDenied(
                    "screenshot denied by compositor policy".to_string(),
                ),
            }
        );
    }

    #[test]
    fn region_request_keeps_unsupported_semantics() {
        let request = ScreenshotBridgeRequest {
            request_id: "req-bridge-2".to_string(),
            kind: ScreenshotKind::FullOutput,
            output: None,
            include_cursor: false,
            region: Some(ScreenshotRegion {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            }),
            metadata: ScreenshotRequestMetadata {
                requester: None,
                origin: ScreenshotRequestOrigin::PortalDbus,
                request_marker: Some(3),
                identity_trusted: false,
            },
        };

        assert_eq!(
            handle_screenshot_bridge_request(request),
            ScreenshotBridgeResult::Error {
                error: ScreenshotBridgeError::Unsupported(
                    "region capture is not implemented yet".to_string(),
                ),
            }
        );
    }
}
