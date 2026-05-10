use std::fmt;

use serde::Deserialize;

use super::Color;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ThemeColors {
    pub background: Color,
    pub surface: Color,
    pub accent: Color,
    pub text: Color,
    pub border: Color,
    pub error: Color,
    pub warning: Color,
    pub success: Color,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            background: Color::rgb(0x1e, 0x1e, 0x2e),
            surface: Color::rgb(0x31, 0x32, 0x44),
            accent: Color::rgb(0xcb, 0xa6, 0xf7),
            text: Color::rgb(0xcd, 0xd6, 0xf4),
            border: Color::rgb(0x45, 0x47, 0x5a),
            error: Color::rgb(0xf3, 0x8b, 0xa8),
            warning: Color::rgb(0xfa, 0xb3, 0x87),
            success: Color::rgb(0xa6, 0xe3, 0xa1),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Decorations {
    pub border_width: u32,
    pub corner_radius: u32,
    pub shadow: bool,
    pub shadow_radius: u32,
    pub gap: u32,
}

impl Default for Decorations {
    fn default() -> Self {
        Self {
            border_width: 2,
            corner_radius: 8,
            shadow: true,
            shadow_radius: 12,
            gap: 8,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Fonts {
    pub ui: String,
    pub mono: String,
}

impl Default for Fonts {
    fn default() -> Self {
        Self {
            ui: "Inter 11".to_string(),
            mono: "JetBrains Mono 10".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Icons {
    pub theme: String,
}

impl Default for Icons {
    fn default() -> Self {
        Self {
            theme: "Papirus-Dark".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Cursor {
    pub theme: String,
    pub size: u32,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
            size: 24,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WallpaperMode {
    #[default]
    Fill,
    Fit,
    Center,
    Tile,
}

impl fmt::Display for WallpaperMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Fill => "fill",
            Self::Fit => "fit",
            Self::Center => "center",
            Self::Tile => "tile",
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Wallpaper {
    pub path: String,
    #[serde(default)]
    pub mode: WallpaperMode,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ThemeConfig {
    pub colors: ThemeColors,
    pub decorations: Decorations,
    pub fonts: Fonts,
    pub icons: Icons,
    pub cursor: Cursor,
    pub wallpaper: Option<Wallpaper>,
}
