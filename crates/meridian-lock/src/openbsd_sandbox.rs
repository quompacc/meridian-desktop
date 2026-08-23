use std::{ffi::CString, io, ptr};

const AUTH_HELPER: &str = "/usr/local/libexec/meridian-openbsd-auth";
const AUTH_PROGRAM: &str = "/usr/libexec/auth/login_passwd";

/// Restrict the Wayland lock UI after configuration, theme, account identity
/// and its compositor connection have already been resolved.
pub fn lock_ui() -> io::Result<()> {
    unveil(AUTH_HELPER, "x")?;
    // Command::spawn opens this for the authentication helper's suppressed
    // standard error stream.
    unveil("/dev/null", "w")?;
    // OpenBSD implements shm_mkstemp(3) with a randomized backing file below
    // /tmp. The lock client needs this for its Wayland framebuffer.
    unveil("/tmp", "rwc")?;
    unveil_runtime_loader()?;
    lock_unveil()?;

    // rpath/wpath/cpath are retained only for shm_mkstemp(3). proc+exec are
    // required for the fixed authentication helper; no execpromises are set
    // because OpenBSD rejects executing a setgid program under execpromises.
    pledge(
        "stdio rpath wpath cpath proc exec sendfd recvfd",
        ptr::null(),
    )
}

/// Restrict the single-request setgid helper before it enters BSD
/// Authentication. This profile deliberately supports only the reference
/// machine's password authentication program.
pub fn auth_helper() -> io::Result<()> {
    unveil("/etc/login.conf", "r")?;
    unveil("/etc/login.conf.d", "r")?;
    unveil(AUTH_PROGRAM, "x")?;
    unveil_runtime_loader()?;
    lock_unveil()?;

    // getpw permits libc's protected account-database lookups. rpath covers
    // login.conf; proc+exec cover auth_userokay(3)'s login_passwd child.
    pledge("stdio rpath getpw proc exec", ptr::null())
}

fn unveil_runtime_loader() -> io::Result<()> {
    unveil("/usr/libexec/ld.so", "rx")?;
    unveil("/var/run/ld.so.hints", "r")?;
    unveil("/usr/lib", "r")
}

fn unveil(path: &str, permissions: &str) -> io::Result<()> {
    let path = CString::new(path).expect("static unveil path contains no NUL");
    let permissions = CString::new(permissions).expect("static unveil permissions contain no NUL");
    if unsafe { libc::unveil(path.as_ptr(), permissions.as_ptr()) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn lock_unveil() -> io::Result<()> {
    if unsafe { libc::unveil(ptr::null(), ptr::null()) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn pledge(promises: &str, execpromises: *const libc::c_char) -> io::Result<()> {
    let promises = CString::new(promises).expect("static pledge promises contain no NUL");
    if unsafe { libc::pledge(promises.as_ptr(), execpromises) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
