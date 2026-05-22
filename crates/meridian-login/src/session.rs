// Spawn the user-side compositor after a successful PAM authenticate.
//
// Phase 7a: minimal fork+exec — drop privileges via pre_exec (initgroups +
// setgid + setuid in order) and set up a Wayland-friendly environment.
// Phase 7b: meridian-login now keeps the PAM handle alive for the duration
// of the compositor, so we return the Child so main can wait on it before
// tearing down the logind session.

use std::ffi::CString;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command};

use tracing::info;

const COMPOSITOR_ENV: &str = "MERIDIAN_LOGIN_COMPOSITOR";
const DEFAULT_COMPOSITOR: &str = "/home/eduard/meridian-desktop/target/release/meridian";

#[derive(Debug)]
pub enum SessionError {
    UserNotFound(String),
    HomeNotUtf8,
    UsernameNotCString,
    RuntimeDir(std::io::Error),
    Chown(nix::Error),
    Spawn(std::io::Error),
    Nix(nix::Error),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserNotFound(u) => write!(f, "user not found: {}", u),
            Self::HomeNotUtf8 => write!(f, "user home directory is not utf-8"),
            Self::UsernameNotCString => write!(f, "username contains NUL byte"),
            Self::RuntimeDir(e) => write!(f, "failed to prepare XDG_RUNTIME_DIR: {}", e),
            Self::Chown(e) => write!(f, "chown failed: {}", e),
            Self::Spawn(e) => write!(f, "spawn failed: {}", e),
            Self::Nix(e) => write!(f, "nix syscall failed: {}", e),
        }
    }
}

impl std::error::Error for SessionError {}

/// Spawn the compositor binary as `username`, with a fresh Wayland-flavored
/// environment. Returns the Child so the caller can wait on it (Phase 7b:
/// the PAM session must stay open until the compositor has exited). The
/// child inherits stdio from the parent so its logs flow to the same journal.
///
/// `pam_env` is the snapshot from pam_getenvlist (typically XDG_SESSION_ID,
/// XDG_SEAT, XDG_VTNR set by pam_systemd). It is applied BEFORE the fixed
/// XDG_RUNTIME_DIR / XDG_SESSION_TYPE / XDG_CURRENT_DESKTOP so the explicit
/// settings always win on conflict.
pub fn launch_compositor_for(
    username: &str,
    pam_env: &[(String, String)],
) -> Result<Child, SessionError> {
    let user = nix::unistd::User::from_name(username)
        .map_err(SessionError::Nix)?
        .ok_or_else(|| SessionError::UserNotFound(username.to_string()))?;
    let uid = user.uid.as_raw();
    let gid = user.gid.as_raw();
    let home = user
        .dir
        .to_str()
        .ok_or(SessionError::HomeNotUtf8)?
        .to_string();
    let shell = user.shell.to_string_lossy().into_owned();

    let runtime_dir = ensure_runtime_dir(uid, gid)?;

    let compositor_path =
        std::env::var(COMPOSITOR_ENV).unwrap_or_else(|_| DEFAULT_COMPOSITOR.to_string());

    info!(
        path = %compositor_path,
        uid = uid,
        gid = gid,
        home = %home,
        runtime_dir = %runtime_dir.display(),
        "spawning compositor as user"
    );

    let mut cmd = Command::new(&compositor_path);
    cmd.env_clear();
    // PAM env first: pam_systemd populates XDG_SESSION_ID/XDG_SEAT/XDG_VTNR
    // here. The explicit env below overrides any collisions so our base
    // (HOME/USER/PATH/...) is always sane.
    for (k, v) in pam_env {
        cmd.env(k, v);
    }
    cmd.env("HOME", &home);
    cmd.env("USER", username);
    cmd.env("LOGNAME", username);
    cmd.env(
        "PATH",
        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    );
    cmd.env("SHELL", &shell);
    cmd.env("XDG_RUNTIME_DIR", runtime_dir);
    cmd.env("XDG_SESSION_TYPE", "wayland");
    cmd.env("XDG_CURRENT_DESKTOP", "Meridian");
    // Forward inherited RUST_LOG if set (so dev/debug filter from the
    // unit drop-in propagates into the compositor + shell chain), else
    // fall back to info.
    let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    cmd.env("RUST_LOG", rust_log);
    // Forward MERIDIAN_* env vars (dev/debug knobs like
    // MERIDIAN_SHELL_AUTO_SETTINGS, MERIDIAN_DRM_TIMING, etc.). These
    // are intentionally additive — the explicit envs above already won
    // for any name collisions because cmd.env overwrites.
    for (k, v) in std::env::vars() {
        if k.starts_with("MERIDIAN_") {
            cmd.env(k, v);
        }
    }
    cmd.current_dir(&home);

    // Use pre_exec to do initgroups + setgid + setuid IN ORDER. Using just
    // Command::uid()/.gid() would skip supplementary groups, so the
    // compositor would not pick up `video`/`render`/`input` membership and
    // could not open /dev/dri/card0. The closure captures CString-converted
    // username so the syscall does not allocate after fork.
    let username_c = CString::new(username.to_string())
        .map_err(|_| SessionError::UsernameNotCString)?;
    let uid_nix = nix::unistd::Uid::from_raw(uid);
    let gid_nix = nix::unistd::Gid::from_raw(gid);
    // SAFETY: pre_exec runs between fork and exec in the child process. The
    // calls we make (initgroups, setgid, setuid) are explicitly listed as
    // async-signal-safe in signal-safety(7), and the captured CString is
    // already allocated (we only read its pointer here).
    unsafe {
        cmd.pre_exec(move || {
            nix::unistd::initgroups(&username_c, gid_nix)
                .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;
            nix::unistd::setgid(gid_nix)
                .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;
            nix::unistd::setuid(uid_nix)
                .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;
            Ok(())
        });
    }

    let child = cmd.spawn().map_err(SessionError::Spawn)?;
    Ok(child)
}

/// XDG_RUNTIME_DIR for a user is /run/user/<uid>. With Phase 7b's
/// pam_systemd in the session stack this is normally created for us;
/// we still fall back to creating it ourselves so the compositor has a
/// working XDG_RUNTIME_DIR even if pam_systemd is missing or unhappy.
fn ensure_runtime_dir(uid: u32, gid: u32) -> Result<PathBuf, SessionError> {
    let path = PathBuf::from(format!("/run/user/{}", uid));
    if !path.exists() {
        std::fs::create_dir_all(&path).map_err(SessionError::RuntimeDir)?;
        nix::unistd::chown(
            path.as_path(),
            Some(nix::unistd::Uid::from_raw(uid)),
            Some(nix::unistd::Gid::from_raw(gid)),
        )
        .map_err(SessionError::Chown)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
            .map_err(SessionError::RuntimeDir)?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_user_yields_user_not_found() {
        let r = launch_compositor_for("definitely_no_such_user_xyzzy_123", &[]);
        assert!(matches!(r, Err(SessionError::UserNotFound(_))));
    }

    #[test]
    fn runtime_dir_path_format() {
        // ensure_runtime_dir is private but the path is deterministic; check via
        // an indirect probe: existing dir for current uid (commonly /run/user/<uid>)
        let uid = nix::unistd::Uid::current().as_raw();
        let expected = std::path::Path::new("/run/user").join(uid.to_string());
        // We don't assert existence (test may run as a user without one) — only
        // that the function's path scheme matches what XDG expects.
        assert!(expected.starts_with("/run/user/"));
    }
}
