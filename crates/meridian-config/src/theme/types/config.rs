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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeSurface {
    Panel,
    Launcher,
    Popup,
    Modal,
    Control,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceTreatment {
    pub radius: f32,
    pub tint_amount: f32,
    pub blur_radius: f32,
    pub fill_alpha: u8,
    pub frame_alpha: u8,
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

impl Decorations {
    pub fn surface_radius(&self, surface: ThemeSurface) -> f32 {
        let base = self.corner_radius as f32;
        if base <= 0.0 {
            return 0.0;
        }
        match surface {
            ThemeSurface::Panel | ThemeSurface::Launcher => base + 2.0,
            ThemeSurface::Popup | ThemeSurface::Modal => base + 4.0,
            ThemeSurface::Control => (base * 0.8).round().max(1.0),
        }
    }

    pub fn surface_treatment(&self, surface: ThemeSurface) -> SurfaceTreatment {
        if !self.glass {
            return SurfaceTreatment {
                radius: self.surface_radius(surface),
                tint_amount: 1.0,
                blur_radius: 0.0,
                fill_alpha: 0xff,
                frame_alpha: 0xff,
            };
        }

        let tint_amount = match surface {
            ThemeSurface::Panel => (self.glass_tint * 0.35).clamp(0.0, 0.35),
            ThemeSurface::Launcher => (self.glass_tint * 0.45).clamp(0.0, 0.45),
            ThemeSurface::Popup => (self.glass_tint * 0.38).clamp(0.0, 0.38),
            ThemeSurface::Modal | ThemeSurface::Control => self.glass_tint.clamp(0.0, 1.0),
        };
        let blur_radius = if self.glass_blur {
            match surface {
                ThemeSurface::Popup => (self.glass_blur_radius * 0.45).max(2.0),
                _ => self.glass_blur_radius.max(0.0),
            }
        } else {
            0.0
        };
        let fill_scale = match surface {
            ThemeSurface::Panel => 0.52,
            _ => 1.0,
        };

        SurfaceTreatment {
            radius: self.surface_radius(surface),
            tint_amount,
            blur_radius,
            fill_alpha: alpha_byte(self.glass_alpha * fill_scale),
            frame_alpha: alpha_byte(self.glass_frame_alpha),
        }
    }
}

fn alpha_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Fonts {
    pub ui: String,
}

impl Default for Fonts {
    fn default() -> Self {
        Self {
            ui: "Adwaita Sans 11".to_string(),
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

impl ThemeConfig {
    pub fn glass_tint_color(&self) -> Color {
        self.decorations
            .glass_tint_color
            .unwrap_or(self.colors.surface_alt)
    }

    pub fn appearance_is_light(&self) -> bool {
        let bg = self.colors.background;
        let lum = 0.299 * bg.r as f32 + 0.587 * bg.g as f32 + 0.114 * bg.b as f32;
        lum > 140.0
    }
}

#[cfg(test)]
mod tests {
    use super::{Color, Cursor, Decorations, Fonts, ThemeColors, ThemeConfig, ThemeSurface};

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
    fn surface_treatment_uses_theme_radius_and_glass_alpha() {
        let decorations = Decorations {
            corner_radius: 10,
            glass: true,
            glass_alpha: 0.5,
            glass_frame_alpha: 0.25,
            glass_tint: 0.6,
            glass_blur_radius: 12.0,
            ..Decorations::default()
        };

        let modal = decorations.surface_treatment(ThemeSurface::Modal);
        assert_eq!(modal.radius, 14.0);
        assert_eq!(modal.fill_alpha, 128);
        assert_eq!(modal.frame_alpha, 64);
        assert_eq!(modal.tint_amount, 0.6);
        assert_eq!(modal.blur_radius, 12.0);
        assert_eq!(decorations.surface_radius(ThemeSurface::Control), 8.0);

        let panel = decorations.surface_treatment(ThemeSurface::Panel);
        assert_eq!(panel.radius, 12.0);
        assert_eq!(panel.fill_alpha, 66);
        assert!((panel.tint_amount - 0.21).abs() < 1e-6);

        let popup = decorations.surface_treatment(ThemeSurface::Popup);
        assert_eq!(popup.radius, 14.0);
        assert!((popup.blur_radius - 5.4).abs() < 1e-6);
    }

    #[test]
    fn surface_treatment_non_glass_is_opaque_without_blur() {
        let decorations = Decorations {
            corner_radius: 0,
            glass: false,
            glass_blur: true,
            ..Decorations::default()
        };

        let popup = decorations.surface_treatment(ThemeSurface::Popup);
        assert_eq!(popup.radius, 0.0);
        assert_eq!(popup.fill_alpha, 0xff);
        assert_eq!(popup.frame_alpha, 0xff);
        assert_eq!(popup.tint_amount, 1.0);
        assert_eq!(popup.blur_radius, 0.0);
    }

    #[test]
    fn theme_config_appearance_tracks_background_luminance() {
        let mut config = ThemeConfig::default();
        config.colors.background = Color::rgb(0xf0, 0xf0, 0xf0);
        assert!(config.appearance_is_light());
        config.colors.background = Color::rgb(0x10, 0x18, 0x20);
        assert!(!config.appearance_is_light());
    }

    #[test]
    fn test_fonts_default_uses_adwaita() {
        let fonts = Fonts::default();
        assert_eq!(fonts.ui, "Adwaita Sans 11");
    }

    #[test]
    fn ui_family_strips_trailing_size() {
        let mk = |ui: &str| Fonts { ui: ui.to_string() };
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
