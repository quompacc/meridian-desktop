use meridian_ipc::{ScreenshotBridgeError, ScreenshotBridgeRequest, ScreenshotBridgeResult};

const SCREENSHOT_PERMISSION_DENIED_MSG: &str = "screenshot denied by compositor policy";

use super::screenshot_policy::{
    ScreenshotPolicy, ScreenshotPolicyContext, ScreenshotPolicyDecision,
};

/// What the caller should do with a bridge request after policy evaluation.
pub(crate) enum ScreenshotBridgeOutcome {
    /// Permitted — capture must be fulfilled by the render loop; no response is
    /// sent yet (the render loop sends it once the PNG is written).
    Queue(ScreenshotBridgeRequest),
    /// Needs user consent — hold the request, show a modal, and decide based on
    /// the answer. No response is sent yet.
    AwaitConsent(ScreenshotBridgeRequest),
    /// Needs the user to pick a region — hold the request, show a fullscreen
    /// drag-rectangle picker, and apply the picked region (or cancel) when the
    /// `ScreenshotRegionResponse` comes back. No response is sent yet.
    AwaitRegionPick(ScreenshotBridgeRequest),
    /// Rejected — send this error result back to the requester immediately.
    Respond(ScreenshotBridgeResult),
}

pub(crate) fn handle_screenshot_bridge_request(
    request: ScreenshotBridgeRequest,
    client_id: u64,
) -> ScreenshotBridgeOutcome {
    let decision = ScreenshotPolicy::evaluate(&request, ScreenshotPolicyContext { client_id });
    match decision {
        ScreenshotPolicyDecision::Allow => ScreenshotBridgeOutcome::Queue(request),
        ScreenshotPolicyDecision::NeedsConsent => ScreenshotBridgeOutcome::AwaitConsent(request),
        ScreenshotPolicyDecision::NeedsRegionPick => {
            ScreenshotBridgeOutcome::AwaitRegionPick(request)
        }
        ScreenshotPolicyDecision::Deny => {
            ScreenshotBridgeOutcome::Respond(ScreenshotBridgeResult::Error {
                error: ScreenshotBridgeError::PermissionDenied(
                    SCREENSHOT_PERMISSION_DENIED_MSG.to_string(),
                ),
            })
        }
        ScreenshotPolicyDecision::Unsupported(message) => {
            ScreenshotBridgeOutcome::Respond(ScreenshotBridgeResult::Error {
                error: ScreenshotBridgeError::Unsupported(message),
            })
        }
        ScreenshotPolicyDecision::Invalid(message) => {
            ScreenshotBridgeOutcome::Respond(ScreenshotBridgeResult::Error {
                error: ScreenshotBridgeError::InvalidRequest(message),
            })
        }
    }
}

/// Build the standard "permission denied" result for a request the user
/// declined (or that policy refused). Shared by the consent-response path.
pub(crate) fn permission_denied_result() -> ScreenshotBridgeResult {
    ScreenshotBridgeResult::Error {
        error: ScreenshotBridgeError::PermissionDenied(
            SCREENSHOT_PERMISSION_DENIED_MSG.to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use meridian_ipc::{
        ScreenshotBridgeError, ScreenshotBridgeRequest, ScreenshotBridgeResult, ScreenshotKind,
        ScreenshotRegion, ScreenshotRequestMetadata, ScreenshotRequestOrigin,
    };

    use super::{handle_screenshot_bridge_request, ScreenshotBridgeOutcome};
    use crate::state::ipc::screenshot_policy::{
        last_evaluated_client_id_for_test, screenshot_policy_test_lock,
    };

    /// Unwrap a rejection outcome to its error result for assertions.
    fn expect_respond(outcome: ScreenshotBridgeOutcome) -> ScreenshotBridgeResult {
        match outcome {
            ScreenshotBridgeOutcome::Respond(result) => result,
            ScreenshotBridgeOutcome::Queue(_) => panic!("expected Respond, got Queue"),
            ScreenshotBridgeOutcome::AwaitConsent(_) => {
                panic!("expected Respond, got AwaitConsent")
            }
            ScreenshotBridgeOutcome::AwaitRegionPick(_) => {
                panic!("expected Respond, got AwaitRegionPick")
            }
        }
    }

    fn is_await_consent(outcome: &ScreenshotBridgeOutcome) -> bool {
        matches!(outcome, ScreenshotBridgeOutcome::AwaitConsent(_))
    }

    fn is_await_region_pick(outcome: &ScreenshotBridgeOutcome) -> bool {
        matches!(outcome, ScreenshotBridgeOutcome::AwaitRegionPick(_))
    }

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
                interactive: false,
            },
        };

        assert_eq!(
            expect_respond(handle_screenshot_bridge_request(request, 7)),
            ScreenshotBridgeResult::Error {
                error: ScreenshotBridgeError::InvalidRequest(
                    "request_id must not be empty".to_string(),
                ),
            }
        );
    }

    #[test]
    fn portal_request_awaits_consent() {
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
                interactive: false,
            },
        };

        // A portal request is neither captured nor denied outright — it waits
        // for the user's consent.
        assert!(is_await_consent(&handle_screenshot_bridge_request(
            request, 7
        )));
    }

    #[test]
    fn region_request_with_nonzero_size_follows_consent_path() {
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
                interactive: false,
            },
        };

        // Region requests now pass validation; portal origin still requires
        // user consent (no interactive flag yet — the region-pick path is
        // wired in a follow-on slice).
        assert!(is_await_consent(&handle_screenshot_bridge_request(
            request, 7
        )));
    }

    #[test]
    fn portal_interactive_request_awaits_region_pick() {
        let _guard = screenshot_policy_test_lock().lock().expect("test lock");
        let request = ScreenshotBridgeRequest {
            request_id: "req-bridge-region".to_string(),
            kind: ScreenshotKind::FullOutput,
            output: Some("eDP-1".to_string()),
            include_cursor: false,
            region: None,
            metadata: ScreenshotRequestMetadata {
                requester: Some("org.example.App".to_string()),
                origin: ScreenshotRequestOrigin::PortalDbus,
                request_marker: Some(9),
                identity_trusted: false,
                interactive: true,
            },
        };

        assert!(is_await_region_pick(&handle_screenshot_bridge_request(
            request, 7
        )));
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
                interactive: false,
            },
        };

        let _ = handle_screenshot_bridge_request(request, 42);
        assert_eq!(last_evaluated_client_id_for_test(), 42);
    }
}
