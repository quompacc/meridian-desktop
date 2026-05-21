// PAM authentication for meridian-login.
//
// Wraps the `pam` crate behind a small AuthResult enum so the UI does not
// need to know the internals. Phase 6 only does `authenticate`; Phase 7
// will additionally open a session (acct_mgmt, open_session) and stash the
// PAM handle for the compositor process.

use pam::Client;
use tracing::{debug, warn};

/// PAM service name. There must be a matching `/etc/pam.d/meridian-login`
/// file on the system.
const PAM_SERVICE: &str = "meridian-login";

#[derive(Debug, PartialEq)]
pub enum AuthResult {
    /// PAM accepted the credentials.
    Ok,
    /// PAM rejected the credentials (wrong password, unknown user, …).
    Failed,
    /// PAM itself encountered an error (config missing, lib not loadable, …).
    Error(String),
}

/// Synchronous PAM authenticate. For pam_unix this returns in a few ms;
/// for slower modules (LDAP, Kerberos) it can take seconds — the caller
/// is expected to handle the UI freeze for now.
///
/// The password is borrowed and never logged.
pub fn try_authenticate(username: &str, password: &str) -> AuthResult {
    if username.is_empty() {
        return AuthResult::Failed;
    }
    let mut client = match Client::with_password(PAM_SERVICE) {
        Ok(c) => c,
        Err(e) => {
            warn!(error = ?e, service = PAM_SERVICE, "PAM init failed");
            return AuthResult::Error(format!("pam init: {:?}", e));
        }
    };
    client
        .conversation_mut()
        .set_credentials(username, password);
    match client.authenticate() {
        Ok(()) => {
            debug!(user = %username, "PAM auth ok");
            AuthResult::Ok
        }
        Err(e) => {
            debug!(user = %username, error = ?e, "PAM auth failed");
            AuthResult::Failed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_username_is_failed() {
        // No PAM call should happen — empty username short-circuits.
        assert_eq!(try_authenticate("", "whatever"), AuthResult::Failed);
    }
}
