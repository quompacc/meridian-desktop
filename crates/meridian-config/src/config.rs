use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{
    keybind::KeybindConfig,
    output::{OutputEntry, OutputToml},
    theme::{Wallpaper, WallpaperMode},
};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct GeneralConfig {
    pub theme: String,
    /// Screen blank timeout in seconds. None = disabled.
    pub idle_timeout_secs: Option<u64>,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            idle_timeout_secs: Some(300),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CursorConfig {
    pub theme: String,
    pub size: u32,
}

impl Default for CursorConfig {
    fn default() -> Self {
        Self {
            // Meridian defaults to KDE Breeze so the desktop cursor
            // matches the small white login cursor by default. Source-only
            // builds without Breeze installed fall back through the compositor
            // cursor loader.
            theme: "Breeze_Light".to_string(),
            size: 24,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WallpaperConfig {
    pub path: String,
    pub mode: WallpaperMode,
}

impl Default for WallpaperConfig {
    fn default() -> Self {
        Self {
            path: String::new(),
            mode: WallpaperMode::Fill,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PinnedAppConfig {
    pub label: String,
    pub program: String,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PanelConfig {
    pub pinned: Vec<PinnedAppConfig>,
}

/// A wallpaper entry returned by scan_wallpaper_dirs.
/// Multiple resolution variants of the same pack are collapsed into one entry.
#[derive(Debug, Clone)]
pub struct WallpaperEntry {
    pub display_name: String,
    /// Best-quality image to apply (largest file in the group).
    pub apply_path: String,
    /// Smallest file — used for fast thumbnail decoding.
    pub thumbnail_path: String,
}

#[derive(Default)]
pub struct MeridianConfig {
    pub keybinds: KeybindConfig,
    pub general: GeneralConfig,
    pub cursor: Option<CursorConfig>,
    pub wallpaper: Option<WallpaperConfig>,
    pub outputs: Vec<OutputEntry>,
    pub panel: PanelConfig,
}

impl MeridianConfig {
    pub fn load() -> Self {
        let config_path = config_directory().join("config.toml");
        Self::load_or_default_from_path(&config_path)
    }

    pub fn reload(&mut self) -> Result<(), String> {
        let config_path = config_directory().join("config.toml");
        self.reload_from_path(&config_path)
    }

    pub fn reload_from_path(&mut self, path: &Path) -> Result<(), String> {
        if !path.exists() {
            info!("config file not found; using defaults ({:?})", path);
            *self = Self::default();
            return Ok(());
        }

        *self = Self::load_from(path)?;
        info!("Reloaded config from {:?}", path);
        Ok(())
    }

    fn load_from(path: &Path) -> Result<Self, String> {
        let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let toml: MeridianToml =
            toml::from_str(&raw).map_err(|e| format!("TOML parse error: {}", e))?;

        let keybinds = if toml.keybinds.is_empty() {
            KeybindConfig::default()
        } else {
            KeybindConfig::from_map(&toml.keybinds)?
        };

        Ok(Self {
            keybinds,
            general: GeneralConfig {
                theme: toml.general.theme,
                idle_timeout_secs: toml.general.idle_timeout_secs,
            },
            cursor: toml.cursor.map(|cursor| CursorConfig {
                theme: cursor.theme,
                size: cursor.size,
            }),
            wallpaper: toml.wallpaper.map(|wallpaper| WallpaperConfig {
                path: wallpaper.path,
                mode: wallpaper.mode,
            }),
            outputs: toml
                .outputs
                .into_iter()
                .map(|(name, raw)| raw.into_entry(name))
                .collect::<Result<Vec<_>, String>>()?,
            panel: PanelConfig {
                pinned: toml
                    .panel
                    .pinned
                    .into_iter()
                    .map(|app| PinnedAppConfig {
                        label: app.label,
                        program: app.program,
                        icon: app.icon,
                    })
                    .collect(),
            },
        })
    }

    pub fn wallpaper_override(&self) -> Option<Wallpaper> {
        let wallpaper = self.wallpaper.as_ref()?;
        Some(Wallpaper {
            path: wallpaper.path.clone(),
            mode: wallpaper.mode,
        })
    }

    fn load_or_default_from_path(path: &Path) -> Self {
        if !path.exists() {
            info!("config file not found; using defaults ({:?})", path);
            return Self::default();
        }

        match Self::load_from(path) {
            Ok(config) => {
                info!("Loaded config from {:?}", path);
                config
            }
            Err(err) => {
                warn!(
                    "Failed to load config from {:?}: {} — using defaults",
                    path, err
                );
                Self::default()
            }
        }
    }
}

fn config_directory() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config").join("meridian")
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PinnedAppToml {
    label: String,
    program: String,
    #[serde(default)]
    icon: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PanelToml {
    pinned: Vec<PinnedAppToml>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct MeridianToml {
    keybinds: HashMap<String, String>,
    general: GeneralToml,
    cursor: Option<CursorToml>,
    wallpaper: Option<WallpaperToml>,
    outputs: BTreeMap<String, OutputToml>,
    panel: PanelToml,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct GeneralToml {
    theme: String,
    #[serde(default)]
    idle_timeout_secs: Option<u64>,
}

impl Default for GeneralToml {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            idle_timeout_secs: Some(300),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct CursorToml {
    theme: String,
    size: u32,
}

impl Default for CursorToml {
    fn default() -> Self {
        Self {
            theme: "Breeze_Light".to_string(),
            size: 24,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct WallpaperToml {
    path: String,
    mode: WallpaperMode,
}

impl Default for WallpaperToml {
    fn default() -> Self {
        Self {
            path: String::new(),
            mode: WallpaperMode::Fill,
        }
    }
}

// Keep the large config parser/save tests near the parser fixtures they cover;
// moving them below the save/wallpaper helpers would be a noisy-only diff.
#[cfg_attr(test, allow(clippy::items_after_test_module))]
#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;

include!("config/mutation.rs");
include!("config/output_toml.rs");
include!("config/wallpapers.rs");
