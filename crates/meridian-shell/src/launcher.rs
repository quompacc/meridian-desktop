use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use meridian_config::{Color, ThemeConfig};
use meridian_ipc::ShellCommand;
use tracing::{debug, info, warn};

use crate::{ClickAction, ClickZone, Painter, Rect, TextRenderer};

const APP_ROW_H: i32 = 36;
const SEARCH_H: i32 = 44;
const HEADER_H: i32 = 22;
const PAD: i32 = 16;
const INNER_PAD: i32 = 12;
const ROW_GAP: i32 = 6;
const LIST_TOP_GAP: i32 = 10;
const MAX_RESULTS: usize = 9;
const XDG_DATA_DIRS_DEFAULT: &str = "/usr/local/share:/usr/share";
const MERIDIAN_DESKTOP_ENV: &str = "Meridian";

#[derive(Debug, Clone)]
pub struct DesktopApp {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    pub terminal: bool,
    name_key: String,
    exec_key: String,
}

impl DesktopApp {
    pub fn load_system() -> Vec<Self> {
        Self::load_from_dirs(desktop_app_dirs())
    }

    fn new(name: String, exec_argv: Vec<String>, terminal: bool) -> Self {
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

        apps.sort_by(|a, b| cmp_apps(a, b));
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
                "Name" => {
                    if !value.is_empty() {
                        name.get_or_insert_with(|| value.to_string());
                    }
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
                "TryExec" => {
                    if !value.is_empty() {
                        try_exec.get_or_insert_with(|| value.to_string());
                    }
                }
                "OnlyShowIn" => {
                    if !value.is_empty() {
                        only_show_in.get_or_insert_with(|| value.to_string());
                    }
                }
                "NotShowIn" => {
                    if !value.is_empty() {
                        not_show_in.get_or_insert_with(|| value.to_string());
                    }
                }
                "Type" => desktop_type = Some(value.to_string()),
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
        let app = Self::new(name, exec_argv, terminal);
        if app.name.is_empty() || app.program.is_empty() {
            return Err("empty-name-or-exec");
        }

        Ok(app)
    }

    fn matches_query(&self, query: &str) -> bool {
        query.is_empty() || self.name_key.contains(query) || self.exec_key.contains(query)
    }
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
        return path
            .metadata()
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
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

pub struct LauncherState {
    pub open: bool,
    pub query: String,
    pub selected_index: usize,
    pub clicks: Vec<ClickZone>,
    pub apps: Vec<DesktopApp>,
}

impl LauncherState {
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            selected_index: 0,
            clicks: Vec::new(),
            apps: DesktopApp::load_system(),
        }
    }

    pub fn toggle(&mut self) -> bool {
        self.open = !self.open;
        self.query.clear();
        self.selected_index = 0;
        self.open
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.selected_index = 0;
    }

    pub fn filtered_apps(&self) -> Vec<DesktopApp> {
        let query = self.query.to_lowercase();
        self.apps
            .iter()
            .filter(|app| app.matches_query(&query))
            .take(MAX_RESULTS)
            .cloned()
            .collect()
    }

    fn filtered_count(&self) -> usize {
        let query = self.query.to_lowercase();
        self.apps
            .iter()
            .filter(|app| app.matches_query(&query))
            .count()
    }

    fn filtered_visible_count(&self) -> usize {
        let query = self.query.to_lowercase();
        self.apps
            .iter()
            .filter(|app| app.matches_query(&query))
            .take(MAX_RESULTS)
            .count()
    }

    fn selected_index_clamped(&self, visible_len: usize) -> usize {
        if visible_len == 0 {
            0
        } else {
            self.selected_index.min(visible_len - 1)
        }
    }

    fn selected_launch_index(&self) -> Option<usize> {
        let visible_len = self.filtered_visible_count();
        if visible_len == 0 {
            None
        } else {
            Some(self.selected_index_clamped(visible_len))
        }
    }

    pub fn set_selected_index(&mut self, index: usize) -> bool {
        let visible_len = self.filtered_visible_count();
        let next = if visible_len == 0 {
            0
        } else {
            index.min(visible_len - 1)
        };
        if self.selected_index != next {
            self.selected_index = next;
            return true;
        }
        false
    }

    pub fn update_hover_selection(&mut self, x: f64, y: f64) -> bool {
        let hovered_index = self
            .clicks
            .iter()
            .find(|zone| zone.rect.contains(x, y))
            .and_then(|zone| match zone.action {
                ClickAction::LaunchApp(index) => Some(index),
                ClickAction::SwitchWorkspace(_) => None,
            });

        match hovered_index {
            Some(index) => self.set_selected_index(index),
            None => false,
        }
    }

    pub fn launch_app(&mut self, index: usize, ipc: &mut crate::IpcClient) {
        let apps = self.filtered_apps();
        if let Some(app) = apps.get(index).cloned() {
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

    pub fn handle_key(
        &mut self,
        ch: Option<char>,
        is_backspace: bool,
        is_enter: bool,
        is_escape: bool,
        is_up: bool,
        is_down: bool,
    ) -> LauncherInputResult {
        if !self.open {
            return LauncherInputResult::None;
        }

        if is_escape {
            self.close();
            return LauncherInputResult::Close;
        }

        if is_enter {
            return self
                .selected_launch_index()
                .map(LauncherInputResult::Launch)
                .unwrap_or(LauncherInputResult::None);
        }

        if is_up {
            if self.selected_index > 0 {
                self.selected_index -= 1;
                return LauncherInputResult::Redraw;
            }
            return LauncherInputResult::None;
        }

        if is_down {
            let visible_len = self.filtered_visible_count();
            if visible_len == 0 {
                return LauncherInputResult::None;
            }
            let max_idx = visible_len.saturating_sub(1);
            if self.selected_index < max_idx {
                self.selected_index += 1;
                return LauncherInputResult::Redraw;
            }
            return LauncherInputResult::None;
        }

        if is_backspace {
            self.query.pop();
            self.selected_index = 0;
            return LauncherInputResult::Redraw;
        }

        if let Some(ch) = ch {
            if !ch.is_control() {
                self.query.push(ch);
                self.selected_index = 0;
                return LauncherInputResult::Redraw;
            }
        }

        LauncherInputResult::None
    }
}

pub enum LauncherInputResult {
    None,
    Redraw,
    Close,
    Launch(usize),
}

pub fn draw_launcher(
    launcher_state: &mut LauncherState,
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    width: u32,
    height: u32,
) {
    if !launcher_state.open {
        return;
    }

    let colors = &theme.colors;
    painter.clear(colors.surface);
    let outer = Rect {
        x: 0,
        y: 0,
        w: width as i32,
        h: height as i32,
    };
    painter.stroke_rect(outer, colors.border);

    launcher_state.clicks.clear();

    let results_total = launcher_state.filtered_count();
    let apps = launcher_state.filtered_apps();
    let selected_idx = launcher_state.selected_index_clamped(apps.len());

    let header_rect = Rect {
        x: PAD,
        y: PAD,
        w: width as i32 - PAD * 2,
        h: HEADER_H,
    };
    painter.text_clipped(
        font,
        "Launcher",
        header_rect.x,
        header_rect.y + 16,
        header_rect.w / 2,
        colors.text,
    );
    let count_text = format!("{} Treffer", results_total);
    painter.text_clipped(
        font,
        &count_text,
        header_rect.x + header_rect.w / 2,
        header_rect.y + 16,
        header_rect.w / 2,
        colors.border,
    );

    let search_rect = Rect {
        x: PAD,
        y: PAD + HEADER_H + 2,
        w: width as i32 - PAD * 2,
        h: SEARCH_H,
    };
    painter.roundish_rect(search_rect, colors.background);
    painter.stroke_rect(search_rect, colors.border);
    let query_text = if launcher_state.query.is_empty() {
        "Type to search applications"
    } else {
        &launcher_state.query
    };
    let query_color = if launcher_state.query.is_empty() {
        colors.border
    } else {
        colors.text
    };
    painter.text_clipped(
        font,
        query_text,
        search_rect.x + INNER_PAD,
        search_rect.y + 28,
        search_rect.w - INNER_PAD * 2,
        query_color,
    );

    let mut y = search_rect.y + SEARCH_H + LIST_TOP_GAP;
    if apps.is_empty() {
        let empty = if launcher_state.query.is_empty() {
            "Keine Anwendungen gefunden"
        } else {
            "Keine Treffer. Suche anpassen"
        };
        let empty_rect = Rect {
            x: PAD,
            y,
            w: width as i32 - PAD * 2,
            h: APP_ROW_H,
        };
        painter.roundish_rect(empty_rect, colors.background);
        painter.text_clipped(
            font,
            empty,
            empty_rect.x + INNER_PAD,
            empty_rect.y + 24,
            empty_rect.w - INNER_PAD * 2,
            colors.border,
        );
        return;
    }

    for (index, app) in apps.iter().enumerate() {
        let is_selected = index == selected_idx;
        let rect = Rect {
            x: PAD,
            y,
            w: width as i32 - PAD * 2,
            h: APP_ROW_H,
        };
        let bg = if is_selected {
            colors.accent
        } else {
            colors.background
        };
        painter.roundish_rect(rect, bg);
        if is_selected {
            painter.stroke_rect(rect, colors.border);
        }
        let text_color = if is_selected {
            Color::rgb(0x1e, 0x1e, 0x2e)
        } else {
            colors.text
        };
        painter.text_clipped(
            font,
            &app.name,
            rect.x + INNER_PAD,
            rect.y + 23,
            rect.w - INNER_PAD * 2,
            text_color,
        );
        launcher_state.clicks.push(ClickZone {
            rect,
            action: ClickAction::LaunchApp(index),
        });
        y += APP_ROW_H + ROW_GAP;
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
    };

    use super::{
        desktop_app_dirs, is_executable_available, parse_exec_argv, ClickAction, ClickZone,
        DesktopApp, LauncherInputResult, LauncherState, Rect, XDG_DATA_DIRS_DEFAULT,
    };

    static TEST_ID: AtomicU64 = AtomicU64::new(1);

    struct EnvVarGuard {
        key: &'static str,
        old: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let old = std::env::var_os(key);
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, old }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.old {
                Some(value) => unsafe {
                    std::env::set_var(self.key, value);
                },
                None => unsafe {
                    std::env::remove_var(self.key);
                },
            }
        }
    }

    fn unique_test_dir(prefix: &str) -> PathBuf {
        let id = TEST_ID.fetch_add(1, AtomicOrdering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "meridian-launcher-tests-{}-{}-{}",
            prefix,
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    #[test]
    fn parses_valid_desktop_entry() {
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Calculator
Exec=gnome-calculator %U
Terminal=false
"#,
        )
        .expect("valid desktop entry");

        assert_eq!(app.name, "Calculator");
        assert_eq!(app.program, "gnome-calculator");
        assert!(app.args.is_empty());
        assert!(!app.terminal);
    }

    #[test]
    fn rejects_hidden_nodisplay_and_non_application_entries() {
        let hidden = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Hidden
Exec=hidden
Hidden=true
"#,
        );
        assert!(hidden.is_err());

        let nodisplay = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=NoDisplay
Exec=nodisplay
NoDisplay=true
"#,
        );
        assert!(nodisplay.is_err());

        let link_type = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Link
Name=Link
Exec=link
"#,
        );
        assert!(link_type.is_err());
    }

    #[test]
    fn only_show_in_with_meridian_is_allowed() {
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Allowed
Exec=allowed
OnlyShowIn=GNOME;Meridian;KDE;
"#,
        );
        assert!(app.is_ok());
    }

    #[test]
    fn only_show_in_without_meridian_is_ignored() {
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Blocked
Exec=blocked
OnlyShowIn=GNOME;KDE;
"#,
        );
        assert!(app.is_err());
    }

    #[test]
    fn not_show_in_with_meridian_is_ignored() {
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Blocked
Exec=blocked
NotShowIn=Meridian;XFCE;
"#,
        );
        assert!(app.is_err());
    }

    #[test]
    fn rejects_empty_name_or_exec() {
        let missing_name = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Exec=app
"#,
        );
        assert!(missing_name.is_err());

        let missing_exec = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=App
Exec=%U
"#,
        );
        assert!(missing_exec.is_err());
    }

    #[test]
    fn load_from_dirs_sorts_stably_and_ignores_invalid_entries() {
        let dir = unique_test_dir("scan");

        fs::write(
            dir.join("03-invalid.desktop"),
            r#"
[Desktop Entry]
Type=Application
Name=Invalid
Exec=%U
"#,
        )
        .expect("write invalid");

        fs::write(
            dir.join("02-zeta.desktop"),
            r#"
[Desktop Entry]
Type=Application
Name=Zeta
Exec=zeta
"#,
        )
        .expect("write zeta");

        fs::write(
            dir.join("01-alpha.desktop"),
            r#"
[Desktop Entry]
Type=Application
Name=alpha
Exec=alpha
"#,
        )
        .expect("write alpha");

        let apps = DesktopApp::load_from_dirs(vec![dir.clone()]);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].name, "alpha");
        assert_eq!(apps[1].name, "Zeta");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn load_from_dirs_deduplicates_same_name_and_exec() {
        let dir = unique_test_dir("dedupe");

        fs::write(
            dir.join("app-a.desktop"),
            r#"
[Desktop Entry]
Type=Application
Name=Viewer
Exec=viewer %F
"#,
        )
        .expect("write app a");

        fs::write(
            dir.join("app-b.desktop"),
            r#"
[Desktop Entry]
Type=Application
Name=viewer
Exec=viewer %U
"#,
        )
        .expect("write app b");

        let apps = DesktopApp::load_from_dirs(vec![dir.clone()]);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name.to_lowercase(), "viewer");
        assert_eq!(apps[0].program, "viewer");
        assert!(apps[0].args.is_empty());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn filter_is_case_insensitive_and_checks_name_and_exec() {
        let apps = vec![
            DesktopApp::new("Firefox".to_string(), vec!["firefox".to_string()], false),
            DesktopApp::new("Terminal".to_string(), vec!["foot".to_string()], true),
        ];

        let query = "FIRE";
        let query = query.to_lowercase();
        let filtered = apps
            .iter()
            .filter(|app| app.matches_query(&query))
            .collect::<Vec<_>>();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Firefox");

        let query = "FOOT".to_lowercase();
        let filtered = apps
            .iter()
            .filter(|app| app.matches_query(&query))
            .collect::<Vec<_>>();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Terminal");
    }

    #[test]
    fn exec_field_codes_are_removed() {
        let argv = parse_exec_argv(r#"app --open %f --name %c %%"#);
        assert_eq!(argv, vec!["app", "--open", "--name", "%"]);
    }

    #[test]
    fn exec_simple_quotes_are_handled() {
        let argv = parse_exec_argv(r#"myapp --title "Hello World" --class 'Meridian App'"#);
        assert_eq!(
            argv,
            vec!["myapp", "--title", "Hello World", "--class", "Meridian App"]
        );
    }

    #[test]
    fn exec_empty_is_rejected() {
        let argv = parse_exec_argv("%f %u %i %c %k");
        assert!(argv.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn try_exec_accepts_absolute_executable_and_path_lookup() {
        use std::os::unix::fs::PermissionsExt;

        let bin_dir = unique_test_dir("tryexec-bin");
        let bin = bin_dir.join("demo-bin");
        fs::write(&bin, "#!/bin/sh\nexit 0\n").expect("write script");
        let mut perms = fs::metadata(&bin).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&bin, perms).expect("chmod");

        assert!(is_executable_available(
            bin.to_str().expect("absolute path utf8")
        ));

        let _path = EnvVarGuard::set("PATH", bin_dir.to_str().expect("path utf8"));
        assert!(is_executable_available("demo-bin"));

        let _ = fs::remove_dir_all(bin_dir);
    }

    #[cfg(unix)]
    #[test]
    fn try_exec_rejects_file_without_execute_bit() {
        use std::os::unix::fs::PermissionsExt;

        let bin_dir = unique_test_dir("tryexec-noexec");
        let bin = bin_dir.join("demo-noexec");
        fs::write(&bin, "echo nope\n").expect("write file");
        let mut perms = fs::metadata(&bin).expect("metadata").permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&bin, perms).expect("chmod");

        assert!(!is_executable_available(
            bin.to_str().expect("absolute path utf8")
        ));

        let _ = fs::remove_dir_all(bin_dir);
    }

    #[test]
    fn desktop_app_dirs_uses_xdg_data_dirs_default_when_empty() {
        let _xdg_data_dirs = EnvVarGuard::set("XDG_DATA_DIRS", "");
        let _xdg_data_home = EnvVarGuard::set("XDG_DATA_HOME", "/tmp/meridian-xdg-home");
        let dirs = desktop_app_dirs();

        assert!(dirs
            .iter()
            .any(|path| path == &PathBuf::from("/tmp/meridian-xdg-home/applications")));
        for base in XDG_DATA_DIRS_DEFAULT.split(':') {
            assert!(dirs
                .iter()
                .any(|path| path == &PathBuf::from(base).join("applications")));
        }
    }

    #[test]
    fn launcher_selection_moves_with_up_down_and_enter_uses_selection() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            clicks: Vec::new(),
            apps: vec![
                DesktopApp::new("Alpha".to_string(), vec!["alpha".to_string()], false),
                DesktopApp::new("Beta".to_string(), vec!["beta".to_string()], false),
                DesktopApp::new("Gamma".to_string(), vec!["gamma".to_string()], false),
            ],
        };

        let moved = state.handle_key(None, false, false, false, false, true);
        assert!(matches!(moved, LauncherInputResult::Redraw));
        assert_eq!(state.selected_index, 1);

        let moved = state.handle_key(None, false, false, false, false, true);
        assert!(matches!(moved, LauncherInputResult::Redraw));
        assert_eq!(state.selected_index, 2);

        let moved = state.handle_key(None, false, false, false, true, false);
        assert!(matches!(moved, LauncherInputResult::Redraw));
        assert_eq!(state.selected_index, 1);

        let launch = state.handle_key(None, false, true, false, false, false);
        assert!(matches!(launch, LauncherInputResult::Launch(1)));
    }

    #[test]
    fn launcher_query_edit_resets_selection() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 2,
            clicks: Vec::new(),
            apps: vec![DesktopApp::new(
                "Terminal".to_string(),
                vec!["foot".to_string()],
                false,
            )],
        };

        let redraw = state.handle_key(Some('t'), false, false, false, false, false);
        assert!(matches!(redraw, LauncherInputResult::Redraw));
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn hover_sets_selected_index() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            clicks: vec![ClickZone {
                rect: Rect {
                    x: 0,
                    y: 0,
                    w: 100,
                    h: 20,
                },
                action: ClickAction::LaunchApp(2),
            }],
            apps: vec![
                DesktopApp::new("A".to_string(), vec!["a".to_string()], false),
                DesktopApp::new("B".to_string(), vec!["b".to_string()], false),
                DesktopApp::new("C".to_string(), vec!["c".to_string()], false),
            ],
        };

        let changed = state.update_hover_selection(10.0, 10.0);
        assert!(changed);
        assert_eq!(state.selected_index, 2);
    }

    #[test]
    fn hover_out_of_range_is_ignored() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 1,
            clicks: vec![ClickZone {
                rect: Rect {
                    x: 0,
                    y: 0,
                    w: 10,
                    h: 10,
                },
                action: ClickAction::LaunchApp(0),
            }],
            apps: vec![
                DesktopApp::new("A".to_string(), vec!["a".to_string()], false),
                DesktopApp::new("B".to_string(), vec!["b".to_string()], false),
            ],
        };

        let changed = state.update_hover_selection(30.0, 30.0);
        assert!(!changed);
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn keyboard_navigation_continues_from_hover_selection() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            clicks: vec![ClickZone {
                rect: Rect {
                    x: 0,
                    y: 0,
                    w: 50,
                    h: 20,
                },
                action: ClickAction::LaunchApp(1),
            }],
            apps: vec![
                DesktopApp::new("A".to_string(), vec!["a".to_string()], false),
                DesktopApp::new("B".to_string(), vec!["b".to_string()], false),
                DesktopApp::new("C".to_string(), vec!["c".to_string()], false),
            ],
        };

        assert!(state.update_hover_selection(5.0, 5.0));
        assert_eq!(state.selected_index, 1);

        let moved = state.handle_key(None, false, false, false, false, true);
        assert!(matches!(moved, LauncherInputResult::Redraw));
        assert_eq!(state.selected_index, 2);
    }

    #[test]
    fn enter_with_no_visible_results_is_ignored() {
        let mut state = LauncherState {
            open: true,
            query: "zzzz-no-hit".to_string(),
            selected_index: 5,
            clicks: Vec::new(),
            apps: vec![DesktopApp::new(
                "Terminal".to_string(),
                vec!["foot".to_string()],
                false,
            )],
        };

        let result = state.handle_key(None, false, true, false, false, false);
        assert!(matches!(result, LauncherInputResult::None));
        assert_eq!(state.selected_index, 5);
    }

    #[test]
    fn enter_uses_clamped_selected_index_for_visible_results() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 99,
            clicks: Vec::new(),
            apps: vec![
                DesktopApp::new("Alpha".to_string(), vec!["alpha".to_string()], false),
                DesktopApp::new("Beta".to_string(), vec!["beta".to_string()], false),
            ],
        };

        let result = state.handle_key(None, false, true, false, false, false);
        assert!(matches!(result, LauncherInputResult::Launch(1)));
    }
}
