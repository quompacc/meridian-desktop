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
        .filter(|value| !value.trim().is_empty())
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
        return candidate.is_file();
    }

    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };

    std::env::split_paths(&path).any(|dir| dir.join(command).is_file())
}

#[cfg(test)]
mod tests {
    use super::prepare_launch;

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
}
