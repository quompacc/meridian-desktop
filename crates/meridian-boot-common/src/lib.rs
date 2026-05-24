use std::{
    fs, io,
    os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt},
    path::Path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketIdentity {
    dev: u64,
    ino: u64,
}

pub fn secure_socket_permissions(path: &Path) -> io::Result<SocketIdentity> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    socket_identity_for_path(path)
}

pub fn socket_identity_for_path(path: &Path) -> io::Result<SocketIdentity> {
    let metadata = fs::metadata(path)?;
    Ok(SocketIdentity {
        dev: metadata.dev(),
        ino: metadata.ino(),
    })
}

pub fn cleanup_socket_path(path: &Path, expected: SocketIdentity) -> io::Result<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err),
    };

    if !metadata.file_type().is_socket() {
        return Ok(false);
    }

    let actual = SocketIdentity {
        dev: metadata.dev(),
        ino: metadata.ino(),
    };
    if actual != expected {
        return Ok(false);
    }

    fs::remove_file(path)?;
    Ok(true)
}

// Prefer a sensible mid-range mode during boot: largest where the longer side
// stays <= 2560 px. This keeps bootsplash and meridian-login visually aligned.
pub fn select_boot_mode(modes: &[drm::control::Mode]) -> Option<drm::control::Mode> {
    let mut filtered: Vec<_> = modes
        .iter()
        .copied()
        .filter(|m| {
            let (w, h) = m.size();
            w.max(h) <= 2560 && w >= 1280 && h >= 720
        })
        .collect();
    filtered.sort_by_key(|m| {
        let (w, h) = m.size();
        std::cmp::Reverse(w as u32 * h as u32)
    });
    filtered.first().copied().or_else(|| modes.first().copied())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        os::unix::net::UnixListener,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{cleanup_socket_path, secure_socket_permissions};

    fn unique_test_dir(prefix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "{}-{}-{}",
            prefix,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    #[test]
    fn cleanup_removes_original_socket() {
        let dir = unique_test_dir("meridian-boot-common-cleanup");
        fs::create_dir_all(&dir).expect("test dir");
        let path = dir.join("boot.sock");
        let listener = UnixListener::bind(&path).expect("bind socket");
        let identity = secure_socket_permissions(&path).expect("socket identity");

        assert!(cleanup_socket_path(&path, identity).expect("cleanup"));
        assert!(!path.exists());

        drop(listener);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cleanup_rejects_replaced_non_socket() {
        let dir = unique_test_dir("meridian-boot-common-replaced");
        fs::create_dir_all(&dir).expect("test dir");
        let path = dir.join("boot.sock");
        let listener = UnixListener::bind(&path).expect("bind socket");
        let identity = secure_socket_permissions(&path).expect("socket identity");
        drop(listener);
        fs::remove_file(&path).expect("remove socket");
        fs::write(&path, b"not a socket").expect("write replacement");

        assert!(!cleanup_socket_path(&path, identity).expect("cleanup"));
        assert!(path.exists());

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(dir);
    }
}
