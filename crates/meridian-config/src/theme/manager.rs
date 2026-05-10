use std::{
    fmt, fs,
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
    themes_dir: PathBuf,
    observers: Vec<ThemeChangedCallback>,
}

impl ThemeManager {
    pub fn new() -> Self {
        let themes_dir = themes_directory();
        let current = load_or_default(&themes_dir);
        Self {
            current,
            themes_dir,
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
        &self.themes_dir
    }

    pub fn set_theme(&mut self, name: &str) -> Result<(), ThemeError> {
        let theme = if name == "default" {
            Theme::builtin_default()
        } else {
            let dir = self.themes_dir.join(name);
            Theme::load_from_dir(name, &dir)?
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
        let mut names = vec!["default".to_string()];
        if let Ok(rd) = fs::read_dir(&self.themes_dir) {
            for entry in rd.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("theme.toml").exists() {
                    if let Some(n) = path.file_name().and_then(|s| s.to_str()) {
                        names.push(n.to_string());
                    }
                }
            }
        }
        names.sort();
        names.dedup();
        names
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
            .field("themes_dir", &self.themes_dir)
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

fn themes_directory() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("meridian")
        .join("themes")
}

fn load_or_default(themes_dir: &Path) -> Theme {
    let dir = themes_dir.join("default");
    if dir.join("theme.toml").exists() {
        match Theme::load_from_dir("default", &dir) {
            Ok(t) => {
                info!("Loaded theme \"default\" from {:?}", dir);
                return t;
            }
            Err(e) => warn!(
                "Failed to load theme from {:?}: {} — using built-in",
                dir, e
            ),
        }
    }
    info!("Using built-in default theme (Catppuccin Mocha)");
    Theme::builtin_default()
}
