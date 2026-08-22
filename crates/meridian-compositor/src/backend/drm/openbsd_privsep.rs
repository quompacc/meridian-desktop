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
    os::fd::{BorrowedFd, IntoRawFd, OwnedFd},
    os::unix::fs::MetadataExt,
    path::Path,
    path::PathBuf,
};

use smithay::backend::session::{libseat::LibSeatSession, Session};
use smithay::reexports::rustix::fs::{major, minor, Dev, OFlags};
use tracing::{debug, warn};

thread_local! {
    static DRM_SESSION: RefCell<Option<LibSeatSession>> = const { RefCell::new(None) };
}

pub(super) fn install(session: LibSeatSession) {
    DRM_SESSION.with(|slot| slot.replace(Some(session)));
}

pub(crate) fn with_xwayland_device_fds<T>(
    primary_fd: BorrowedFd<'_>,
    kms_path: &Path,
    run: impl FnOnce(&OwnedFd, &OwnedFd) -> T,
) -> Result<T, String> {
    let render_path = matching_render_node_path(kms_path)?;

    DRM_SESSION.with(|slot| {
        let mut slot = slot
            .try_borrow_mut()
            .map_err(|_| "DRM session is already borrowed".to_string())?;
        let session = slot
            .as_mut()
            .ok_or_else(|| "DRM session is not installed".to_string())?;
        let flags = OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK;
        let primary = primary_fd
            .try_clone_to_owned()
            .map_err(|err| format!("could not duplicate active KMS descriptor: {err}"))?;
        let render = match session.open(&render_path, flags) {
            Ok(fd) => fd,
            Err(err) => {
                return Err(format!(
                    "seatd could not open {}: {err}",
                    render_path.display()
                ));
            }
        };

        let result = run(&primary, &render);
        if let Err(err) = session.close(render) {
            warn!(%err, "seatd could not close parent Xwayland render descriptor");
        }
        drop(primary);
        Ok(result)
    })
}

fn matching_render_node_path(kms_path: &Path) -> Result<PathBuf, String> {
    let card_name = kms_path
        .parent()
        .filter(|parent| *parent == Path::new("/dev/dri"))
        .and_then(|_| kms_path.file_name())
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("unsupported OpenBSD KMS node: {}", kms_path.display()))?;
    let render_name = render_node_name(card_name)
        .ok_or_else(|| format!("unsupported OpenBSD KMS node: {}", kms_path.display()))?;
    let render_path = Path::new("/dev/dri").join(render_name);

    let primary_meta = std::fs::metadata(kms_path)
        .map_err(|err| format!("could not stat {}: {err}", kms_path.display()))?;
    let render_meta = std::fs::metadata(&render_path)
        .map_err(|err| format!("could not stat {}: {err}", render_path.display()))?;
    let primary_dev = Dev::try_from(primary_meta.rdev())
        .map_err(|_| "primary DRM device id does not fit dev_t".to_string())?;
    let render_dev = Dev::try_from(render_meta.rdev())
        .map_err(|_| "render DRM device id does not fit dev_t".to_string())?;
    if major(primary_dev) != major(render_dev)
        || minor(primary_dev).checked_add(128) != Some(minor(render_dev))
    {
        return Err(format!(
            "{} does not match active KMS device {}",
            render_path.display(),
            kms_path.display()
        ));
    }
    Ok(render_path)
}

fn render_node_name(card_name: &str) -> Option<String> {
    let card_index = card_name
        .strip_prefix("card")
        .filter(|suffix| !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|suffix| suffix.parse::<u32>().ok())?;
    Some(format!("renderD{}", 128u32.checked_add(card_index)?))
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
    use super::{is_allowed_drm_path, render_node_name};
    use std::path::Path;

    #[test]
    fn allows_only_numbered_drm_nodes() {
        assert!(is_allowed_drm_path(Path::new("/dev/dri/card0")));
        assert!(is_allowed_drm_path(Path::new("/dev/dri/renderD128")));
        assert!(!is_allowed_drm_path(Path::new("/dev/dri/card")));
        assert!(!is_allowed_drm_path(Path::new("/dev/dri/card0/other")));
        assert!(!is_allowed_drm_path(Path::new("/dev/wskbd0")));
    }

    #[test]
    fn maps_openbsd_card_index_to_render_minor_range() {
        assert_eq!(render_node_name("card0").as_deref(), Some("renderD128"));
        assert_eq!(render_node_name("card3").as_deref(), Some("renderD131"));
        assert_eq!(render_node_name("card"), None);
        assert_eq!(render_node_name("renderD128"), None);
    }
}
