use std::{
    env,
    ffi::{c_char, c_int, CString},
    io,
    path::{Path, PathBuf},
    ptr,
};

use meridian_config::config_directory;
use meridian_ipc::socket_path;

unsafe extern "C" {
    #[link_name = "pledge"]
    fn openbsd_pledge(promises: *const c_char, execpromises: *const c_char) -> c_int;
    #[link_name = "unveil"]
    fn openbsd_unveil(path: *const c_char, permissions: *const c_char) -> c_int;
}

/// Restrict the long-lived portal backend after it owns its session-bus name.
///
/// FileChooser is deliberately not served by this process on OpenBSD. Its GTK
/// backend is a separate process with the broad filesystem view that an
/// interactive file picker necessarily needs.
pub fn install() -> io::Result<()> {
    // Resolve optional paths before the first unveil call hides all other
    // candidates. Missing user overrides remain invisible if created later.
    let config_dir = existing(config_directory());
    let theme_roots = existing_theme_roots();
    let compositor_socket = socket_path();
    require_socket(&compositor_socket)?;

    if let Some(config_dir) = config_dir {
        unveil_path(&config_dir, "r")?;
    }
    for theme_root in theme_roots {
        unveil_path(&theme_root, "r")?;
    }

    // The socket must already exist. This makes activation outside a live
    // Meridian session fail closed instead of leaving a partially useful
    // backend with a silently broken screenshot path.
    unveil_path(&compositor_socket, "rw")?;
    lock_unveil()?;

    // rpath covers config/theme polling. unix covers the established D-Bus
    // connection and per-request compositor socket. Descriptor passing is
    // retained for the D-Bus transport even though current methods use none.
    pledge("stdio rpath unix sendfd recvfd")
}

fn existing_theme_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let home = env::var_os("HOME");
    if let Some(home) = home.as_ref() {
        let home = Path::new(home);
        push_existing(&mut roots, home.join(".config/meridian/themes"));
    }

    if let Some(path) = env::var_os("MERIDIAN_THEME_DIR") {
        push_existing(&mut roots, PathBuf::from(path));
    }
    if let Some(paths) = env::var_os("MERIDIAN_THEME_DIRS") {
        for path in env::split_paths(&paths) {
            push_existing(&mut roots, path);
        }
    }

    if let Some(path) = env::var_os("XDG_DATA_HOME") {
        push_existing(&mut roots, PathBuf::from(path).join("meridian/themes"));
    } else if let Some(home) = home {
        push_existing(
            &mut roots,
            PathBuf::from(home).join(".local/share/meridian/themes"),
        );
    }

    let data_dirs = env::var_os("XDG_DATA_DIRS")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    for path in env::split_paths(&data_dirs) {
        push_existing(&mut roots, path.join("meridian/themes"));
    }
    roots
}

fn existing(path: PathBuf) -> Option<PathBuf> {
    path.exists().then_some(path)
}

fn push_existing(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if path.exists() && !paths.contains(&path) {
        paths.push(path);
    }
}

fn require_socket(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::FileTypeExt;

    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("required compositor socket {}: {error}", path.display()),
        )
    })?;
    if !metadata.file_type().is_socket() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "required compositor socket is not a socket: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn unveil_path(path: &Path, permissions: &str) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;

    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "unveil path contains NUL"))?;
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

fn lock_unveil() -> io::Result<()> {
    if unsafe { openbsd_unveil(ptr::null(), ptr::null()) } == -1 {
        let error = io::Error::last_os_error();
        return Err(io::Error::new(
            error.kind(),
            format!("lock unveil view: {error}"),
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
