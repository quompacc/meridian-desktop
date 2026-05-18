use std::fmt;

use serde::Deserialize;

use super::Color;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ThemeColors {
    pub background: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub accent: Color,
    pub accent_alt: Color,
    pub text: Color,
    pub text_dim: Color,
    pub border: Color,
    pub error: Color,
    pub warning: Color,
    pub success: Color,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            background: Color::rgb(0x1a, 0x1b, 0x26),
            surface: Color::rgb(0x24, 0x28, 0x3b),
            surface_alt: Color::rgb(0x1f, 0x23, 0x35),
            accent: Color::rgb(0x7a, 0xa2, 0xf7),
            accent_alt: Color::rgb(0xbb, 0x9a, 0xf7),
            text: Color::rgb(0xc0, 0xca, 0xf5),
            text_dim: Color::rgb(0xa9, 0xb1, 0xd6),
            border: Color::rgb(0x41, 0x48, 0x68),
            error: Color::rgb(0xf7, 0x76, 0x8e),
            warning: Color::rgb(0xe0, 0xaf, 0x68),
            success: Color::rgb(0x9e, 0xce, 0x6a),
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
    pub shadow_radius_top: u32,
    pub shadow_alpha: f32,
    pub shadow_offset_y: i32,
    pub gap: u32,
}

impl Default for Decorations {
    fn default() -> Self {
        Self {
            border_width: 1,
            corner_radius: 0,
            shadow: false,
            shadow_radius: 16,
            shadow_radius_top: 8,
            shadow_alpha: 0.18,
            shadow_offset_y: 0,
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
            ui: "Adwaita Sans 11".to_string(),
            mono: "Adwaita Mono 10".to_string(),
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
            theme: "Vanilla-DMZ".to_string(),
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

#[cfg(test)]
mod tests {
    use super::{Color, Decorations, Fonts, ThemeColors, ThemeConfig};

    #[test]
    fn test_theme_colors_default_tokyo_night() {
        let colors = ThemeColors::default();
        assert_eq!(colors.background, Color::rgb(0x1a, 0x1b, 0x26));
        assert_eq!(colors.surface, Color::rgb(0x24, 0x28, 0x3b));
        assert_eq!(colors.surface_alt, Color::rgb(0x1f, 0x23, 0x35));
        assert_eq!(colors.accent, Color::rgb(0x7a, 0xa2, 0xf7));
        assert_eq!(colors.accent_alt, Color::rgb(0xbb, 0x9a, 0xf7));
        assert_eq!(colors.text, Color::rgb(0xc0, 0xca, 0xf5));
        assert_eq!(colors.text_dim, Color::rgb(0xa9, 0xb1, 0xd6));
        assert_eq!(colors.border, Color::rgb(0x41, 0x48, 0x68));
        assert_eq!(colors.error, Color::rgb(0xf7, 0x76, 0x8e));
        assert_eq!(colors.warning, Color::rgb(0xe0, 0xaf, 0x68));
        assert_eq!(colors.success, Color::rgb(0x9e, 0xce, 0x6a));
    }

    #[test]
    fn test_decorations_default_soft_form() {
        let decorations = Decorations::default();
        assert_eq!(decorations.border_width, 1);
        assert_eq!(decorations.corner_radius, 0);
        assert!(!decorations.shadow);
        assert_eq!(decorations.shadow_radius, 16);
        assert_eq!(decorations.shadow_radius_top, 8);
        assert_eq!(decorations.shadow_alpha, 0.18);
        assert_eq!(decorations.shadow_offset_y, 0);
        assert_eq!(decorations.gap, 8);
    }

    #[test]
    fn test_theme_config_partial_toml_fills_new_defaults() {
        let config: ThemeConfig = toml::from_str(
            r##"
            [colors]
            background = "#000000"
            "##,
        )
        .expect("partial theme config should deserialize");

        assert_eq!(config.colors.background, Color::rgb(0x00, 0x00, 0x00));
        assert_eq!(config.colors.surface, Color::rgb(0x24, 0x28, 0x3b));
        assert_eq!(config.colors.surface_alt, Color::rgb(0x1f, 0x23, 0x35));
        assert_eq!(config.colors.accent, Color::rgb(0x7a, 0xa2, 0xf7));
        assert_eq!(config.colors.accent_alt, Color::rgb(0xbb, 0x9a, 0xf7));
        assert_eq!(config.colors.text, Color::rgb(0xc0, 0xca, 0xf5));
        assert_eq!(config.colors.text_dim, Color::rgb(0xa9, 0xb1, 0xd6));
        assert_eq!(config.colors.border, Color::rgb(0x41, 0x48, 0x68));
        assert_eq!(config.colors.error, Color::rgb(0xf7, 0x76, 0x8e));
        assert_eq!(config.colors.warning, Color::rgb(0xe0, 0xaf, 0x68));
        assert_eq!(config.colors.success, Color::rgb(0x9e, 0xce, 0x6a));
        assert_eq!(config.decorations.border_width, 1);
        assert_eq!(config.decorations.corner_radius, 0);
        assert!(!config.decorations.shadow);
        assert_eq!(config.decorations.shadow_radius, 16);
        assert_eq!(config.decorations.shadow_radius_top, 8);
        assert_eq!(config.decorations.shadow_alpha, 0.18);
        assert_eq!(config.decorations.shadow_offset_y, 0);
        assert_eq!(config.decorations.gap, 8);
    }

    #[test]
    fn test_fonts_default_uses_adwaita() {
        let fonts = Fonts::default();
        assert_eq!(fonts.ui, "Adwaita Sans 11");
        assert_eq!(fonts.mono, "Adwaita Mono 10");
    }
}
