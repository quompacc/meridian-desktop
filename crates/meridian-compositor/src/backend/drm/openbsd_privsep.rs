//! OpenBSD libdrm device-open bridge.
//!
//! OpenBSD's libdrm is built with `X_PRIVSEP` and exposes
//! `priv_open_device` as a weak symbol. Mesa uses that hook when it needs an
//! additional DRM descriptor while probing a GBM device. Route those opens
//! through the compositor's existing seatd session, which is Meridian's
//! privileged device broker.

use std::{
    cell::RefCell,
    ffi::{c_char, c_int, CStr},
    os::fd::IntoRawFd,
    path::Path,
};

use smithay::backend::session::{libseat::LibSeatSession, Session};
use smithay::reexports::rustix::fs::OFlags;
use tracing::{debug, warn};

thread_local! {
    static DRM_SESSION: RefCell<Option<LibSeatSession>> = const { RefCell::new(None) };
}

pub(super) fn install(session: LibSeatSession) {
    DRM_SESSION.with(|slot| slot.replace(Some(session)));
}

fn is_allowed_drm_path(path: &Path) -> bool {
    if path.parent() != Some(Path::new("/dev/dri")) {
        return false;
    }

    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let Some(index) = name
        .strip_prefix("card")
        .or_else(|| name.strip_prefix("renderD"))
    else {
        return false;
    };

    !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit())
}

/// Strong override for OpenBSD libdrm's weak `priv_open_device` symbol.
///
/// Mesa owns the returned descriptor and closes it when it is no longer
/// needed. seatd retains its own bookkeeping for the lifetime of the session,
/// matching the other session-opened DRM descriptors used by this backend.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn priv_open_device(path: *const c_char) -> c_int {
    if path.is_null() {
        return -1;
    }

    // SAFETY: libdrm calls this hook with a non-null, NUL-terminated path.
    let path = unsafe { CStr::from_ptr(path) };
    let Ok(path) = path.to_str() else {
        return -1;
    };
    let path = Path::new(path);
    if !is_allowed_drm_path(path) {
        warn!(path = %path.display(), "rejected libdrm device-open request");
        return -1;
    }

    DRM_SESSION.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return -1;
        };
        let Some(session) = slot.as_mut() else {
            return -1;
        };

        match session.open(
            path,
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
        ) {
            Ok(fd) => fd.into_raw_fd(),
            Err(err) => {
                debug!(path = %path.display(), %err, "seatd could not open probed libdrm device");
                -1
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::is_allowed_drm_path;
    use std::path::Path;

    #[test]
    fn allows_only_numbered_drm_nodes() {
        assert!(is_allowed_drm_path(Path::new("/dev/dri/card0")));
        assert!(is_allowed_drm_path(Path::new("/dev/dri/renderD128")));
        assert!(!is_allowed_drm_path(Path::new("/dev/dri/card")));
        assert!(!is_allowed_drm_path(Path::new("/dev/dri/card0/other")));
        assert!(!is_allowed_drm_path(Path::new("/dev/wskbd0")));
    }
}
