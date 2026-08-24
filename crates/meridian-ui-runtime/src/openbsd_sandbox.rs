use std::{
    ffi::{c_char, c_int, CString},
    io, ptr,
};

unsafe extern "C" {
    #[link_name = "pledge"]
    fn openbsd_pledge(promises: *const c_char, execpromises: *const c_char) -> c_int;
    #[link_name = "unveil"]
    fn openbsd_unveil(path: *const c_char, permissions: *const c_char) -> c_int;
}

pub(crate) fn install() -> io::Result<()> {
    unveil("/tmp", "rwc")?;
    for path in [
        "/usr/local/share/icons",
        "/usr/local/share/mime",
        "/usr/local/share/pixmaps",
    ] {
        unveil(path, "r")?;
    }
    if unsafe { openbsd_unveil(ptr::null(), ptr::null()) } == -1 {
        let error = io::Error::last_os_error();
        return Err(io::Error::new(
            error.kind(),
            format!("lock unveil view: {error}"),
        ));
    }

    pledge("stdio rpath wpath cpath unix sendfd recvfd")
}

fn unveil(path: &str, permissions: &str) -> io::Result<()> {
    let path = CString::new(path).expect("static unveil path contains no NUL");
    let permissions = CString::new(permissions).expect("static unveil permissions contain no NUL");
    if unsafe { openbsd_unveil(path.as_ptr(), permissions.as_ptr()) } == -1 {
        let error = io::Error::last_os_error();
        return Err(io::Error::new(
            error.kind(),
            format!(
                "unveil {} as {}: {error}",
                path.to_string_lossy(),
                permissions.to_string_lossy()
            ),
        ));
    }
    Ok(())
}

fn pledge(promises: &str) -> io::Result<()> {
    let promises = CString::new(promises).expect("static pledge promises contain no NUL");
    if unsafe { openbsd_pledge(promises.as_ptr(), ptr::null()) } == -1 {
        let error = io::Error::last_os_error();
        return Err(io::Error::new(
            error.kind(),
            format!("pledge {}: {error}", promises.to_string_lossy()),
        ));
    }
    Ok(())
}
