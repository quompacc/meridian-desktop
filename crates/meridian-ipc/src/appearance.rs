use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppearanceTheme {
    Dark,
    Light,
}

impl AppearanceTheme {
    pub fn config_name(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppearanceWallpaperMode {
    Fill,
    Fit,
    Center,
    Tile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppearanceSnapshot {
    pub theme: AppearanceTheme,
    pub wallpaper_name: Option<String>,
    pub wallpaper_mode: AppearanceWallpaperMode,
}

impl AppearanceSnapshot {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.wallpaper_name.as_ref().is_some_and(|name| {
            name.is_empty() || name.len() > 256 || name.chars().any(char::is_control)
        }) {
            return Err("invalid wallpaper display name");
        }
        Ok(())
    }
}
