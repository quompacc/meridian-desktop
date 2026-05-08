use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::keybind::{KeybindConfig, KeybindToml};
use tracing::{info, warn};

pub struct MeridianConfig {
    pub keybinds: KeybindConfig,
}

impl MeridianConfig {
    pub fn load() -> Self {
        let config_dir = config_directory();
        let config_path = config_dir.join("config.toml");

        if config_path.exists() {
            match Self::load_from(&config_path) {
                Ok(config) => {
                    info!("Loaded config from {:?}", config_path);
                    return config;
                }
                Err(err) => {
                    warn!("Failed to load config from {:?}: {} — using defaults", config_path, err);
                }
            }
        }

        info!("Using default config (no {:?} found)", config_path);
        Self::default()
    }

    pub fn reload(&mut self) -> Result<(), String> {
        let config_dir = config_directory();
        let config_path = config_dir.join("config.toml");

        if !config_path.exists() {
            self.keybinds = KeybindConfig::default();
            return Ok(());
        }

        let new = Self::load_from(&config_path)?;
        self.keybinds = new.keybinds;
        info!("Reloaded config from {:?}", config_path);
        Ok(())
    }

    fn load_from(path: &Path) -> Result<Self, String> {
        let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let toml: KeybindToml =
            toml::from_str(&raw).map_err(|e| format!("TOML parse error: {}", e))?;

        let keybinds = if toml.keybinds.is_empty() {
            KeybindConfig::default()
        } else {
            KeybindConfig::from_map(&toml.keybinds)?
        };

        Ok(Self { keybinds })
    }
}

impl Default for MeridianConfig {
    fn default() -> Self {
        Self {
            keybinds: KeybindConfig::default(),
        }
    }
}

fn config_directory() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config").join("meridian")
}
