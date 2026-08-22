use std::{path::Path, process::Command};

pub(super) struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
}

pub(super) fn prepare_launch(program: &str, args: &[String], terminal: bool) -> Option<LaunchSpec> {
    if program.trim().is_empty() {
        return None;
    }

    if !terminal {
        return Some(LaunchSpec {
            program: program.to_string(),
            args: args.to_vec(),
        });
    }

    let terminal_program = std::env::var("TERMINAL")
        .ok()
        .filter(|value| command_exists(value))
        .or_else(|| {
            [
                "foot",
                "alacritty",
                "kitty",
                "wezterm",
                "ghostty",
                "kgx",
                "konsole",
                "xterm",
            ]
            .into_iter()
            .find(|candidate| command_exists(candidate))
            .map(str::to_string)
        })?;

    let mut terminal_args = Vec::with_capacity(args.len() + 2);
    terminal_args.push("-e".to_string());
    terminal_args.push(program.to_string());
    terminal_args.extend(args.iter().cloned());

    Some(LaunchSpec {
        program: terminal_program,
        args: terminal_args,
    })
}

pub(super) fn spawn_and_reap(mut launch: Command, program: &str, args: &[String]) {
    let mut child = match launch.spawn() {
        Ok(child) => child,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                tracing::warn!("failed to launch app: program not found: {:?}", program);
            } else {
                tracing::warn!(
                    "failed to launch app program {:?} args {:?}: {}",
                    program,
                    args,
                    err
                );
            }
            return;
        }
    };

    let program = program.to_string();
    let args = args.to_vec();
    if let Err(err) = std::thread::Builder::new()
        .name(format!("meridian-launch-reaper-{program}"))
        .spawn(move || match child.wait() {
            Ok(status) if !status.success() => {
                tracing::warn!(
                    "launched app exited unsuccessfully: program={:?} args={:?} status={}",
                    program,
                    args,
                    status
                );
            }
            Ok(_) => {}
            Err(err) => tracing::warn!("failed to reap launched app {:?}: {}", program, err),
        })
    {
        tracing::warn!("failed to spawn launch reaper thread: {}", err);
    }
}

pub(super) fn apply_launch_environment(launch: &mut Command, program: &str) {
    // OpenBSD's current Qt 6 Wayland path cannot create the OpenGL/QRhi
    // contexts required by applications such as FreeCAD, while the same
    // packaged applications work through the already-supported XWayland GLX
    // path. Keep this platform boundary in one launch adapter and respect an
    // explicit user/session override.
    #[cfg(target_os = "openbsd")]
    if std::env::var_os("QT_QPA_PLATFORM").is_none() {
        launch.env("QT_QPA_PLATFORM", "xcb");
    }

    // Thunar's GTK3 client-side decoration does not match Meridian's window
    // chrome and does not negotiate xdg-decoration when forced CSD-off on its
    // native Wayland backend. Run only Thunar through the existing XWayland
    // compatibility path so GTK yields the frame to Meridian SSD. Other GTK
    // applications remain native Wayland clients.
    if is_thunar_program(program) {
        launch.env("GDK_BACKEND", "x11").env("GTK_CSD", "0");
    }
}

fn is_thunar_program(program: &str) -> bool {
    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("thunar"))
}

pub(super) fn is_firefox_program(program: &str) -> bool {
    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("firefox") || name.eq_ignore_ascii_case("firefox-esr")
        })
}

fn command_exists(command: &str) -> bool {
    if command.trim().is_empty() {
        return false;
    }
    let candidate = Path::new(command);
    if candidate.is_absolute() {
        return is_executable_file(candidate);
    }

    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };

    std::env::split_paths(&path).any(|dir| is_executable_file(&dir.join(command)))
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, OpenOptions},
        io::Write,
        os::unix::fs::PermissionsExt,
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[cfg(target_os = "openbsd")]
    use super::apply_launch_environment;
    use super::prepare_launch;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_env_vars<R>(vars: &[(&str, Option<&str>)], f: impl FnOnce() -> R) -> R {
        let _guard = env_lock().lock().expect("env lock");
        let previous: Vec<(&str, Option<String>)> = vars
            .iter()
            .map(|(key, _)| (*key, std::env::var(key).ok()))
            .collect();

        for (key, value) in vars {
            match value {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }

        let result = f();

        for (key, value) in previous {
            if let Some(v) = value {
                std::env::set_var(key, v);
            } else {
                std::env::remove_var(key);
            }
        }

        result
    }

    #[test]
    fn non_terminal_launch_keeps_program_and_args() {
        let args = vec!["--foo".to_string(), "bar".to_string()];
        let spec = prepare_launch("demo", &args, false).expect("launch spec");
        assert_eq!(spec.program, "demo");
        assert_eq!(spec.args, args);
    }

    #[cfg(target_os = "openbsd")]
    #[test]
    fn openbsd_qt_platform_defaults_to_xcb_but_respects_session_override() {
        use std::process::Command;

        with_env_vars(&[("QT_QPA_PLATFORM", None)], || {
            let mut launch = Command::new("true");
            apply_launch_environment(&mut launch, "FreeCAD");
            assert_eq!(
                launch
                    .get_envs()
                    .find(|(name, _)| *name == "QT_QPA_PLATFORM")
                    .and_then(|(_, value)| value),
                Some(std::ffi::OsStr::new("xcb"))
            );
        });

        with_env_vars(&[("QT_QPA_PLATFORM", Some("wayland"))], || {
            let mut launch = Command::new("true");
            apply_launch_environment(&mut launch, "FreeCAD");
            assert!(launch.get_envs().all(|(name, _)| name != "QT_QPA_PLATFORM"));
        });
    }

    #[test]
    fn thunar_uses_xwayland_without_client_side_decorations() {
        use std::process::Command;

        let mut thunar = Command::new("thunar");
        apply_launch_environment(&mut thunar, "/usr/local/bin/thunar");
        assert_eq!(
            thunar
                .get_envs()
                .find(|(name, _)| *name == "GDK_BACKEND")
                .and_then(|(_, value)| value),
            Some(std::ffi::OsStr::new("x11"))
        );
        assert_eq!(
            thunar
                .get_envs()
                .find(|(name, _)| *name == "GTK_CSD")
                .and_then(|(_, value)| value),
            Some(std::ffi::OsStr::new("0"))
        );

        let mut other = Command::new("foot");
        apply_launch_environment(&mut other, "foot");
        assert!(other
            .get_envs()
            .all(|(name, _)| name != "GDK_BACKEND" && name != "GTK_CSD"));
    }

    #[test]
    fn empty_program_is_rejected() {
        let spec = prepare_launch(" ", &[], false);
        assert!(spec.is_none());
    }

    #[test]
    fn terminal_env_non_executable_file_is_rejected() {
        let tmpdir = std::env::temp_dir().join(format!(
            "meridian-launch-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&tmpdir).expect("create temp dir");
        let terminal_path = tmpdir.join("terminal-script");
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&terminal_path)
            .expect("create file");
        writeln!(file, "#!/bin/sh").expect("write file");
        file.flush().expect("flush file");
        fs::set_permissions(&terminal_path, fs::Permissions::from_mode(0o644))
            .expect("set permissions");

        let terminal_str = terminal_path.to_string_lossy().to_string();
        let spec = with_env_vars(
            &[("PATH", Some("")), ("TERMINAL", Some(&terminal_str))],
            || prepare_launch("app", &[], true),
        );
        assert!(spec.is_none());
        let _ = fs::remove_file(&terminal_path);
        let _ = fs::remove_dir(&tmpdir);
    }

    #[test]
    fn terminal_env_executable_file_is_accepted() {
        let tmpdir = std::env::temp_dir().join(format!(
            "meridian-launch-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&tmpdir).expect("create temp dir");
        let terminal_path = tmpdir.join("terminal-script");
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&terminal_path)
            .expect("create file");
        writeln!(file, "#!/bin/sh").expect("write file");
        file.flush().expect("flush file");
        fs::set_permissions(&terminal_path, fs::Permissions::from_mode(0o755))
            .expect("set permissions");

        let terminal_str = terminal_path.to_string_lossy().to_string();
        let spec = with_env_vars(
            &[("PATH", Some("")), ("TERMINAL", Some(&terminal_str))],
            || prepare_launch("app", &["--flag".to_string()], true),
        )
        .expect("launch spec");

        assert_eq!(spec.program, terminal_str);
        assert_eq!(spec.args, vec!["-e", "app", "--flag"]);
        let _ = fs::remove_file(&terminal_path);
        let _ = fs::remove_dir(&tmpdir);
    }
}
