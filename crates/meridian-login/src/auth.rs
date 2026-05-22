// PAM authentication and session management for meridian-login.
//
// Phase 7b: We do not just authenticate, we also keep the PAM handle alive
// for the duration of the spawned compositor. open_session() is what runs
// pam_systemd and creates a logind session so the compositor can acquire
// /dev/dri/card0 + input devices via libseat. Dropping the handle later
// triggers close_session + pam_end.
//
// The pam crate's Client cannot be freely sent across threads, so the
// worker thread that runs PAM also OWNS the handle for the full session
// lifetime. main signals shutdown via a oneshot channel; the worker reacts
// by dropping the Client and exiting.
//
//   start_auth_session  ->  (Receiver<AuthResult>, AuthDriver)
//                                   |                  |
//                                   v                  v
//                          "Ok / Failed / Error"   close()/Drop
//                          (poll from render loop)  joins worker
//
// On AuthResult::Failed or AuthResult::Error the worker has already
// finished — dropping the AuthDriver just joins it.

use std::sync::mpsc;
use std::thread;

use pam::Client;
use tracing::{debug, warn};
use zeroize::Zeroizing;

const PAM_SERVICE: &str = "meridian-login";

#[derive(Debug, PartialEq)]
pub enum AuthResult {
    /// Credentials accepted AND open_session() succeeded. The compositor
    /// can now be spawned. The matching [`AuthDriver`] must be kept alive
    /// until the compositor exits, then [`AuthDriver::close`]ed.
    Ok,
    /// Credentials rejected (wrong password, unknown user, account locked, …).
    Failed,
    /// PAM itself failed (missing config, library not loadable, session
    /// setup error, …). The caller treats this like Failed for the UI but
    /// logs the detail.
    Error(String),
}

/// Handle held by main for the lifetime of the user's session. Sending
/// `()` (or dropping it) tells the worker thread to drop the pam::Client,
/// which in turn calls close_session and pam_end. Both `close()` and
/// `Drop` block until the worker thread has fully finished, so by the
/// time control returns to main the logind session is gone.
pub struct AuthDriver {
    close_tx: mpsc::SyncSender<()>,
    join: Option<thread::JoinHandle<()>>,
}

impl AuthDriver {
    /// Explicit close — sends the shutdown signal and waits for the worker.
    /// Prefer this over Drop so the teardown point is visible in logs.
    pub fn close(mut self) {
        let _ = self.close_tx.try_send(());
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for AuthDriver {
    fn drop(&mut self) {
        let _ = self.close_tx.try_send(());
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

/// Spawn a worker that runs the full PAM lifecycle (authenticate →
/// open_session → wait → close_session via Drop). Returns a receiver for
/// the auth outcome and an AuthDriver that controls session teardown.
pub fn start_auth_session(
    username: String,
    password: Zeroizing<String>,
) -> (mpsc::Receiver<AuthResult>, AuthDriver) {
    let (result_tx, result_rx) = mpsc::channel();
    let (close_tx, close_rx) = mpsc::sync_channel::<()>(1);

    let join = thread::spawn(move || {
        run_pam_session(&username, password, &result_tx, &close_rx);
    });

    (result_rx, AuthDriver { close_tx, join: Some(join) })
}

fn run_pam_session(
    username: &str,
    password: Zeroizing<String>,
    result_tx: &mpsc::Sender<AuthResult>,
    close_rx: &mpsc::Receiver<()>,
) {
    if username.is_empty() {
        let _ = result_tx.send(AuthResult::Failed);
        return;
    }

    let mut client = match Client::with_password(PAM_SERVICE) {
        Ok(c) => c,
        Err(e) => {
            warn!(error = ?e, service = PAM_SERVICE, "PAM init failed");
            let _ = result_tx.send(AuthResult::Error(format!("pam init: {:?}", e)));
            return;
        }
    };
    client
        .conversation_mut()
        .set_credentials(username, password.as_str());
    // Now that PAM has its own copy, wipe ours.
    drop(password);

    if let Err(e) = client.authenticate() {
        debug!(user = %username, error = ?e, "PAM auth failed");
        let _ = result_tx.send(AuthResult::Failed);
        return;
    }
    debug!(user = %username, "PAM auth ok");

    if let Err(e) = client.open_session() {
        warn!(user = %username, error = ?e, "PAM open_session failed");
        let _ = result_tx.send(AuthResult::Error(format!("open_session: {:?}", e)));
        return;
    }
    debug!(user = %username, "PAM session opened");

    let _ = result_tx.send(AuthResult::Ok);

    // Block until main signals teardown. RecvError == sender dropped, same
    // outcome: tear the session down.
    let _ = close_rx.recv();
    debug!(user = %username, "PAM session close requested");

    drop(client);
    debug!(user = %username, "PAM session closed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn empty_username_returns_failed_quickly() {
        let (rx, driver) = start_auth_session(String::new(), Zeroizing::new(String::new()));
        let deadline = Instant::now() + Duration::from_millis(200);
        let result = loop {
            if let Ok(r) = rx.try_recv() {
                break r;
            }
            if Instant::now() > deadline {
                panic!("worker did not deliver Failed within 200ms");
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(result, AuthResult::Failed);
        driver.close();
    }
}
