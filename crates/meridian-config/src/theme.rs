use std::{
    fmt, fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use serde::Deserialize;
use tracing::{info, warn};

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ThemeError {
    Io(std::io::Error),
    Parse(toml::de::Error),
    NotFound(String),
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::Parse(e) => write!(f, "TOML parse error: {}", e),
            Self::NotFound(n) => write!(f, "theme not found: {}", n),
        }
    }
}

impl std::error::Error for ThemeError {}

impl From<std::io::Error> for ThemeError {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}

impl From<toml::de::Error> for ThemeError {
    fn from(e: toml::de::Error) -> Self { Self::Parse(e) }
}

// ── Color ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self { Self { r, g, b, a } }
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self { Self::rgba(r, g, b, 255) }

    pub fn as_f32_array(self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }

    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }
}

impl FromStr for Color {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim_start_matches('#');
        let byte = |i: usize| -> Result<u8, String> {
            u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string())
        };
        match s.len() {
            6 => Ok(Self::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 => Ok(Self::rgba(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            _ => Err(format!("invalid color \"#{}\": expected 6 or 8 hex digits", s)),
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

// ── [colors] ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ThemeColors {
    pub background: Color,
    pub surface:    Color,
    pub accent:     Color,
    pub text:       Color,
    pub border:     Color,
    pub error:      Color,
    pub warning:    Color,
    pub success:    Color,
}

impl Default for ThemeColors {
    fn default() -> Self {
        // Catppuccin Mocha
        Self {
            background: Color::rgb(0x1e, 0x1e, 0x2e),
            surface:    Color::rgb(0x31, 0x32, 0x44),
            accent:     Color::rgb(0xcb, 0xa6, 0xf7),
            text:       Color::rgb(0xcd, 0xd6, 0xf4),
            border:     Color::rgb(0x45, 0x47, 0x5a),
            error:      Color::rgb(0xf3, 0x8b, 0xa8),
            warning:    Color::rgb(0xfa, 0xb3, 0x87),
            success:    Color::rgb(0xa6, 0xe3, 0xa1),
        }
    }
}

// ── [decorations] ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Decorations {
    pub border_width:  u32,
    pub corner_radius: u32,
    pub shadow:        bool,
    pub shadow_radius: u32,
    pub gap:           u32,
}

impl Default for Decorations {
    fn default() -> Self {
        Self {
            border_width:  2,
            corner_radius: 8,
            shadow:        true,
            shadow_radius: 12,
            gap:           8,
        }
    }
}

// ── [fonts] ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Fonts {
    pub ui:   String,
    pub mono: String,
}

impl Default for Fonts {
    fn default() -> Self {
        Self {
            ui:   "Inter 11".to_string(),
            mono: "JetBrains Mono 10".to_string(),
        }
    }
}

// ── [icons] ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Icons {
    pub theme: String,
}

impl Default for Icons {
    fn default() -> Self {
        Self { theme: "Papirus-Dark".to_string() }
    }
}

// ── [cursor] ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Cursor {
    pub theme: String,
    pub size:  u32,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            theme: "Bibata-Modern-Classic".to_string(),
            size:  24,
        }
    }
}

// ── [wallpaper] ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WallpaperMode {
    #[default] Fill,
    Fit,
    Center,
    Tile,
}

impl fmt::Display for WallpaperMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Fill   => "fill",
            Self::Fit    => "fit",
            Self::Center => "center",
            Self::Tile   => "tile",
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Wallpaper {
    pub path: String,
    #[serde(default)]
    pub mode: WallpaperMode,
}

// ── ThemeConfig (full theme.toml) ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ThemeConfig {
    pub colors:      ThemeColors,
    pub decorations: Decorations,
    pub fonts:       Fonts,
    pub icons:       Icons,
    pub cursor:      Cursor,
    pub wallpaper:   Option<Wallpaper>,
}

// ── Theme ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Theme {
    pub name:   String,
    /// Directory on disk; empty for the built-in default.
    pub dir:    PathBuf,
    pub config: ThemeConfig,
}

impl Theme {
    fn builtin_default() -> Self {
        Self {
            name:   "default".to_string(),
            dir:    PathBuf::new(),
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
        Ok(Self { name: name.to_string(), dir: dir.to_path_buf(), config })
    }

    /// Path to the optional CSS file for this theme.
    pub fn css_path(&self) -> Option<PathBuf> {
        if self.dir.as_os_str().is_empty() { return None; }
        let p = self.dir.join("style.css");
        p.exists().then_some(p)
    }

    /// Resolve an asset name relative to `assets/` inside the theme dir.
    pub fn asset_path(&self, name: &str) -> Option<PathBuf> {
        if self.dir.as_os_str().is_empty() { return None; }
        let p = self.dir.join("assets").join(name);
        p.exists().then_some(p)
    }

    /// Resolve the wallpaper path (supports both relative and absolute).
    pub fn wallpaper_path(&self) -> Option<PathBuf> {
        let wp = self.config.wallpaper.as_ref()?;
        let p = Path::new(&wp.path);
        if p.is_absolute() {
            Some(p.to_path_buf())
        } else {
            Some(self.dir.join(p))
        }
    }
}

// ── ThemeManager ──────────────────────────────────────────────────────────────

type ThemeChangedCallback = Box<dyn Fn(&Theme) + 'static>;

pub struct ThemeManager {
    current:    Theme,
    themes_dir: PathBuf,
    observers:  Vec<ThemeChangedCallback>,
}

impl ThemeManager {
    pub fn new() -> Self {
        let themes_dir = themes_directory();
        let current = load_or_default(&themes_dir);
        Self { current, themes_dir, observers: Vec::new() }
    }

    pub fn current(&self) -> &Theme {
        &self.current
    }

    pub fn themes_dir(&self) -> &Path {
        &self.themes_dir
    }

    /// Switch to a named theme. The name `"default"` always loads the built-in.
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

    /// Reload the active theme from disk. No-op for the built-in default.
    pub fn reload(&mut self) -> Result<(), ThemeError> {
        let name = self.current.name.clone();
        self.set_theme(&name)
    }

    /// All theme names available on disk, plus "default".
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

    /// Register a callback that is invoked after every theme switch.
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
    fn default() -> Self { Self::new() }
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

// ── Helpers ───────────────────────────────────────────────────────────────────

fn themes_directory() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config").join("meridian").join("themes")
}

fn load_or_default(themes_dir: &Path) -> Theme {
    let dir = themes_dir.join("default");
    if dir.join("theme.toml").exists() {
        match Theme::load_from_dir("default", &dir) {
            Ok(t) => {
                info!("Loaded theme \"default\" from {:?}", dir);
                return t;
            }
            Err(e) => warn!("Failed to load theme from {:?}: {} — using built-in", dir, e),
        }
    }
    info!("Using built-in default theme (Catppuccin Mocha)");
    Theme::builtin_default()
}
