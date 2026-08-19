use std::{
    ffi::{c_char, c_int, CString},
    ptr,
    sync::mpsc,
    thread,
};

use tracing::debug;
use zeroize::Zeroizing;

unsafe extern "C" {
    fn auth_userokay(
        name: *mut c_char,
        style: *mut c_char,
        auth_type: *mut c_char,
        password: *mut c_char,
    ) -> c_int;
}

#[derive(Debug, PartialEq)]
pub enum AuthResult {
    Ok(Vec<(String, String)>),
    Failed,
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthBackend {
    Password,
    Smartcard,
}

pub struct AuthDriver {
    close_tx: mpsc::SyncSender<()>,
    join: Option<thread::JoinHandle<()>>,
}

impl AuthDriver {
    pub fn close(mut self) {
        let _ = self.close_tx.try_send(());
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for AuthDriver {
    fn drop(&mut self) {
        let _ = self.close_tx.try_send(());
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

pub fn start_auth_session(
    username: String,
    password: Zeroizing<String>,
    backend: AuthBackend,
) -> (mpsc::Receiver<AuthResult>, AuthDriver) {
    let (result_tx, result_rx) = mpsc::channel();
    let (close_tx, close_rx) = mpsc::sync_channel(1);
    let join = thread::spawn(move || {
        if backend == AuthBackend::Smartcard {
            let _ = result_tx.send(AuthResult::Error(
                "smartcard authentication is not configured for OpenBSD".to_string(),
            ));
            return;
        }

        let accepted = authenticate_password(&username, password);
        let _ = result_tx.send(if accepted {
            // OpenBSD has no pam_getenvlist/pam_systemd session. The
            // launcher supplies the native runtime/session environment.
            AuthResult::Ok(Vec::new())
        } else {
            AuthResult::Failed
        });
        if accepted {
            let _ = close_rx.recv();
            debug!(user = %username, "BSD Authentication session close requested");
        }
    });

    (
        result_rx,
        AuthDriver {
            close_tx,
            join: Some(join),
        },
    )
}

fn authenticate_password(username: &str, password: Zeroizing<String>) -> bool {
    if username.is_empty() {
        return false;
    }
    let Ok(user) = CString::new(username) else {
        return false;
    };
    let Ok(pass) = CString::new(password.as_str()) else {
        return false;
    };
    drop(password);
    let mut user = user.into_bytes_with_nul();
    let mut pass = pass.into_bytes_with_nul();

    // SAFETY: all pointers are valid writable C strings for the duration
    // of the call. OpenBSD's auth_userokay(3) zeroes `password` before
    // returning; null style/type select the user's configured defaults.
    unsafe {
        auth_userokay(
            user.as_mut_ptr().cast(),
            ptr::null_mut(),
            ptr::null_mut(),
            pass.as_mut_ptr().cast(),
        ) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn empty_username_returns_failed() {
        let (rx, driver) = start_auth_session(
            String::new(),
            Zeroizing::new(String::new()),
            AuthBackend::Password,
        );
        assert_eq!(
            rx.recv_timeout(Duration::from_millis(200)),
            Ok(AuthResult::Failed)
        );
        driver.close();
    }

    #[test]
    fn smartcard_reports_explicit_configuration_error() {
        let (rx, driver) = start_auth_session(
            "nobody".to_string(),
            Zeroizing::new(String::new()),
            AuthBackend::Smartcard,
        );
        assert!(matches!(
            rx.recv_timeout(Duration::from_millis(200)),
            Ok(AuthResult::Error(_))
        ));
        driver.close();
    }
}
