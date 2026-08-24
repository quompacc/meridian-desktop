use std::{
    env,
    ffi::CString,
    io,
    path::{Path, PathBuf},
    ptr,
};

use meridian_config::config_directory;

const POLKIT_HELPER: &str = "/usr/local/lib/polkit-1/polkit-agent-helper-1";

/// Restrict the long-lived authentication agent after its system D-Bus
/// registration and Wayland bootstrap have completed.
pub fn install() -> io::Result<()> {
    // Resolve optional paths before the first unveil call hides everything
    // else. Path::exists() after that point would incorrectly report hidden
    // candidates as absent.
    let config_dir = config_directory();
    let config_dir = config_dir.exists().then_some(config_dir);
    let theme_roots = existing_theme_roots();

    unveil(POLKIT_HELPER, "x")?;
    unveil("/dev/null", "w")?;
    // OpenBSD implements shm_mkstemp(3) with backing files below /tmp.
    unveil("/tmp", "rwc")?;

    if let Some(config_dir) = config_dir {
        unveil_path(&config_dir, "r")?;
    }
    for theme_root in theme_roots {
        unveil_path(&theme_root, "r")?;
    }

    // Loader paths are needed while executing the fixed upstream setuid
    // helper. Its protected BSD Authentication and system-bus paths are not
    // accessible to this unprivileged process and belong to that separately
    // audited package boundary, not to the UI agent's filesystem view.
    unveil_runtime_loader()?;
    lock_unveil()?;

    // getpw resolves authorised unix-user identities. proc+exec are limited
    // by unveil to the fixed polkit helper. No execpromises are used because
    // OpenBSD rejects executing a setuid program under execpromises.
    pledge(
        "stdio rpath wpath cpath getpw proc exec sendfd recvfd",
        ptr::null(),
    )
}

fn existing_theme_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let home = env::var_os("HOME");
    if let Some(home) = home.as_ref() {
        let home = Path::new(home);
        push_existing(&mut roots, home.join(".local/share/meridian/themes"));
    }

    if let Some(path) = env::var_os("MERIDIAN_THEME_DIR") {
        push_existing(&mut roots, PathBuf::from(path));
    }
    if let Some(paths) = env::var_os("MERIDIAN_THEME_DIRS") {
        for path in env::split_paths(&paths) {
            push_existing(&mut roots, path);
        }
    }

    let data_dirs = env::var_os("XDG_DATA_DIRS")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    for path in env::split_paths(&data_dirs) {
        push_existing(&mut roots, path.join("meridian/themes"));
    }
    roots
}

fn push_existing(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if path.exists() && !paths.contains(&path) {
        paths.push(path);
    }
}

fn unveil_runtime_loader() -> io::Result<()> {
    unveil("/usr/libexec/ld.so", "rx")?;
    unveil("/var/run/ld.so.hints", "r")?;
    unveil("/usr/lib", "r")?;
    unveil("/usr/local/lib", "r")
}

fn unveil(path: &str, permissions: &str) -> io::Result<()> {
    unveil_path(Path::new(path), permissions)
}

fn unveil_path(path: &Path, permissions: &str) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;

    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "unveil path contains NUL"))?;
    let permissions = CString::new(permissions).expect("static unveil permissions contain no NUL");
    if unsafe { libc::unveil(path.as_ptr(), permissions.as_ptr()) } == -1 {
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

fn lock_unveil() -> io::Result<()> {
    if unsafe { libc::unveil(ptr::null(), ptr::null()) } == -1 {
        let error = io::Error::last_os_error();
        return Err(io::Error::new(
            error.kind(),
            format!("lock unveil view: {error}"),
        ));
    }
    Ok(())
}

fn pledge(promises: &str, execpromises: *const libc::c_char) -> io::Result<()> {
    let promises = CString::new(promises).expect("static pledge promises contain no NUL");
    if unsafe { libc::pledge(promises.as_ptr(), execpromises) } == -1 {
        let error = io::Error::last_os_error();
        return Err(io::Error::new(
            error.kind(),
            format!("pledge {}: {error}", promises.to_string_lossy()),
        ));
    }
    Ok(())
}
