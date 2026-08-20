//! External-process helpers with a hard timeout.
//!
//! The shell is single-threaded: every synchronous `Command` call on the
//! event loop freezes the whole UI (clock, launcher, popups, OSD) while the
//! tool runs. nmcli, wpctl and mixer normally return in well under 100 ms,
//! but a wedged NetworkManager, D-Bus or PipeWire can hang indefinitely
//! (P2-1, AUDIT_2026-08-19). Every read/write path on the event loop
//! therefore goes through [`output_with_timeout`], which bounds the wait and
//! kills the child when the deadline expires.

use std::{
    io::Read,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

/// How often the wait loop checks whether the child has exited. Small enough
/// that a finished tool does not cost a noticeable follow-up delay, large
/// enough that the polling itself is negligible.
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Run `cmd` and kill it once `timeout` elapses.
///
/// Returns `None` on spawn failure, wait error or timeout — callers treat
/// that exactly like "tool unavailable" (offline state, empty list, no-op
/// write), so a hung backend degrades gracefully instead of stalling the
/// shell. stdout/stderr are drained on reader threads so a chatty child
/// cannot fill the OS pipe buffers and deadlock the wait loop.
pub(crate) fn output_with_timeout(cmd: &mut Command, timeout: Duration) -> Option<Output> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(_) => return None,
    };

    let stdout = child.stdout.take().map(drain_pipe);
    let stderr = child.stderr.take().map(drain_pipe);

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    // Leave the reader threads owning their pipe ends; killing
                    // the child closes the write ends and lets them finish.
                    let _ = child.kill();
                    let _ = child.wait();
                    tracing::warn!("{program} exceeded its {timeout:?} deadline and was killed");
                    return None;
                }
                thread::sleep(WAIT_POLL_INTERVAL);
            }
            Err(_) => return None,
        }
    };

    let stdout = stdout
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();
    let stderr = stderr
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();
    Some(Output {
        status,
        stdout,
        stderr,
    })
}

/// Read a child pipe to EOF on a dedicated thread.
fn drain_pipe<R: Read + Send + 'static>(mut pipe: R) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        buf
    })
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::output_with_timeout;
    use std::{process::Command, time::Duration};

    #[test]
    fn fast_command_returns_stdout() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "echo meridian-ok"]);
        let output = output_with_timeout(&mut cmd, Duration::from_secs(5)).expect("ran");
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "meridian-ok"
        );
    }

    #[test]
    fn hung_command_is_killed_and_reported_as_none() {
        let start = std::time::Instant::now();
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        let output = output_with_timeout(&mut cmd, Duration::from_millis(150));
        assert!(output.is_none());
        // Generous upper bound: the point is that we do not wait for `sleep 30`.
        assert!(start.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn missing_binary_is_none() {
        let mut cmd = Command::new("definitely-not-a-real-binary-xyz");
        assert!(output_with_timeout(&mut cmd, Duration::from_secs(1)).is_none());
    }
}
