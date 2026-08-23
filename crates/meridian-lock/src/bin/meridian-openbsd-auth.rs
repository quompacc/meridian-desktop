#[cfg(target_os = "openbsd")]
fn main() -> std::process::ExitCode {
    use std::{
        ffi::{c_char, c_int, CStr, CString},
        io::{self, Write},
        ptr,
    };

    use zeroize::Zeroizing;

    use meridian_lock::auth_protocol;

    unsafe extern "C" {
        fn auth_userokay(
            name: *mut c_char,
            style: *mut c_char,
            auth_type: *mut c_char,
            password: *mut c_char,
        ) -> c_int;
    }

    if std::env::args_os().len() != 1 {
        return std::process::ExitCode::FAILURE;
    }

    let password = match auth_protocol::read_password(io::stdin().lock()) {
        Ok(password) => Zeroizing::new(password),
        Err(_) => return std::process::ExitCode::FAILURE,
    };
    let Ok(password) = CString::new(password.as_slice()) else {
        return std::process::ExitCode::FAILURE;
    };

    if let Err(error) = meridian_lock::openbsd_sandbox::auth_helper() {
        eprintln!("meridian-openbsd-auth: failed to install sandbox: {error}");
        return std::process::ExitCode::FAILURE;
    }

    // SAFETY: getpwuid returns process-owned account data. Copy the login name
    // before invoking BSD Authentication. auth_userokay receives writable,
    // owned C strings and wipes the password buffer before returning.
    let authenticated = unsafe {
        let account = libc::getpwuid(libc::getuid());
        if account.is_null() || (*account).pw_name.is_null() {
            false
        } else {
            let Ok(user) = CString::new(CStr::from_ptr((*account).pw_name).to_bytes()) else {
                return std::process::ExitCode::FAILURE;
            };
            let mut user = user.into_bytes_with_nul();
            let mut password = password.into_bytes_with_nul();
            auth_userokay(
                user.as_mut_ptr().cast(),
                ptr::null_mut(),
                ptr::null_mut(),
                password.as_mut_ptr().cast(),
            ) != 0
        }
    };

    if io::stdout()
        .lock()
        .write_all(&[u8::from(authenticated)])
        .is_err()
    {
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(not(target_os = "openbsd"))]
fn main() -> std::process::ExitCode {
    eprintln!("meridian-openbsd-auth is available only on OpenBSD");
    std::process::ExitCode::FAILURE
}
