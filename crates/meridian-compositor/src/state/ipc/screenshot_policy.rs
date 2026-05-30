use meridian_ipc::{ScreenshotBridgeError, ScreenshotBridgeRequest, ScreenshotRequestOrigin};

pub(crate) struct ScreenshotPolicy;

#[cfg(test)]
static LAST_EVALUATED_CLIENT_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[cfg(test)]
pub(crate) fn screenshot_policy_test_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScreenshotPolicyContext {
    pub client_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScreenshotPolicyDecision {
    /// Capture is permitted immediately; the render loop should fulfil it.
    Allow,
    /// The request must be confirmed by the user before capture; the caller
    /// shows a consent modal and decides based on the answer.
    NeedsConsent,
    Deny,
    Unsupported(String),
    Invalid(String),
}

impl ScreenshotPolicy {
    pub(crate) fn evaluate(
        request: &ScreenshotBridgeRequest,
        context: ScreenshotPolicyContext,
    ) -> ScreenshotPolicyDecision {
        #[cfg(test)]
        LAST_EVALUATED_CLIENT_ID.store(context.client_id, std::sync::atomic::Ordering::Relaxed);

        tracing::info!(
            "screenshot policy evaluating request: request_id={} client_id={} origin={:?} requester={:?} trusted={} kind={:?} output={:?} include_cursor={}",
            request.request_id,
            context.client_id,
            request.metadata.origin,
            request.metadata.requester,
            request.metadata.identity_trusted,
            request.kind,
            request.output,
            request.include_cursor
        );

        if let Err(error) = request.validate() {
            return match error {
                ScreenshotBridgeError::InvalidRequest(message) => {
                    tracing::info!("screenshot policy decision: invalid");
                    ScreenshotPolicyDecision::Invalid(message)
                }
                ScreenshotBridgeError::Unsupported(message) => {
                    tracing::info!("screenshot policy decision: unsupported");
                    ScreenshotPolicyDecision::Unsupported(message)
                }
                ScreenshotBridgeError::PermissionDenied(_)
                | ScreenshotBridgeError::CompositorUnavailable(_)
                | ScreenshotBridgeError::Internal(_) => {
                    tracing::info!("screenshot policy decision: invalid");
                    ScreenshotPolicyDecision::Invalid(
                        "unexpected request validation state".to_string(),
                    )
                }
            };
        }

        // SECURITY: `origin` is a self-declared field in the request, so it
        // cannot by itself prove trust — any local process with access to the
        // bridge socket could assert `Internal`. The real internal screenshot
        // path (the panel button) uses wlr-screencopy, not this bridge, and the
        // external portal route (origin=PortalDbus) must go through interactive
        // consent (a later slice). Production is therefore deny-by-default for
        // every origin. The Internal allow-path exists ONLY to verify the
        // capture engine in development and is gated behind an explicit env
        // flag the compositor process must be started with — it is never on by
        // default, so it is not a self-declared-field bypass in production.
        if request.metadata.origin == ScreenshotRequestOrigin::Internal
            && internal_capture_dev_enabled()
        {
            tracing::warn!("screenshot policy: allowing internal-origin request (DEV flag set)");
            return ScreenshotPolicyDecision::Allow;
        }

        // Portal-routed requests (a normal app asking via xdg-desktop-portal)
        // are neither blindly allowed nor denied: they require explicit user
        // consent. The compositor shows a modal and captures only if the user
        // agrees. This is the trustworthy path — consent is mediated by the
        // compositor/shell, not by any self-declared request field.
        if request.metadata.origin == ScreenshotRequestOrigin::PortalDbus {
            tracing::info!("screenshot policy decision: needs-consent (portal origin)");
            return ScreenshotPolicyDecision::NeedsConsent;
        }

        if !request.metadata.identity_trusted || request.metadata.requester.is_none() {
            tracing::info!("requester identity unknown/untrusted");
        }

        tracing::info!("screenshot policy decision: deny");
        ScreenshotPolicyDecision::Deny
    }
}

/// Dev-only gate for the internal capture path (see the SECURITY note in
/// `evaluate`). Off unless `MERIDIAN_SCREENSHOT_DEV=1` is in the compositor's
/// environment; never enabled in a normal session.
fn internal_capture_dev_enabled() -> bool {
    std::env::var("MERIDIAN_SCREENSHOT_DEV")
        .map(|v| v == "1")
        .unwrap_or(false)
}

#[cfg(test)]
pub(crate) fn last_evaluated_client_id_for_test() -> u64 {
    LAST_EVALUATED_CLIENT_ID.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use meridian_ipc::{
        ScreenshotBridgeRequest, ScreenshotKind, ScreenshotRequestMetadata, ScreenshotRequestOrigin,
    };

    use super::{
        screenshot_policy_test_lock, ScreenshotPolicy, ScreenshotPolicyContext,
        ScreenshotPolicyDecision,
    };

    fn valid_request() -> ScreenshotBridgeRequest {
        ScreenshotBridgeRequest {
            request_id: "req-policy-1".to_string(),
            kind: ScreenshotKind::FullOutput,
            output: Some("eDP-1".to_string()),
            include_cursor: false,
            region: None,
            metadata: ScreenshotRequestMetadata {
                requester: None,
                origin: ScreenshotRequestOrigin::PortalDbus,
                request_marker: Some(1),
                identity_trusted: false,
            },
        }
    }

    #[test]
    fn portal_request_needs_consent_not_auto_allowed() {
        let _guard = screenshot_policy_test_lock().lock().expect("test lock");
        // valid_request() uses the PortalDbus origin: a normal app asking via
        // the portal must be confirmed by the user, never auto-allowed.
        let decision =
            ScreenshotPolicy::evaluate(&valid_request(), ScreenshotPolicyContext { client_id: 7 });
        assert_eq!(decision, ScreenshotPolicyDecision::NeedsConsent);
    }

    #[test]
    fn unknown_origin_is_denied() {
        let _guard = screenshot_policy_test_lock().lock().expect("test lock");
        let mut request = valid_request();
        request.metadata.origin = ScreenshotRequestOrigin::Unknown;
        let decision =
            ScreenshotPolicy::evaluate(&request, ScreenshotPolicyContext { client_id: 7 });
        assert_eq!(decision, ScreenshotPolicyDecision::Deny);
    }

    #[test]
    fn internal_origin_allowed_only_with_dev_flag() {
        let _guard = screenshot_policy_test_lock().lock().expect("test lock");
        let mut request = valid_request();
        request.metadata.origin = ScreenshotRequestOrigin::Internal;

        // Without the dev flag, even internal origin is denied (no
        // self-declared-field bypass in production).
        std::env::remove_var("MERIDIAN_SCREENSHOT_DEV");
        assert_eq!(
            ScreenshotPolicy::evaluate(&request, ScreenshotPolicyContext { client_id: 0 }),
            ScreenshotPolicyDecision::Deny
        );

        // With the dev flag set, internal origin is allowed.
        std::env::set_var("MERIDIAN_SCREENSHOT_DEV", "1");
        assert_eq!(
            ScreenshotPolicy::evaluate(&request, ScreenshotPolicyContext { client_id: 0 }),
            ScreenshotPolicyDecision::Allow
        );
        std::env::remove_var("MERIDIAN_SCREENSHOT_DEV");
    }

    #[test]
    fn internal_origin_with_invalid_request_still_rejected() {
        // Validation runs before the origin allow-path, so a malformed internal
        // request is still rejected even with the dev flag set.
        let _guard = screenshot_policy_test_lock().lock().expect("test lock");
        std::env::set_var("MERIDIAN_SCREENSHOT_DEV", "1");
        let mut request = valid_request();
        request.metadata.origin = ScreenshotRequestOrigin::Internal;
        request.request_id = " ".to_string();
        let decision =
            ScreenshotPolicy::evaluate(&request, ScreenshotPolicyContext { client_id: 0 });
        std::env::remove_var("MERIDIAN_SCREENSHOT_DEV");
        assert_eq!(
            decision,
            ScreenshotPolicyDecision::Invalid("request_id must not be empty".to_string())
        );
    }

    #[test]
    fn region_request_is_unsupported() {
        let _guard = screenshot_policy_test_lock().lock().expect("test lock");
        let mut request = valid_request();
        request.region = Some(meridian_ipc::ScreenshotRegion {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        });
        let decision =
            ScreenshotPolicy::evaluate(&request, ScreenshotPolicyContext { client_id: 7 });
        assert_eq!(
            decision,
            ScreenshotPolicyDecision::Unsupported(
                "region capture is not implemented yet".to_string()
            )
        );
    }

    #[test]
    fn invalid_request_is_invalid() {
        let _guard = screenshot_policy_test_lock().lock().expect("test lock");
        let mut request = valid_request();
        request.request_id = " ".to_string();
        let decision =
            ScreenshotPolicy::evaluate(&request, ScreenshotPolicyContext { client_id: 7 });
        assert_eq!(
            decision,
            ScreenshotPolicyDecision::Invalid("request_id must not be empty".to_string())
        );
    }

    #[test]
    fn unknown_requester_via_portal_still_needs_consent() {
        let _guard = screenshot_policy_test_lock().lock().expect("test lock");
        // Even with no trusted requester identity, a portal request is gated by
        // consent rather than blindly allowed — the user is the gate.
        let mut request = valid_request();
        request.metadata.requester = None;
        request.metadata.identity_trusted = false;
        let decision =
            ScreenshotPolicy::evaluate(&request, ScreenshotPolicyContext { client_id: 7 });
        assert_eq!(decision, ScreenshotPolicyDecision::NeedsConsent);
    }
}
