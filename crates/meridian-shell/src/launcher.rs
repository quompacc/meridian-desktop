use std::{
    cmp::Ordering,
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use meridian_ipc::ShellCommand;
use tracing::{debug, info, warn};

const XDG_DATA_DIRS_DEFAULT: &str = "/usr/local/share:/usr/share";
const MERIDIAN_DESKTOP_ENV: &str = "Meridian";

#[derive(Debug, Clone)]
pub struct DesktopApp {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    pub terminal: bool,
    pub categories: Vec<String>,
    pub icon_name: Option<String>,
    name_key: String,
    exec_key: String,
}

impl DesktopApp {
    pub fn load_system() -> Vec<Self> {
        Self::load_from_dirs(desktop_app_dirs())
    }

    pub(crate) fn new(name: String, exec_argv: Vec<String>, terminal: bool) -> Self {
        let name = name.trim().to_string();
        let program = exec_argv.first().cloned().unwrap_or_default();
        let args = exec_argv.iter().skip(1).cloned().collect::<Vec<_>>();
        let exec = argv_to_display(&program, &args);
        Self {
            name_key: name.to_lowercase(),
            exec_key: exec.to_lowercase(),
            name,
            program,
            args,
            terminal,
            categories: Vec::new(),
            icon_name: None,
        }
    }

    fn load_from_dirs(dirs: Vec<PathBuf>) -> Vec<Self> {
        let mut apps = Vec::new();
        let mut seen = HashSet::new();

        for dir in dirs {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            let mut paths = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| is_desktop_file(path))
                .collect::<Vec<_>>();
            paths.sort();

            for path in paths {
                let Some(app) = Self::from_file(&path) else {
                    continue;
                };
                if seen.insert((app.name_key.clone(), app.exec_key.clone())) {
                    apps.push(app);
                }
            }
        }

        apps.sort_by(cmp_apps);
        apps
    }

    fn from_file(path: &Path) -> Option<Self> {
        let raw = fs::read_to_string(path).ok()?;
        match Self::from_desktop_entry_str_with_reason(&raw) {
            Ok(app) => Some(app),
            Err(reason) => {
                debug!(path=?path, reason, "launcher ignored desktop entry");
                None
            }
        }
    }

    fn from_desktop_entry_str_with_reason(raw: &str) -> Result<Self, &'static str> {
        let mut in_desktop_entry = false;
        let mut name = None;
        let mut exec_argv = None;
        let mut try_exec = None;
        let mut only_show_in = None;
        let mut not_show_in = None;
        let mut terminal = false;
        let mut hidden = false;
        let mut no_display = false;
        let mut desktop_type = None;
        let mut categories = None;
        let mut icon_name = None;

        for line in raw.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                in_desktop_entry = line == "[Desktop Entry]";
                continue;
            }
            if !in_desktop_entry || line.starts_with('#') || line.is_empty() {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();

            match key {
                "Name" if !value.is_empty() => {
                    name.get_or_insert_with(|| value.to_string());
                }
                "Exec" => {
                    let argv = parse_exec_argv(value);
                    if !argv.is_empty() {
                        exec_argv.get_or_insert(argv);
                    }
                }
                "Terminal" => terminal = value.eq_ignore_ascii_case("true"),
                "Hidden" => hidden = value.eq_ignore_ascii_case("true"),
                "NoDisplay" => no_display = value.eq_ignore_ascii_case("true"),
                "TryExec" if !value.is_empty() => {
                    try_exec.get_or_insert_with(|| value.to_string());
                }
                "OnlyShowIn" if !value.is_empty() => {
                    only_show_in.get_or_insert_with(|| value.to_string());
                }
                "NotShowIn" if !value.is_empty() => {
                    not_show_in.get_or_insert_with(|| value.to_string());
                }
                "Type" => desktop_type = Some(value.to_string()),
                "Categories" if !value.is_empty() => {
                    categories.get_or_insert_with(|| value.to_string());
                }
                "Icon" => {
                    icon_name = normalize_icon_name(value);
                }
                _ => {}
            };
        }

        if desktop_type.as_deref() != Some("Application") {
            return Err("unsupported-type");
        }

        if hidden || no_display {
            return Err("hidden-or-nodisplay");
        }

        if let Some(only_show_in) = only_show_in {
            if !desktop_env_list_contains(&only_show_in, MERIDIAN_DESKTOP_ENV) {
                return Err("onlyshowin-excludes-meridian");
            }
        }

        if let Some(not_show_in) = not_show_in {
            if desktop_env_list_contains(&not_show_in, MERIDIAN_DESKTOP_ENV) {
                return Err("notshowin-includes-meridian");
            }
        }

        if let Some(try_exec) = try_exec {
            if !is_executable_available(try_exec.trim()) {
                return Err("tryexec-unavailable");
            }
        }

        let name = name.ok_or("missing-name")?;
        let exec_argv = exec_argv.ok_or("missing-exec")?;
        let mut app = Self::new(name, exec_argv, terminal);
        if let Some(raw_categories) = categories {
            app.categories = parse_categories(&raw_categories);
        }
        app.icon_name = icon_name;
        if app.name.is_empty() || app.program.is_empty() {
            return Err("empty-name-or-exec");
        }

        Ok(app)
    }
}

#[derive(Debug, Clone)]
pub struct LauncherState {
    pub open: bool,
    pub apps: Vec<DesktopApp>,
}

impl LauncherState {
    pub fn new_with_apps(apps: Vec<DesktopApp>) -> Self {
        Self { open: false, apps }
    }

    pub fn toggle(&mut self) -> bool {
        self.open = !self.open;
        if self.open {
            self.apps = DesktopApp::load_system();
        }
        self.open
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn reshuffle(&mut self) {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64 ^ d.as_secs().wrapping_mul(0x9e37_79b9))
            .unwrap_or(0xdead_beef);
        let mut rng = seed;
        let n = self.apps.len();
        for i in (1..n).rev() {
            rng = rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let j = (rng >> 33) as usize % (i + 1);
            self.apps.swap(i, j);
        }
    }

    pub(crate) fn launch_desktop_app(app: DesktopApp, ipc: &mut crate::IpcClient) {
        if app.program.trim().is_empty() {
            warn!("ignoring launch request for {}: empty argv", app.name);
            return;
        }

        info!(
            "requesting launch: {} (program={} args={:?})",
            app.name, app.program, app.args
        );
        let command = ShellCommand::LaunchApp {
            program: app.program.clone(),
            args: app.args.clone(),
            terminal: app.terminal,
        };
        if !ipc.send(&command) {
            warn!(
                "IPC unavailable, launching locally: program={} args={:?}",
                app.program, app.args
            );
            let mut local = if app.terminal {
                let Some(terminal_program) = terminal_program() else {
                    warn!(
                        "cannot launch terminal app {:?}: no terminal emulator found",
                        app.name
                    );
                    return;
                };
                let mut cmd = Command::new(terminal_program);
                cmd.arg("-e").arg(&app.program);
                cmd
            } else {
                Command::new(&app.program)
            };

            if let Ok(wayland_display) = std::env::var("WAYLAND_DISPLAY") {
                local.env("WAYLAND_DISPLAY", wayland_display);
            }
            if let Ok(xdg_runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
                local.env("XDG_RUNTIME_DIR", xdg_runtime_dir);
            }
            local
                .env("XDG_SESSION_TYPE", "wayland")
                .env("XDG_CURRENT_DESKTOP", "Meridian")
                .env("XDG_SESSION_DESKTOP", "meridian")
                .env("DESKTOP_SESSION", "meridian");
            if is_firefox_program(&app.program) && env::var_os("MOZ_ENABLE_WAYLAND").is_none() {
                local.env("MOZ_ENABLE_WAYLAND", "1");
            }

            match local.args(&app.args).spawn() {
                Ok(child) => info!("local launch pid: {}", child.id()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    warn!("failed to launch {}: program not found", app.name)
                }
                Err(err) => warn!("failed to launch {}: {}", app.name, err),
            }
        }
    }
}

fn parse_categories(raw: &str) -> Vec<String> {
    raw.split(';')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn normalize_icon_name(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    if value.starts_with('/') {
        return Some(value.to_string());
    }

    let lowered = value.to_ascii_lowercase();
    if lowered.ends_with(".png") || lowered.ends_with(".svg") || lowered.ends_with(".xpm") {
        return value.rsplit_once('.').map(|(base, _)| base.to_string());
    }

    Some(value.to_string())
}

fn parse_exec_argv(exec: &str) -> Vec<String> {
    tokenize_exec(exec)
        .into_iter()
        .filter_map(|token| {
            let cleaned = strip_field_codes(&token).trim().to_string();
            (!cleaned.is_empty()).then_some(cleaned)
        })
        .collect()
}

fn tokenize_exec(exec: &str) -> Vec<String> {
    #[derive(Copy, Clone, Eq, PartialEq)]
    enum Quote {
        Single,
        Double,
    }

    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut chars = exec.chars().peekable();

    while let Some(ch) = chars.next() {
        match quote {
            Some(Quote::Single) => {
                if ch == '\'' {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            Some(Quote::Double) => {
                if ch == '"' {
                    quote = None;
                } else if ch == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else {
                    current.push(ch);
                }
            }
            None => {
                if ch.is_whitespace() {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                } else if ch == '\'' {
                    quote = Some(Quote::Single);
                } else if ch == '"' {
                    quote = Some(Quote::Double);
                } else if ch == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else {
                    current.push(ch);
                }
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn strip_field_codes(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            output.push(ch);
            continue;
        }

        match chars.peek().copied() {
            Some('%') => {
                output.push('%');
                let _ = chars.next();
            }
            Some(next) if next.is_ascii_alphabetic() => {
                let _ = chars.next();
            }
            Some(next) => {
                output.push('%');
                output.push(next);
                let _ = chars.next();
            }
            None => output.push('%'),
        }
    }
    output
}

fn argv_to_display(program: &str, args: &[String]) -> String {
    if program.is_empty() {
        return String::new();
    }

    let mut display = String::from(program);
    for arg in args {
        display.push(' ');
        display.push_str(arg);
    }
    display
}

fn desktop_env_list_contains(value: &str, needle: &str) -> bool {
    value
        .split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .any(|entry| entry.eq_ignore_ascii_case(needle))
}

fn cmp_apps(left: &DesktopApp, right: &DesktopApp) -> Ordering {
    left.name_key
        .cmp(&right.name_key)
        .then_with(|| left.exec_key.cmp(&right.exec_key))
        .then_with(|| left.terminal.cmp(&right.terminal))
}

fn is_desktop_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("desktop"))
}

fn desktop_app_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen = HashSet::new();

    let local = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    if let Some(local) = local {
        push_unique_dir(&mut dirs, &mut seen, local.join("applications"));
    }

    let data_dirs = env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| XDG_DATA_DIRS_DEFAULT.to_string());
    for base in data_dirs
        .split(':')
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        push_unique_dir(
            &mut dirs,
            &mut seen,
            PathBuf::from(base).join("applications"),
        );
    }

    dirs
}

fn push_unique_dir(dirs: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, dir: PathBuf) {
    if seen.insert(dir.clone()) {
        dirs.push(dir);
    }
}

fn is_executable_available(binary_or_path: &str) -> bool {
    if binary_or_path.is_empty() {
        return false;
    }
    let candidate = Path::new(binary_or_path);
    if candidate.is_absolute() {
        return is_executable_file(candidate);
    }

    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path)
        .map(|entry| entry.join(binary_or_path))
        .any(|candidate| is_executable_file(&candidate))
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn terminal_program() -> Option<String> {
    env::var("TERMINAL")
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
            .find(|candidate| is_executable_available(candidate))
            .map(str::to_string)
        })
}

fn is_firefox_program(program: &str) -> bool {
    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("firefox") || name.eq_ignore_ascii_case("firefox-esr")
        })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        is_executable_available, normalize_icon_name, parse_categories, parse_exec_argv,
        DesktopApp, LauncherState,
    };

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(label: &str) -> Self {
            let mut path = std::env::temp_dir();
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            path.push(format!(
                "meridian-shell-launcher-{label}-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        fs::write(path, "#!/bin/sh\nexit 0\n").expect("write executable");
        let mut perms = fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(perms.mode() | 0o755);
        fs::set_permissions(path, perms).expect("chmod");
    }

    #[test]
    fn parses_valid_desktop_entry() {
        let raw = "\
[Desktop Entry]
Type=Application
Name=Firefox
Exec=firefox --new-window %u
Terminal=false
Categories=Network;WebBrowser;
Icon=firefox.png
";

        let app = DesktopApp::from_desktop_entry_str_with_reason(raw).expect("valid app");
        assert_eq!(app.name, "Firefox");
        assert_eq!(app.program, "firefox");
        assert_eq!(app.args, vec!["--new-window"]);
        assert!(!app.terminal);
        assert_eq!(app.categories, vec!["network", "webbrowser"]);
        assert_eq!(app.icon_name.as_deref(), Some("firefox"));
    }

    #[test]
    fn rejects_hidden_nodisplay_and_non_application_entries() {
        let unsupported = DesktopApp::from_desktop_entry_str_with_reason(
            "[Desktop Entry]\nType=Link\nName=X\nExec=x\n",
        );
        assert_eq!(unsupported.unwrap_err(), "unsupported-type");

        let hidden = DesktopApp::from_desktop_entry_str_with_reason(
            "[Desktop Entry]\nType=Application\nName=X\nExec=x\nHidden=true\n",
        );
        assert_eq!(hidden.unwrap_err(), "hidden-or-nodisplay");

        let no_display = DesktopApp::from_desktop_entry_str_with_reason(
            "[Desktop Entry]\nType=Application\nName=X\nExec=x\nNoDisplay=true\n",
        );
        assert_eq!(no_display.unwrap_err(), "hidden-or-nodisplay");
    }

    #[test]
    fn desktop_visibility_respects_meridian_environment_keys() {
        assert!(DesktopApp::from_desktop_entry_str_with_reason(
            "[Desktop Entry]\nType=Application\nName=X\nExec=x\nOnlyShowIn=Meridian;\n"
        )
        .is_ok());
        let other_desktop = DesktopApp::from_desktop_entry_str_with_reason(
            "[Desktop Entry]\nType=Application\nName=X\nExec=x\nOnlyShowIn=GNOME;\n",
        );
        assert_eq!(other_desktop.unwrap_err(), "onlyshowin-excludes-meridian");

        let hidden_from_meridian = DesktopApp::from_desktop_entry_str_with_reason(
            "[Desktop Entry]\nType=Application\nName=X\nExec=x\nNotShowIn=Meridian;\n",
        );
        assert_eq!(
            hidden_from_meridian.unwrap_err(),
            "notshowin-includes-meridian"
        );
    }

    #[test]
    fn exec_field_codes_are_removed() {
        assert_eq!(
            parse_exec_argv("firefox --new-window %u %% --name=%c"),
            vec!["firefox", "--new-window", "%", "--name="]
        );
    }

    #[test]
    fn exec_quotes_are_handled() {
        assert_eq!(
            parse_exec_argv("app \"two words\" 'three words' escaped\\ space"),
            vec!["app", "two words", "three words", "escaped space"]
        );
    }

    #[test]
    fn parses_categories_and_normalizes_icon_names() {
        assert_eq!(
            parse_categories("Network; WebBrowser;;"),
            vec!["network", "webbrowser"]
        );
        assert_eq!(
            normalize_icon_name("firefox.svg").as_deref(),
            Some("firefox")
        );
        assert_eq!(
            normalize_icon_name("/opt/icons/firefox.png").as_deref(),
            Some("/opt/icons/firefox.png")
        );
        assert_eq!(normalize_icon_name("   "), None);
    }

    #[test]
    fn try_exec_rejects_missing_binary() {
        assert!(!is_executable_available(""));
        assert!(!is_executable_available(
            "/definitely/missing/meridian-test-binary"
        ));
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            "[Desktop Entry]\nType=Application\nName=X\nExec=x\nTryExec=/definitely/missing/meridian-test-binary\n",
        );
        assert_eq!(app.unwrap_err(), "tryexec-unavailable");
    }

    #[cfg(unix)]
    #[test]
    fn load_from_dirs_deduplicates_sorts_and_checks_try_exec() {
        let temp = TempDir::new("load");
        let apps_dir = temp.path().join("applications");
        fs::create_dir_all(&apps_dir).expect("apps dir");
        let helper = temp.path().join("helper");
        make_executable(&helper);

        fs::write(
            apps_dir.join("b.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName=Beta\nExec={}\nTryExec={}\n",
                helper.display(),
                helper.display()
            ),
        )
        .expect("write beta");
        fs::write(
            apps_dir.join("a.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName=Alpha\nExec={}\n",
                helper.display()
            ),
        )
        .expect("write alpha");
        fs::write(
            apps_dir.join("duplicate.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName=Alpha\nExec={}\n",
                helper.display()
            ),
        )
        .expect("write duplicate");
        fs::write(
            apps_dir.join("ignored.desktop"),
            "[Desktop Entry]\nType=Application\nName=Ignored\nExec=ignored\nTryExec=/missing\n",
        )
        .expect("write ignored");

        let apps = DesktopApp::load_from_dirs(vec![apps_dir]);
        assert_eq!(
            apps.iter().map(|app| app.name.as_str()).collect::<Vec<_>>(),
            vec!["Alpha", "Beta"]
        );
    }

    #[test]
    fn launcher_state_tracks_open_close_and_apps() {
        let apps = vec![DesktopApp::new(
            "Firefox".to_string(),
            vec!["firefox".to_string()],
            false,
        )];
        let mut state = LauncherState::new_with_apps(apps);
        assert!(!state.open);
        assert_eq!(state.apps.len(), 1);
        state.close();
        assert!(!state.open);
    }
}
