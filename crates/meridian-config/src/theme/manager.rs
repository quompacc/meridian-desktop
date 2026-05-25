use std::{
    collections::HashSet,
    env, fmt, fs,
    path::{Path, PathBuf},
};

use tracing::{info, warn};

use super::{ThemeConfig, ThemeError};

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub dir: PathBuf,
    pub config: ThemeConfig,
}

impl Theme {
    fn builtin_default() -> Self {
        Self {
            name: "default".to_string(),
            dir: PathBuf::new(),
            config: ThemeConfig::default(),
        }
    }

    fn load_from_dir(name: &str, dir: &Path) -> Result<Self, ThemeError> {
        let toml_path = dir.join("theme.toml");
        if !toml_path.exists() {
            return Err(ThemeError::NotFound(name.to_string()));
        }
        let raw = fs::read_to_string(&toml_path)?;
        let config: ThemeConfig = toml::from_str(&raw)?;
        Ok(Self {
            name: name.to_string(),
            dir: dir.to_path_buf(),
            config,
        })
    }

    pub fn css_path(&self) -> Option<PathBuf> {
        if self.dir.as_os_str().is_empty() {
            return None;
        }
        let p = self.dir.join("style.css");
        p.exists().then_some(p)
    }

    pub fn asset_path(&self, name: &str) -> Option<PathBuf> {
        if self.dir.as_os_str().is_empty() {
            return None;
        }
        let p = self.dir.join("assets").join(name);
        p.exists().then_some(p)
    }

    pub fn wallpaper_path(&self) -> Option<PathBuf> {
        let wp = self.config.wallpaper.as_ref()?;
        if wp.path.trim().is_empty() {
            return None;
        }
        let expanded = expand_tilde(&wp.path);
        if expanded.is_absolute() {
            Some(expanded)
        } else {
            Some(self.dir.join(expanded))
        }
    }
}

type ThemeChangedCallback = Box<dyn Fn(&Theme) + 'static>;

pub struct ThemeManager {
    current: Theme,
    user_themes_dir: PathBuf,
    theme_dirs: Vec<PathBuf>,
    observers: Vec<ThemeChangedCallback>,
}

impl ThemeManager {
    pub fn new() -> Self {
        let theme_dirs = theme_directories();
        let user_themes_dir = user_theme_directory();
        let current = load_or_default(&theme_dirs);
        Self {
            current,
            user_themes_dir,
            theme_dirs,
            observers: Vec::new(),
        }
    }

    #[cfg(test)]
    fn new_with_dirs_for_tests(theme_dirs: Vec<PathBuf>) -> Self {
        let user_themes_dir = theme_dirs.first().cloned().unwrap_or_default();
        let current = load_or_default(&theme_dirs);
        Self {
            current,
            user_themes_dir,
            theme_dirs,
            observers: Vec::new(),
        }
    }

    pub fn current(&self) -> &Theme {
        &self.current
    }

    pub fn current_mut(&mut self) -> &mut Theme {
        &mut self.current
    }

    pub fn themes_dir(&self) -> &Path {
        &self.user_themes_dir
    }

    pub fn theme_dirs(&self) -> &[PathBuf] {
        &self.theme_dirs
    }

    pub fn set_theme(&mut self, name: &str) -> Result<(), ThemeError> {
        let theme = if name == "default" {
            load_named_theme("default", &self.theme_dirs)
                .unwrap_or_else(|_| Theme::builtin_default())
        } else {
            load_named_theme(name, &self.theme_dirs)?
        };
        info!("Theme: {} -> {}", self.current.name, theme.name);
        self.current = theme;
        self.notify_observers();
        Ok(())
    }

    pub fn reload(&mut self) -> Result<(), ThemeError> {
        let name = self.current.name.clone();
        self.set_theme(&name)
    }

    pub fn available_themes(&self) -> Vec<String> {
        available_theme_names(&self.theme_dirs)
    }

    pub fn register_observer(&mut self, f: impl Fn(&Theme) + 'static) {
        self.observers.push(Box::new(f));
    }

    fn notify_observers(&self) {
        for cb in &self.observers {
            cb(&self.current);
        }
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ThemeManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThemeManager")
            .field("current", &self.current.name)
            .field("user_themes_dir", &self.user_themes_dir)
            .field("theme_dirs", &self.theme_dirs)
            .field("observers", &self.observers.len())
            .finish()
    }
}

fn expand_tilde(s: &str) -> PathBuf {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    } else if s == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home);
        }
    }
    PathBuf::from(s)
}

fn user_theme_directory() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("meridian")
        .join("themes")
}

fn theme_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    push_unique_path(&mut dirs, user_theme_directory());

    if let Ok(value) = env::var("MERIDIAN_THEME_DIR") {
        push_unique_path(&mut dirs, PathBuf::from(value));
    }
    if let Ok(value) = env::var("MERIDIAN_THEME_DIRS") {
        for path in env::split_paths(&value) {
            push_unique_path(&mut dirs, path);
        }
    }

    let data_home = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    if let Some(dir) = data_home {
        push_unique_path(&mut dirs, dir.join("meridian").join("themes"));
    }

    let data_dirs = env::var_os("XDG_DATA_DIRS")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    for dir in env::split_paths(&data_dirs) {
        push_unique_path(&mut dirs, dir.join("meridian").join("themes"));
    }

    push_unique_path(&mut dirs, dev_theme_directory());
    dirs
}

fn dev_theme_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes")
}

fn push_unique_path(dirs: &mut Vec<PathBuf>, path: PathBuf) {
    if path.as_os_str().is_empty() || dirs.iter().any(|existing| existing == &path) {
        return;
    }
    dirs.push(path);
}

fn load_named_theme(name: &str, theme_dirs: &[PathBuf]) -> Result<Theme, ThemeError> {
    for base in theme_dirs {
        let dir = base.join(name);
        if dir.join("theme.toml").exists() {
            return Theme::load_from_dir(name, &dir);
        }
    }
    Err(ThemeError::NotFound(name.to_string()))
}

fn available_theme_names(theme_dirs: &[PathBuf]) -> Vec<String> {
    let mut names = vec!["default".to_string()];
    let mut seen = HashSet::from(["default".to_string()]);
    for dir in theme_dirs {
        let Ok(rd) = fs::read_dir(dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("theme.toml").exists() {
                if let Some(n) = path.file_name().and_then(|s| s.to_str()) {
                    if seen.insert(n.to_string()) {
                        names.push(n.to_string());
                    }
                }
            }
        }
    }
    names.sort();
    names
}

fn load_or_default(theme_dirs: &[PathBuf]) -> Theme {
    match load_named_theme("default", theme_dirs) {
        Ok(theme) => {
            info!("Loaded theme \"default\" from {:?}", theme.dir);
            theme
        }
        Err(ThemeError::NotFound(_)) => {
            info!("Using built-in default theme (Catppuccin Mocha)");
            Theme::builtin_default()
        }
        Err(err) => {
            warn!(
                "Failed to load external default theme: {} - using built-in",
                err
            );
            info!("Using built-in default theme (Catppuccin Mocha)");
            Theme::builtin_default()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::ThemeManager;

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
                "meridian-theme-manager-{label}-{}-{nanos}",
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

    fn write_theme(base: &Path, name: &str, accent: &str) {
        let dir = base.join(name);
        fs::create_dir_all(&dir).expect("create theme dir");
        fs::write(
            dir.join("theme.toml"),
            format!(
                r##"
[colors]
accent = "{accent}"
"##
            ),
        )
        .expect("write theme");
    }

    #[test]
    fn available_themes_scans_all_configured_dirs() {
        let user = TempDir::new("user");
        let system = TempDir::new("system");
        write_theme(user.path(), "earth-cream", "#6f5336");
        write_theme(system.path(), "catppuccin-mocha", "#cba6f7");

        let manager = ThemeManager::new_with_dirs_for_tests(vec![
            user.path().to_path_buf(),
            system.path().to_path_buf(),
        ]);

        assert_eq!(
            manager.available_themes(),
            vec!["catppuccin-mocha", "default", "earth-cream"]
        );
    }

    #[test]
    fn set_theme_prefers_earlier_dirs() {
        let user = TempDir::new("user-precedence");
        let system = TempDir::new("system-precedence");
        write_theme(user.path(), "shared", "#111111");
        write_theme(system.path(), "shared", "#222222");

        let mut manager = ThemeManager::new_with_dirs_for_tests(vec![
            user.path().to_path_buf(),
            system.path().to_path_buf(),
        ]);
        manager.set_theme("shared").expect("set theme");

        assert_eq!(manager.current().config.colors.accent.to_hex(), "#111111");
    }
}
