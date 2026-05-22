// Phase 8: client side of the meridian-login IPC handover.
//
// meridian-login (which spawned this compositor) holds the DRM master and
// keeps its login framebuffer on screen. We need to tell it when to step
// aside so our libseat acquire + first KMS commit can take over.
//
// Protocol mirrors bootsplash → login:
//   `handover\n`  →  login releases DRM master but keeps the fd alive so
//                    the scanout buffer stays referenced (no black flash).
//   `exit\n`      →  login closes the fd. By the time we send this, our
//                    first frame is already on screen and owns the
//                    scanout, so login dropping its fb is safe.
//
// Both calls are best-effort. If the socket does not exist (we were not
// launched by meridian-login, or its IPC bind failed), we log a warn and
// continue. The compositor still works without the handshake — there is
// just a brief black flash during the transition.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use tracing::{debug, warn};

const LOGIN_SOCKET: &str = "/run/meridian-login.sock";
const IPC_TIMEOUT: Duration = Duration::from_millis(500);

/// Tell meridian-login to release DRM master (but keep its fd open).
/// Call before `LibSeatSession::new()` / any DRM acquire.
pub fn send_handover() {
    match send_command(b"handover\n") {
        Ok(resp) => debug!(
            response = %resp.trim(),
            "login ipc: handover acked"
        ),
        Err(e) => warn!(
            error = %e,
            socket = LOGIN_SOCKET,
            "login ipc: handover send failed (not launched by meridian-login?)"
        ),
    }
}

/// Tell meridian-login that our first frame is committed to KMS, so it
/// can drop its framebuffer fd. Call right after the first successful
/// `queue_frame`.
pub fn send_first_frame() {
    match send_command(b"exit\n") {
        Ok(resp) => debug!(
            response = %resp.trim(),
            "login ipc: first-frame exit acked"
        ),
        Err(e) => warn!(
            error = %e,
            socket = LOGIN_SOCKET,
            "login ipc: first-frame exit send failed"
        ),
    }
}

fn send_command(cmd: &[u8]) -> std::io::Result<String> {
    let mut s = UnixStream::connect(LOGIN_SOCKET)?;
    s.set_read_timeout(Some(IPC_TIMEOUT))?;
    s.set_write_timeout(Some(IPC_TIMEOUT))?;
    s.write_all(cmd)?;
    let mut buf = [0u8; 256];
    let n = s.read(&mut buf).unwrap_or(0);
    let resp = String::from_utf8_lossy(&buf[..n]).into_owned();
    if resp.starts_with("ok") {
        Ok(resp)
    } else {
        Err(std::io::Error::other(format!(
            "peer refused: {}",
            resp.trim()
        )))
    }
}
