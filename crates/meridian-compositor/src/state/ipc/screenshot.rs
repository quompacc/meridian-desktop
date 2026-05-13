use meridian_ipc::{ScreenshotBridgeError, ScreenshotBridgeRequest, ScreenshotBridgeResult};

const SCREENSHOT_PERMISSION_DENIED_MSG: &str = "screenshot denied by compositor policy";

use super::screenshot_policy::{
    ScreenshotPolicy, ScreenshotPolicyContext, ScreenshotPolicyDecision,
};

pub(crate) fn handle_screenshot_bridge_request(
    request: ScreenshotBridgeRequest,
    client_id: u64,
) -> ScreenshotBridgeResult {
    let decision = ScreenshotPolicy::evaluate(&request, ScreenshotPolicyContext { client_id });
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
    use crate::state::ipc::screenshot_policy::{
        last_evaluated_client_id_for_test, screenshot_policy_test_lock,
    };

    #[test]
    fn invalid_request_is_rejected() {
        let _guard = screenshot_policy_test_lock().lock().expect("test lock");
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
            handle_screenshot_bridge_request(request, 7),
            ScreenshotBridgeResult::Error {
                error: ScreenshotBridgeError::InvalidRequest(
                    "request_id must not be empty".to_string(),
                ),
            }
        );
    }

    #[test]
    fn deny_only_response_is_stable_for_valid_request() {
        let _guard = screenshot_policy_test_lock().lock().expect("test lock");
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
            handle_screenshot_bridge_request(request, 7),
            ScreenshotBridgeResult::Error {
                error: ScreenshotBridgeError::PermissionDenied(
                    "screenshot denied by compositor policy".to_string(),
                ),
            }
        );
    }

    #[test]
    fn region_request_keeps_unsupported_semantics() {
        let _guard = screenshot_policy_test_lock().lock().expect("test lock");
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
            handle_screenshot_bridge_request(request, 7),
            ScreenshotBridgeResult::Error {
                error: ScreenshotBridgeError::Unsupported(
                    "region capture is not implemented yet".to_string(),
                ),
            }
        );
    }

    #[test]
    fn nonzero_client_id_is_forwarded_to_policy_context() {
        let _guard = screenshot_policy_test_lock().lock().expect("test lock");
        let request = ScreenshotBridgeRequest {
            request_id: "req-bridge-3".to_string(),
            kind: ScreenshotKind::FullOutput,
            output: Some("eDP-1".to_string()),
            include_cursor: false,
            region: None,
            metadata: ScreenshotRequestMetadata {
                requester: None,
                origin: ScreenshotRequestOrigin::PortalDbus,
                request_marker: Some(4),
                identity_trusted: false,
            },
        };

        let _ = handle_screenshot_bridge_request(request, 42);
        assert_eq!(last_evaluated_client_id_for_test(), 42);
    }
}
