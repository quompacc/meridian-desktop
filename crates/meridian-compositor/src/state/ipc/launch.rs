use std::path::Path;

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

    use super::prepare_launch;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_env_var<R>(key: &str, value: Option<&str>, f: impl FnOnce() -> R) -> R {
        let _guard = env_lock().lock().expect("env lock");
        let previous = std::env::var(key).ok();
        match value {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
        let result = f();
        if let Some(v) = previous {
            std::env::set_var(key, v);
        } else {
            std::env::remove_var(key);
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
        let spec = with_env_var("PATH", Some(""), || {
            with_env_var("TERMINAL", Some(&terminal_str), || {
                prepare_launch("app", &[], true)
            })
        });
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
        let spec = with_env_var("PATH", Some(""), || {
            with_env_var("TERMINAL", Some(&terminal_str), || {
                prepare_launch("app", &["--flag".to_string()], true)
            })
        })
        .expect("launch spec");

        assert_eq!(spec.program, terminal_str);
        assert_eq!(spec.args, vec!["-e", "app", "--flag"]);
        let _ = fs::remove_file(&terminal_path);
        let _ = fs::remove_dir(&tmpdir);
    }
}
