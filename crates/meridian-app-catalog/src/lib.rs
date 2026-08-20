#![deny(unsafe_code)]
//! Cached, platform-neutral application metadata for Meridian shell clients.
//!
//! Filesystem discovery and desktop-entry parsing happen in Rust. UI documents
//! receive only the validated snapshot and never ambient filesystem access.

use std::{
    cmp::Ordering,
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};

use tracing::debug;

const XDG_DATA_DIRS_DEFAULT: &str = "/usr/local/share:/usr/share";
const MERIDIAN_DESKTOP_ENV: &str = "Meridian";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopApp {
    pub desktop_id: String,
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

    pub fn new(name: String, exec_argv: Vec<String>, terminal: bool) -> Self {
        let name = name.trim().to_string();
        let program = exec_argv.first().cloned().unwrap_or_default();
        let args = exec_argv.iter().skip(1).cloned().collect::<Vec<_>>();
        let exec = argv_to_display(&program, &args);
        Self {
            desktop_id: String::new(),
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

    pub fn load_from_dirs(dirs: Vec<PathBuf>) -> Vec<Self> {
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
        match Self::parse(&raw) {
            Ok(mut app) => {
                app.desktop_id = path.file_name()?.to_string_lossy().into_owned();
                Some(app)
            }
            Err(reason) => {
                debug!(path=?path, reason, "launcher ignored desktop entry");
                None
            }
        }
    }

    pub fn parse(raw: &str) -> Result<Self, &'static str> {
        let mut in_entry = false;
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
                in_entry = line == "[Desktop Entry]";
                continue;
            }
            if !in_entry || line.starts_with('#') || line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "Name" if !value.is_empty() => {
                    name.get_or_insert_with(|| value.to_string());
                }
                "Exec" => {
                    let argv = parse_exec_argv(value);
                    if !argv.is_empty() {
                        exec_argv.get_or_insert(argv);
                    }
                }
                "Terminal" => {
                    terminal = value.eq_ignore_ascii_case("true");
                }
                "Hidden" => {
                    hidden = value.eq_ignore_ascii_case("true");
                }
                "NoDisplay" => {
                    no_display = value.eq_ignore_ascii_case("true");
                }
                "TryExec" if !value.is_empty() => {
                    try_exec.get_or_insert_with(|| value.to_string());
                }
                "OnlyShowIn" if !value.is_empty() => {
                    only_show_in.get_or_insert_with(|| value.to_string());
                }
                "NotShowIn" if !value.is_empty() => {
                    not_show_in.get_or_insert_with(|| value.to_string());
                }
                "Type" => {
                    desktop_type = Some(value.to_string());
                }
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
        if only_show_in
            .is_some_and(|value| !desktop_env_list_contains(&value, MERIDIAN_DESKTOP_ENV))
        {
            return Err("onlyshowin-excludes-meridian");
        }
        if not_show_in.is_some_and(|value| desktop_env_list_contains(&value, MERIDIAN_DESKTOP_ENV))
        {
            return Err("notshowin-includes-meridian");
        }
        if try_exec.is_some_and(|value| !is_executable_available(value.trim())) {
            return Err("tryexec-unavailable");
        }

        let mut app = Self::new(
            name.ok_or("missing-name")?,
            exec_argv.ok_or("missing-exec")?,
            terminal,
        );
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
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut chars = exec.chars().peekable();
    while let Some(ch) = chars.next() {
        match quote {
            Some(expected) if ch == expected => quote = None,
            Some('"') if ch == '\\' => current.extend(chars.next()),
            Some(_) => current.push(ch),
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            None if ch == '\'' || ch == '"' => quote = Some(ch),
            None if ch == '\\' => current.extend(chars.next()),
            None => current.push(ch),
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
        match chars.next() {
            Some('%') => output.push('%'),
            Some(next) if next.is_ascii_alphabetic() => {}
            Some(next) => {
                output.push('%');
                output.push(next);
            }
            None => output.push('%'),
        }
    }
    output
}

fn argv_to_display(program: &str, args: &[String]) -> String {
    std::iter::once(program)
        .chain(args.iter().map(String::as_str))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn desktop_env_list_contains(value: &str, needle: &str) -> bool {
    value
        .split(';')
        .map(str::trim)
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
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| XDG_DATA_DIRS_DEFAULT.to_string());
    for base in data_dirs
        .split(':')
        .map(str::trim)
        .filter(|value| !value.is_empty())
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
    env::var_os("PATH").is_some_and(|path| {
        env::split_paths(&path)
            .map(|entry| entry.join(binary_or_path))
            .any(|candidate| is_executable_file(&candidate))
    })
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests;
