// Authenticate via polkit's setuid helper `polkit-agent-helper-1`.
//
// The agent process itself runs as the user (non-root), so it cannot
// call `org.freedesktop.PolicyKit1.Authority.AuthenticationAgentResponse2`
// directly — polkitd refuses with "Only uid 0 may invoke this method."
// Every standard polkit agent (polkit-gnome, polkit-kde, mate-polkit,
// lxqt-policykit) shells out to the setuid helper instead. The helper
// owns the PAM conversation AND the Response call on polkitd, both as
// root.
//
// Protocol (stable since polkit 0.105):
//
//   spawn  /usr/lib/polkit-1/polkit-agent-helper-1 <username>
//   write  <cookie>\n        — on helper's stdin
//   loop   read line from helper's stdout
//           "PAM_PROMPT_ECHO_OFF <prompt>\n" → write password + \n
//           "PAM_PROMPT_ECHO_ON  <prompt>\n" → write text + \n
//           "PAM_TEXT_INFO <line>\n"          → log only
//           "PAM_ERROR_MSG <line>\n"          → log only
//           "SUCCESS\n"                       → helper already called
//                                              Response on polkitd
//           "FAILURE\n"                       → bad password / cancelled
//
// We only support a single ECHO_OFF prompt (password). ECHO_ON would
// indicate the PAM stack wants extra input we don't have a UI for; we
// answer with an empty line so the helper can fail cleanly instead of
// hanging.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use zeroize::Zeroizing;

const HELPER_PATHS: &[&str] = &[
    "/usr/local/lib/polkit-1/polkit-agent-helper-1",
    "/usr/lib/polkit-1/polkit-agent-helper-1",
    "/usr/libexec/polkit-agent-helper-1",
    "/usr/lib/policykit-1/polkit-agent-helper-1",
];
const HELPER_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

fn find_helper() -> Option<&'static str> {
    #[cfg(target_os = "openbsd")]
    {
        // The sandbox grants execute-only access to the fixed package helper.
        // A preceding Path::exists() would require read-path metadata access
        // and would also introduce a check/use race.
        return HELPER_PATHS.first().copied();
    }

    #[cfg(not(target_os = "openbsd"))]
    HELPER_PATHS
        .iter()
        .copied()
        .find(|p| std::path::Path::new(p).exists())
}

/// Drive the helper for a single auth attempt. Blocking.
/// Returns `true` if the helper reported SUCCESS (and therefore already
/// invoked `AuthenticationAgentResponse` on polkitd).
pub fn authenticate_via_helper(username: &str, cookie: &str, password: &Zeroizing<String>) -> bool {
    let Some(helper) = find_helper() else {
        tracing::error!("polkit-agent-helper-1 not found on system");
        return false;
    };

    let mut child = match Command::new(helper)
        .arg(username)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(helper, error = %e, "spawn polkit-agent-helper-1 failed");
            return false;
        }
    };

    // Send the cookie + newline; the helper waits for this before
    // starting PAM.
    {
        let stdin = match child.stdin.as_mut() {
            Some(s) => s,
            None => {
                tracing::error!("polkit-agent-helper-1 stdin not available");
                let _ = child.kill();
                return false;
            }
        };
        if let Err(e) = writeln!(stdin, "{cookie}") {
            tracing::error!(error = %e, "write cookie to helper failed");
            let _ = child.kill();
            return false;
        }
    }

    // Loop over the helper's PAM proxy lines.
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            tracing::error!("polkit-agent-helper-1 stdout not available");
            let _ = child.kill();
            return false;
        }
    };
    let mut stdin = child.stdin.take().expect("piped stdin");
    let (line_tx, line_rx) = mpsc::channel::<Result<Option<String>, String>>();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = line_tx.send(Ok(None));
                    break;
                }
                Ok(_) => {
                    if line_tx.send(Ok(Some(line))).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    let _ = line_tx.send(Err(err.to_string()));
                    break;
                }
            }
        }
    });
    let mut password_sent = false;

    loop {
        let line = match line_rx.recv_timeout(HELPER_IDLE_TIMEOUT) {
            Ok(Ok(Some(line))) => line,
            Ok(Ok(None)) => {
                tracing::warn!("polkit-agent-helper-1 closed stdout without SUCCESS/FAILURE");
                break;
            }
            Ok(Err(err)) => {
                tracing::error!(error = %err, "read from helper failed");
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                tracing::warn!(
                    timeout_secs = HELPER_IDLE_TIMEOUT.as_secs(),
                    "polkit-agent-helper-1 timed out waiting for PAM conversation"
                );
                break;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                tracing::warn!("polkit-agent-helper-1 reader thread stopped");
                break;
            }
        };
        let trimmed = line.trim_end_matches(|c| c == char::from(13) || c == char::from(10));
        tracing::trace!(line = %trimmed, "helper >");

        if let Some(_prompt) = trimmed.strip_prefix("PAM_PROMPT_ECHO_OFF ") {
            if password_sent {
                tracing::debug!("helper asked for a second ECHO_OFF; sending empty");
                let _ = writeln!(stdin);
            } else {
                if let Err(e) = writeln!(stdin, "{}", password.as_str()) {
                    tracing::error!(error = %e, "write password to helper failed");
                    break;
                }
                password_sent = true;
            }
        } else if let Some(_prompt) = trimmed.strip_prefix("PAM_PROMPT_ECHO_ON ") {
            tracing::debug!(prompt = %_prompt, "helper requested ECHO_ON input; sending empty");
            let _ = writeln!(stdin);
        } else if let Some(msg) = trimmed.strip_prefix("PAM_TEXT_INFO ") {
            tracing::debug!(msg = %msg, "PAM info");
        } else if let Some(msg) = trimmed.strip_prefix("PAM_ERROR_MSG ") {
            tracing::debug!(msg = %msg, "PAM error message");
        } else if trimmed == "SUCCESS" {
            let _ = child.wait();
            return true;
        } else if trimmed == "FAILURE" {
            let _ = child.wait();
            return false;
        } else if !trimmed.is_empty() {
            tracing::warn!(line = %trimmed, "unexpected helper line");
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_candidates_include_openbsd_package_path() {
        assert_eq!(
            HELPER_PATHS.first().copied(),
            Some("/usr/local/lib/polkit-1/polkit-agent-helper-1")
        );
    }
}
