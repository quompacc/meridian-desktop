use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{
    keybind::KeybindConfig,
    theme::{Wallpaper, WallpaperMode},
};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct GeneralConfig {
    pub theme: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
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
            theme: "default".to_string(),
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

pub struct MeridianConfig {
    pub keybinds: KeybindConfig,
    pub general: GeneralConfig,
    pub cursor: Option<CursorConfig>,
    pub wallpaper: Option<WallpaperConfig>,
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
            },
            cursor: toml.cursor.map(|cursor| CursorConfig {
                theme: cursor.theme,
                size: cursor.size,
            }),
            wallpaper: toml.wallpaper.map(|wallpaper| WallpaperConfig {
                path: wallpaper.path,
                mode: wallpaper.mode,
            }),
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

impl Default for MeridianConfig {
    fn default() -> Self {
        Self {
            keybinds: KeybindConfig::default(),
            general: GeneralConfig::default(),
            cursor: None,
            wallpaper: None,
        }
    }
}

fn config_directory() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config").join("meridian")
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct MeridianToml {
    keybinds: HashMap<String, String>,
    general: GeneralToml,
    cursor: Option<CursorToml>,
    wallpaper: Option<WallpaperToml>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct GeneralToml {
    theme: String,
}

impl Default for GeneralToml {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
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
            theme: "default".to_string(),
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

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use super::{MeridianConfig, WallpaperMode};

    #[test]
    fn missing_file_uses_defaults() {
        let path = unique_test_path("missing.toml");
        let config = MeridianConfig::load_or_default_from_path(&path);
        assert_eq!(config.general.theme, "default");
        assert!(config.cursor.is_none());
        assert!(config.wallpaper.is_none());
    }

    #[test]
    fn valid_toml_parses_general_cursor_and_wallpaper() {
        let path = unique_test_path("valid.toml");
        write(
            &path,
            r#"
[general]
theme = "catppuccin-mocha"

[cursor]
theme = "default"
size = 24

[wallpaper]
path = "/tmp/wall.png"
mode = "fill"
"#,
        );

        let config = MeridianConfig::load_from(&path).expect("valid config");
        assert_eq!(config.general.theme, "catppuccin-mocha");
        let cursor = config.cursor.expect("cursor section");
        assert_eq!(cursor.theme, "default");
        assert_eq!(cursor.size, 24);
        let wallpaper = config.wallpaper.expect("wallpaper section");
        assert_eq!(wallpaper.path, "/tmp/wall.png");
        assert_eq!(wallpaper.mode, WallpaperMode::Fill);
    }

    #[test]
    fn invalid_toml_falls_back_to_defaults() {
        let path = unique_test_path("invalid.toml");
        write(&path, r#"[general theme = "broken""#);
        let config = MeridianConfig::load_or_default_from_path(&path);
        assert_eq!(config.general.theme, "default");
        assert!(config.cursor.is_none());
        assert!(config.wallpaper.is_none());
    }

    #[test]
    fn cursor_and_wallpaper_modes_parse() {
        let path = unique_test_path("cursor-wallpaper.toml");
        write(
            &path,
            r#"
[cursor]
theme = "Breeze"
size = 32

[wallpaper]
path = "~/wallpapers/space.png"
mode = "fit"
"#,
        );
        let config = MeridianConfig::load_from(&path).expect("valid config");
        let cursor = config.cursor.expect("cursor");
        assert_eq!(cursor.theme, "Breeze");
        assert_eq!(cursor.size, 32);
        let wallpaper = config.wallpaper.expect("wallpaper");
        assert_eq!(wallpaper.path, "~/wallpapers/space.png");
        assert_eq!(wallpaper.mode, WallpaperMode::Fit);
    }

    #[test]
    fn reload_from_path_with_valid_file_updates_all_sections() {
        let path = unique_test_path("reload-valid.toml");
        write(
            &path,
            r#"
[general]
theme = "catppuccin-mocha"

[cursor]
theme = "default"
size = 28

[wallpaper]
path = ""
mode = "tile"
"#,
        );

        let mut config = MeridianConfig::default();
        config.reload_from_path(&path).expect("reload valid");
        assert_eq!(config.general.theme, "catppuccin-mocha");
        let cursor = config.cursor.expect("cursor");
        assert_eq!(cursor.theme, "default");
        assert_eq!(cursor.size, 28);
        let wallpaper = config.wallpaper.expect("wallpaper");
        assert_eq!(wallpaper.path, "");
        assert_eq!(wallpaper.mode, WallpaperMode::Tile);
    }

    #[test]
    fn reload_from_path_with_invalid_file_returns_error_and_preserves_old_config() {
        let path = unique_test_path("reload-invalid.toml");
        write(&path, r#"[general theme = "broken""#);

        let mut config = MeridianConfig::default();
        config.general.theme = "old-theme".to_string();
        config.cursor = Some(super::CursorConfig {
            theme: "old-cursor".to_string(),
            size: 31,
        });

        let result = config.reload_from_path(&path);
        assert!(result.is_err());
        assert_eq!(config.general.theme, "old-theme");
        let cursor = config.cursor.expect("cursor preserved");
        assert_eq!(cursor.theme, "old-cursor");
        assert_eq!(cursor.size, 31);
    }

    #[test]
    fn reload_from_path_with_missing_file_resets_to_defaults() {
        let path = unique_test_path("reload-missing.toml");
        let mut config = MeridianConfig::default();
        config.general.theme = "custom".to_string();
        config.cursor = Some(super::CursorConfig {
            theme: "custom-cursor".to_string(),
            size: 33,
        });

        config.reload_from_path(&path).expect("reload missing");
        assert_eq!(config.general.theme, "default");
        assert!(config.cursor.is_none());
        assert!(config.wallpaper.is_none());
    }

    #[test]
    fn keybinds_section_remains_supported() {
        let path = unique_test_path("keybinds.toml");
        write(
            &path,
            r#"
[keybinds]
"Super+1" = "workspace 1"
"Super+Space" = "toggle-launcher"
"#,
        );
        let config = MeridianConfig::load_from(&path).expect("valid keybind config");
        assert!(config.keybinds.bindings().len() >= 2);
    }

    #[test]
    fn reload_from_path_with_invalid_keybind_keeps_previous_config() {
        let path = unique_test_path("invalid-keybind.toml");
        write(
            &path,
            r#"
[general]
theme = "default"

[keybinds]
"Super+NotARealKey" = "toggle-tiling"
"#,
        );

        let mut config = MeridianConfig::default();
        config.general.theme = "old-theme".to_string();

        let result = config.reload_from_path(&path);
        assert!(result.is_err());
        assert_eq!(config.general.theme, "old-theme");
    }

    fn unique_test_path(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "meridian-config-test-{}-{}-{}",
            std::process::id(),
            nanos,
            name
        ))
    }

    fn write(path: &std::path::Path, content: &str) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(path, content).expect("write test file");
    }
}
