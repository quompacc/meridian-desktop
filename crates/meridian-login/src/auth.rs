// PAM authentication and session management for meridian-login.
//
// Phase 7b/7c: We keep the PAM handle alive for the duration of the spawned
// compositor. open_session() runs pam_systemd which creates a logind session
// so the compositor can acquire /dev/dri/card0 + input devices via libseat.
// Dropping the handle later triggers close_session + pam_end.
//
// Why pam-sys instead of the `pam` crate? pam_systemd derives `seat` and
// `vtnr` from PAM_TTY ("tty1" -> seat0+vtnr=1). Without that the resulting
// session has SEAT="" and libseat-logind refuses to hand over DRM. The
// `pam` crate does not expose set_item, so we call pam-sys directly.
//
// The pam_handle_t pointer is not Send-safe across threads, so the worker
// thread that runs pam_start ALSO owns the handle for the full session
// lifetime. main signals shutdown via a oneshot channel; the worker reacts
// by running pam_close_session + pam_end and exits.
//
//   start_auth_session  ->  (Receiver<AuthResult>, AuthDriver)
//                                   |                  |
//                                   v                  v
//                          "Ok / Failed / Error"   close()/Drop
//                          (poll from render loop)  joins worker

use std::ffi::{CStr, CString};
use std::os::raw::{c_int, c_void};
use std::ptr;
use std::sync::mpsc;
use std::thread;

use libc::{calloc, free, size_t, strdup};
use pam_sys::{
    pam_acct_mgmt, pam_authenticate, pam_close_session, pam_conv, pam_end, pam_getenvlist,
    pam_handle_t, pam_message, pam_open_session, pam_response, pam_set_item, pam_start,
    PAM_BUF_ERR, PAM_CONV_ERR, PAM_ERROR_MSG, PAM_PROMPT_ECHO_OFF, PAM_PROMPT_ECHO_ON,
    PAM_SUCCESS, PAM_TEXT_INFO, PAM_TTY,
};
use tracing::{debug, warn};
use zeroize::Zeroizing;

const PAM_SERVICE: &str = "meridian-login";
/// pam_systemd parses PAM_TTY: a value like "tty1" yields seat=seat0,
/// vtnr=1 in the new logind session. Required for libseat-logind to
/// accept the DRM acquire from the compositor.
const PAM_TTY_VALUE: &str = "tty1";

#[derive(Debug, PartialEq)]
pub enum AuthResult {
    /// Credentials accepted AND open_session() succeeded. Carries the
    /// snapshot of the PAM environment as exposed by pam_getenvlist —
    /// typically XDG_SESSION_ID, XDG_SEAT, XDG_VTNR set by pam_systemd.
    /// The matching [`AuthDriver`] must be kept alive until the compositor
    /// exits, then [`AuthDriver::close`]ed.
    Ok(Vec<(String, String)>),
    /// Credentials rejected (wrong password, unknown user, account locked, ...).
    Failed,
    /// PAM itself failed (missing config, library not loadable, session
    /// setup error, ...). The caller treats this like Failed for the UI but
    /// logs the detail.
    Error(String),
}

/// Handle held by main for the lifetime of the user's session. Sending
/// `()` (or dropping it) tells the worker thread to run pam_close_session
/// + pam_end. Both `close()` and `Drop` block until the worker thread
/// has fully finished, so by the time control returns to main the logind
/// session is gone.
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

/// Spawn a worker that runs the full PAM lifecycle (pam_start ->
/// pam_set_item(PAM_TTY) -> authenticate -> open_session -> wait ->
/// close_session -> pam_end). Returns a receiver for the auth outcome
/// and an AuthDriver that controls session teardown.
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

/// Conversation-handler state. Lives on the heap (Box) so its address is
/// stable; pointer is handed to libpam via pam_conv.appdata_ptr.
struct ConvData {
    user: CString,
    pass: CString,
}

impl Drop for ConvData {
    fn drop(&mut self) {
        // Zeroize the password bytes before CString's free returns the
        // allocation to the heap. Without this the plaintext lingers in
        // the freed slab until a later allocation overwrites it — and
        // since ConvData lives for the full compositor session (often
        // hours), that window is far larger than necessary.
        //
        // Note: this only zeroes OUR copy. libpam's conv_cb response
        // (strdup'd inside the conversation callback) is freed by libpam
        // without zeroization shortly after pam_authenticate returns —
        // that brief window is the unavoidable cost of using libpam.
        let pass = std::mem::replace(&mut self.pass, CString::default());
        let raw = pass.into_raw();
        // SAFETY: raw was just produced by CString::into_raw and points
        // to a NUL-terminated C-string we still uniquely own; strlen is
        // bounded by the original allocation.
        let len = unsafe { libc::strlen(raw) } + 1;
        for i in 0..len {
            // write_volatile prevents the optimiser from eliding the
            // zeroization on the about-to-be-freed memory.
            unsafe {
                std::ptr::write_volatile(raw.add(i) as *mut u8, 0);
            }
        }
        // Reconstruct + drop frees the (now-zeroed) allocation.
        unsafe { drop(CString::from_raw(raw)) };
    }
}

unsafe extern "C" fn conv_cb(
    num_msg: c_int,
    msg: *mut *const pam_message,
    out_resp: *mut *mut pam_response,
    appdata_ptr: *mut c_void,
) -> c_int {
    if num_msg <= 0 || appdata_ptr.is_null() || msg.is_null() || out_resp.is_null() {
        return PAM_CONV_ERR;
    }
    let resp = calloc(num_msg as usize, std::mem::size_of::<pam_response>() as size_t)
        as *mut pam_response;
    if resp.is_null() {
        return PAM_BUF_ERR;
    }
    let data = &*(appdata_ptr as *const ConvData);
    for i in 0..num_msg as isize {
        let m_ptr = *msg.offset(i);
        if m_ptr.is_null() {
            free(resp as *mut c_void);
            return PAM_CONV_ERR;
        }
        let m = &*m_ptr;
        let r = &mut *resp.offset(i);
        match m.msg_style {
            x if x == PAM_PROMPT_ECHO_ON => {
                r.resp = strdup(data.user.as_ptr());
            }
            x if x == PAM_PROMPT_ECHO_OFF => {
                r.resp = strdup(data.pass.as_ptr());
            }
            x if x == PAM_TEXT_INFO || x == PAM_ERROR_MSG => {
                // No textual response needed.
            }
            _ => {
                free(resp as *mut c_void);
                return PAM_CONV_ERR;
            }
        }
    }
    *out_resp = resp;
    PAM_SUCCESS
}

/// Drain pam_getenvlist into a Rust-owned Vec, freeing every entry and
/// the array itself per the Linux-PAM contract.
fn drain_pam_env(pamh: *mut pam_handle_t) -> Vec<(String, String)> {
    let mut out = Vec::new();
    // SAFETY: pamh comes from pam_start and is still live (open_session ok).
    let list = unsafe { pam_getenvlist(pamh) };
    if list.is_null() {
        return out;
    }
    let mut i = 0isize;
    loop {
        // SAFETY: NULL-terminated array, we stop at the first null entry.
        let entry = unsafe { *list.offset(i) };
        if entry.is_null() {
            break;
        }
        // SAFETY: entry is a NUL-terminated KEY=VALUE C-string owned by libpam.
        if let Ok(s) = unsafe { CStr::from_ptr(entry) }.to_str() {
            if let Some((k, v)) = s.split_once('=') {
                out.push((k.to_string(), v.to_string()));
            }
        }
        // Linux-PAM hands us ownership of each entry; we must free them.
        unsafe { free(entry as *mut c_void) };
        i += 1;
    }
    unsafe { free(list as *mut c_void) };
    out
}

/// Owns the raw pam handle so pam_end always runs even on early returns.
struct PamGuard {
    pamh: *mut pam_handle_t,
    last_status: c_int,
}

impl Drop for PamGuard {
    fn drop(&mut self) {
        if !self.pamh.is_null() {
            // SAFETY: pamh was returned by pam_start and not yet ended.
            unsafe {
                pam_end(self.pamh, self.last_status);
            }
            self.pamh = ptr::null_mut();
        }
    }
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

    // CString conversion: a NUL byte inside the password would otherwise
    // silently truncate, so a NulError is treated as auth failure.
    let user_c = match CString::new(username) {
        Ok(s) => s,
        Err(_) => {
            let _ = result_tx.send(AuthResult::Failed);
            return;
        }
    };
    let pass_c = match CString::new(password.as_str()) {
        Ok(s) => s,
        Err(_) => {
            let _ = result_tx.send(AuthResult::Failed);
            return;
        }
    };
    // Plaintext password now lives only inside pass_c; wipe ours.
    drop(password);

    let service_c = CString::new(PAM_SERVICE).expect("static service name has no NUL");
    let tty_c = CString::new(PAM_TTY_VALUE).expect("static tty value has no NUL");

    // Box keeps a stable address while libpam holds the appdata_ptr.
    let conv_data = Box::new(ConvData {
        user: user_c.clone(),
        pass: pass_c,
    });
    let conv_data_ptr = Box::into_raw(conv_data);

    let conversation = pam_conv {
        conv: Some(conv_cb),
        appdata_ptr: conv_data_ptr as *mut c_void,
    };

    let mut pamh: *mut pam_handle_t = ptr::null_mut();
    let rc = unsafe {
        pam_start(
            service_c.as_ptr(),
            user_c.as_ptr(),
            &conversation,
            &mut pamh,
        )
    };
    if rc != PAM_SUCCESS || pamh.is_null() {
        warn!(rc, service = PAM_SERVICE, "pam_start failed");
        // SAFETY: conv_data_ptr came from Box::into_raw and was never aliased.
        unsafe { drop(Box::from_raw(conv_data_ptr)) };
        let _ = result_tx.send(AuthResult::Error(format!("pam_start: rc={}", rc)));
        return;
    }
    let mut guard = PamGuard {
        pamh,
        last_status: PAM_SUCCESS,
    };

    // Seat-binding: pam_systemd derives seat=seat0, vtnr=1 from PAM_TTY="tty1".
    // Without this the new logind session has SEAT="" and libseat-logind
    // refuses to hand over DRM, breaking the compositor launch.
    let rc = unsafe {
        pam_set_item(
            guard.pamh,
            PAM_TTY as c_int,
            tty_c.as_ptr() as *const c_void,
        )
    };
    if rc != PAM_SUCCESS {
        // Non-fatal: log and continue. Auth may still succeed, but the
        // resulting session will lack a seat and the compositor spawn
        // downstream will fail loudly on DRM acquire.
        warn!(rc, "pam_set_item(PAM_TTY) failed; session will lack seat");
    } else {
        debug!(tty = PAM_TTY_VALUE, "pam_set_item(PAM_TTY) ok");
    }

    let rc = unsafe { pam_authenticate(guard.pamh, 0) };
    guard.last_status = rc;
    if rc != PAM_SUCCESS {
        debug!(user = %username, rc, "PAM auth failed");
        let _ = result_tx.send(AuthResult::Failed);
        drop(guard);
        unsafe { drop(Box::from_raw(conv_data_ptr)) };
        return;
    }
    debug!(user = %username, "PAM auth ok");

    let rc = unsafe { pam_acct_mgmt(guard.pamh, 0) };
    guard.last_status = rc;
    if rc != PAM_SUCCESS {
        debug!(user = %username, rc, "PAM acct_mgmt failed");
        let _ = result_tx.send(AuthResult::Failed);
        drop(guard);
        unsafe { drop(Box::from_raw(conv_data_ptr)) };
        return;
    }

    let rc = unsafe { pam_open_session(guard.pamh, 0) };
    guard.last_status = rc;
    if rc != PAM_SUCCESS {
        warn!(user = %username, rc, "PAM open_session failed");
        let _ = result_tx.send(AuthResult::Error(format!("open_session: rc={}", rc)));
        drop(guard);
        unsafe { drop(Box::from_raw(conv_data_ptr)) };
        return;
    }
    debug!(user = %username, "PAM session opened");

    let pam_env = drain_pam_env(guard.pamh);
    debug!(user = %username, count = pam_env.len(), "captured PAM env snapshot");

    let _ = result_tx.send(AuthResult::Ok(pam_env));

    // Block until main signals teardown. RecvError == sender dropped, same
    // outcome: tear the session down.
    let _ = close_rx.recv();
    debug!(user = %username, "PAM session close requested");

    let rc = unsafe { pam_close_session(guard.pamh, 0) };
    if rc != PAM_SUCCESS {
        warn!(rc, "pam_close_session returned non-success");
    }
    guard.last_status = rc;
    drop(guard); // triggers pam_end
    // SAFETY: libpam no longer references appdata_ptr after pam_end.
    unsafe { drop(Box::from_raw(conv_data_ptr)) };
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

    #[test]
    fn drain_pam_env_on_null_handle_is_empty() {
        // pam_getenvlist on a null handle returns null per the Linux-PAM
        // contract; drain_pam_env must yield an empty Vec without panic.
        let env = drain_pam_env(std::ptr::null_mut());
        assert!(env.is_empty());
    }
}
