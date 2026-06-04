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
    // Defaults derive from the canonical palette in `meridian_tokens` — the
    // single source of truth. No hand-synced mirror (design-tokens audit
    // 2026-06-04). The per-field hex assertions in the tests below guard that
    // this stays equal to the documented Tokyo-Night-Metro spec.
    fn default() -> Self {
        let p = meridian_tokens::Palette::TOKYO_NIGHT_METRO;
        Self {
            background: p.background,
            surface: p.surface,
            surface_alt: p.surface_alt,
            accent: p.accent,
            accent_alt: p.accent_alt,
            text: p.text,
            text_dim: p.text_dim,
            border: p.border,
            error: p.error,
            warning: p.warning,
            success: p.success,
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
    /// Liquid-glass titlebar: translucent tinted fill with a specular top
    /// edge instead of an opaque bar. Blur-behind lands in a later phase.
    pub glass: bool,
    /// Base fill opacity of the glass titlebar (0.0..1.0).
    pub glass_alpha: f32,
    /// Specular top-edge highlight strength (0.0..~1.5).
    pub glass_specular: f32,
    /// Use the textured blur-behind glass (frosted) instead of the
    /// tint-only fallback. Requires `glass = true`.
    pub glass_blur: bool,
    /// Blur radius behind the glass, in physical pixels.
    pub glass_blur_radius: f32,
    /// How strongly the frosted background is pulled toward the tint
    /// colour (0.0 = clear glass, 1.0 = solid tint).
    pub glass_tint: f32,
    /// Opacity of the cool-white glass window frame (0.0..1.0).
    pub glass_frame_alpha: f32,
    /// Base opacity of the colour veil on the glass window buttons (0.0..1.0).
    pub glass_button_alpha: f32,
    /// Optional explicit tint colour for the frosted glass — the "blur
    /// accent". When `None` the glass tints toward the surface colour as
    /// before; when set, this colour drives the tint on every glass surface
    /// (titlebar, panel, launcher, popups), decoupling the blur accent from
    /// `surface` so it can be themed independently.
    pub glass_tint_color: Option<Color>,
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
            glass: false,
            glass_alpha: 0.55,
            glass_specular: 0.6,
            glass_blur: true,
            glass_blur_radius: 8.0,
            glass_tint: 0.5,
            glass_frame_alpha: 0.4,
            glass_button_alpha: 0.45,
            glass_tint_color: None,
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

impl Fonts {
    /// The UI font family with any trailing Pango-style point size stripped:
    /// "Adwaita Sans 11" -> "Adwaita Sans". Used to resolve the active font;
    /// the size itself is not yet honoured (the render scale is fixed).
    pub fn ui_family(&self) -> &str {
        let trimmed = self.ui.trim();
        match trimmed.rsplit_once(' ') {
            Some((head, tail))
                if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit() || c == '.') =>
            {
                head.trim_end()
            }
            _ => trimmed,
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
            theme: "Breeze_Light".to_string(),
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
    use super::{Color, Cursor, Decorations, Fonts, ThemeColors, ThemeConfig};

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
    fn test_glass_tint_color_defaults_none_and_parses() {
        // No-regress contract for the blur-accent decoupling: absent means
        // None (tint follows surface as before); a hex value parses to Some.
        assert_eq!(Decorations::default().glass_tint_color, None);

        let config: ThemeConfig = toml::from_str(
            r##"
            [decorations]
            glass_tint_color = "#5b9bd5"
            "##,
        )
        .expect("decorations with glass_tint_color should deserialize");
        assert_eq!(
            config.decorations.glass_tint_color,
            Some(Color::rgb(0x5b, 0x9b, 0xd5))
        );
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

    #[test]
    fn ui_family_strips_trailing_size() {
        let mk = |ui: &str| Fonts {
            ui: ui.to_string(),
            mono: String::new(),
        };
        assert_eq!(mk("Adwaita Sans 11").ui_family(), "Adwaita Sans");
        assert_eq!(mk("Noto Sans 13.5").ui_family(), "Noto Sans");
        assert_eq!(mk("Inter").ui_family(), "Inter");
        assert_eq!(mk("  Cantarell 12  ").ui_family(), "Cantarell");
    }

    #[test]
    fn test_cursor_default_uses_installed_theme() {
        let cursor = Cursor::default();
        assert_eq!(cursor.theme, "Breeze_Light");
        assert_eq!(cursor.size, 24);
    }
}
