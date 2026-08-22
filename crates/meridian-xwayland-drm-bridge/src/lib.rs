//! OpenBSD-only DRM descriptor bridge for the unprivileged Xwayland child.
//!
//! OpenBSD libdrm calls the weak `priv_open_device` symbol when direct access
//! to a DRM node is unavailable. Meridian pre-opens exactly the active GPU's
//! primary and render nodes through seatd and passes their descriptor numbers
//! in the environment. For DRI3 clients, the authorized descriptor already
//! received from Xwayland is reused. This library only duplicates a descriptor
//! when the requested canonical device path and `st_rdev` both match.

#[cfg(target_os = "openbsd")]
mod openbsd {
    use std::ffi::{c_char, c_int, CStr};

    const PRIMARY_FD_ENV: &[u8] = b"MERIDIAN_XWAYLAND_DRM_PRIMARY_FD\0";
    const RENDER_FD_ENV: &[u8] = b"MERIDIAN_XWAYLAND_DRM_RENDER_FD\0";

    fn allowed_path(path: &[u8]) -> bool {
        let suffix = path
            .strip_prefix(b"/dev/dri/card")
            .or_else(|| path.strip_prefix(b"/dev/dri/renderD"));
        suffix.is_some_and(|value| {
            !value.is_empty() && value.iter().all(|byte| byte.is_ascii_digit())
        })
    }

    unsafe fn env_fd(name: &[u8]) -> Option<c_int> {
        // SAFETY: callers pass static NUL-terminated environment names.
        let value = unsafe { libc::getenv(name.as_ptr().cast()) };
        if value.is_null() {
            return None;
        }
        // SAFETY: getenv returns a NUL-terminated value owned by the process.
        let bytes = unsafe { CStr::from_ptr(value) }.to_bytes();
        if bytes.is_empty() || !bytes.iter().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        std::str::from_utf8(bytes).ok()?.parse().ok()
    }

    unsafe fn requested_rdev(requested: &CStr) -> Option<libc::dev_t> {
        let mut requested_stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: requested is NUL-terminated and the output points to writable
        // stat storage.
        if unsafe { libc::stat(requested.as_ptr(), requested_stat.as_mut_ptr()) } != 0 {
            return None;
        }
        // SAFETY: successful stat initialized the value.
        Some(unsafe { requested_stat.assume_init() }.st_rdev)
    }

    unsafe fn duplicate_if_matching(fd: c_int, requested_rdev: libc::dev_t) -> Option<c_int> {
        let mut inherited_stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: the pointer refers to writable stat storage.
        if unsafe { libc::fstat(fd, inherited_stat.as_mut_ptr()) } != 0 {
            return None;
        }
        // SAFETY: successful fstat initialized the value.
        let inherited_stat = unsafe { inherited_stat.assume_init() };
        if requested_rdev != inherited_stat.st_rdev {
            return None;
        }
        // libdrm owns and closes the returned descriptor.
        let duplicated = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
        (duplicated >= 0).then_some(duplicated)
    }

    unsafe fn duplicate_inherited_device(requested: &CStr) -> Option<c_int> {
        if !allowed_path(requested.to_bytes()) {
            return None;
        }
        let requested_rdev = unsafe { requested_rdev(requested) }?;
        for name in [PRIMARY_FD_ENV, RENDER_FD_ENV] {
            // SAFETY: environment names are static and NUL-terminated; parsed
            // descriptors are validated with fstat before duplication.
            if let Some(fd) = unsafe { env_fd(name) } {
                if let Some(duplicated) = unsafe { duplicate_if_matching(fd, requested_rdev) } {
                    return Some(duplicated);
                }
            }
        }

        // DRI3 passes an already-authorized render descriptor from Xwayland to
        // the client. OpenBSD libdrm may subsequently ask priv_open_device for
        // the same node while resolving device metadata. Reuse only a matching
        // descriptor that is already present in this process; this grants no
        // access beyond what DRI3 has supplied.
        let descriptor_limit = unsafe { libc::getdtablesize() };
        for fd in 0..descriptor_limit {
            if let Some(duplicated) = unsafe { duplicate_if_matching(fd, requested_rdev) } {
                return Some(duplicated);
            }
        }
        None
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn priv_open_device(path: *const c_char) -> c_int {
        if path.is_null() {
            return -1;
        }
        // SAFETY: libdrm supplies a non-null NUL-terminated path.
        let requested = unsafe { CStr::from_ptr(path) };
        // SAFETY: requested remains valid for the duration of the lookup.
        unsafe { duplicate_inherited_device(requested) }.unwrap_or(-1)
    }

    /// Mesa's Wayland-EGL path opens the render node directly instead of using
    /// libdrm's weak hook. Interpose only `open`; all non-matching paths are
    /// forwarded to the non-interposed `openat` libc entry point.
    #[cfg(not(test))]
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn open(path: *const c_char, flags: c_int, mode: libc::mode_t) -> c_int {
        if !path.is_null() {
            // SAFETY: libc callers supply a NUL-terminated path.
            let requested = unsafe { CStr::from_ptr(path) };
            // SAFETY: descriptor candidates are verified against the requested
            // device's st_rdev before duplication.
            if let Some(fd) = unsafe { duplicate_inherited_device(requested) } {
                return fd;
            }
        }

        // open(2) only supplies a meaningful third argument with O_CREAT. Do
        // not read the ABI placeholder in the common two-argument case.
        if flags & libc::O_CREAT != 0 {
            // SAFETY: this preserves libc open semantics through openat.
            unsafe { libc::openat(libc::AT_FDCWD, path, flags, mode) }
        } else {
            // SAFETY: openat is variadic and accepts the two-argument form when
            // O_CREAT is absent.
            unsafe { libc::openat(libc::AT_FDCWD, path, flags) }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::allowed_path;

        #[test]
        fn accepts_only_numbered_canonical_drm_nodes() {
            assert!(allowed_path(b"/dev/dri/card0"));
            assert!(allowed_path(b"/dev/dri/renderD128"));
            assert!(!allowed_path(b"/dev/dri/card"));
            assert!(!allowed_path(b"/dev/dri/card0/other"));
            assert!(!allowed_path(b"/dev/wskbd0"));
        }
    }
}
