use meridian_ipc::{ScreenshotBridgeError, ScreenshotBridgeRequest};

pub(crate) struct ScreenshotPolicy;

#[cfg(test)]
static LAST_EVALUATED_CLIENT_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScreenshotPolicyContext {
    pub client_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScreenshotPolicyDecision {
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

        if !request.metadata.identity_trusted || request.metadata.requester.is_none() {
            tracing::info!("requester identity unknown/untrusted");
        }

        tracing::info!("screenshot policy decision: deny");
        ScreenshotPolicyDecision::Deny
    }
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

    use super::{ScreenshotPolicy, ScreenshotPolicyContext, ScreenshotPolicyDecision};

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
    fn valid_full_output_is_denied_by_default() {
        let decision =
            ScreenshotPolicy::evaluate(&valid_request(), ScreenshotPolicyContext { client_id: 7 });
        assert_eq!(decision, ScreenshotPolicyDecision::Deny);
    }

    #[test]
    fn region_request_is_unsupported() {
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
    fn unknown_requester_is_denied_safely() {
        let mut request = valid_request();
        request.metadata.requester = None;
        request.metadata.identity_trusted = false;
        let decision =
            ScreenshotPolicy::evaluate(&request, ScreenshotPolicyContext { client_id: 7 });
        assert_eq!(decision, ScreenshotPolicyDecision::Deny);
    }
}
