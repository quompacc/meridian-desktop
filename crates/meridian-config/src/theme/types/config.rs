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
        let p = meridian_tokens::Palette::DARK;
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
    /// Opacity of the cool-white glass window frame (0.0..1.0).
    pub glass_frame_alpha: f32,
    /// Base opacity of the colour veil on the glass window buttons (0.0..1.0).
    pub glass_button_alpha: f32,
    /// Opacity of the hairline dividers between the glass-titlebar button zones
    /// (0.0..1.0). Central so the compositor no longer hardcodes it.
    pub glass_divider_alpha: f32,
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
    // THE single source of truth for every non-colour decoration value. Both
    // shipped themes (`dark`/`light`) carry only `[colors]`, so these defaults
    // define the shared geometry/glass treatment for all of them. The glass
    // values follow the "andeutung" target measured from the reference mockups
    // (GUI_CENTRALIZATION_PLAN §5): nearly opaque fills, a faint blur and a
    // light tint instead of the old heavy frosted glass.
    fn default() -> Self {
        Self {
            border_width: 1,
            corner_radius: 10,
            shadow: true,
            shadow_radius: 22,
            shadow_radius_top: 11,
            shadow_alpha: 0.35,
            shadow_offset_y: 3,
            gap: 8,
            glass: true,
            // High = opaque frosted surface (the launcher look the user signed
            // off on). Transparency stays a hint; the frost does the work.
            glass_alpha: 0.92,
            glass_specular: 0.6,
            glass_blur: true,
            // Strong frosted blur — the surfaces must read as milk-glass, not
            // clear glass. (User feedback: blur far too weak at 3.)
            glass_blur_radius: 20.0,
            glass_frame_alpha: 0.4,
            glass_button_alpha: 0.45,
            glass_divider_alpha: 0.30,
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

        // ONE solidity source for the whole desktop. Panel/launcher get their
        // look from the shell painting a body at `glass_alpha`; the glass panes
        // (titlebar, popups, modals) must read equally solid, so they tint toward
        // the surface colour by the SAME `glass_alpha`. No separate tint knob, no
        // per-surface scaling — every element is identical but for its corner
        // radius (layout). Move `glass_alpha` and every surface moves together.
        let tint_amount = self.glass_alpha.clamp(0.0, 1.0);
        let blur_radius = if self.glass_blur {
            self.glass_blur_radius.max(0.0)
        } else {
            0.0
        };
        // `glass_alpha` is the single opacity knob for every glass surface —
        // panel, launcher, popup and titlebar all land at the same fill so the
        // desktop reads consistently (the launcher look). No per-surface scale.
        SurfaceTreatment {
            radius: self.surface_radius(surface),
            tint_amount,
            blur_radius,
            fill_alpha: alpha_byte(self.glass_alpha),
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
    // Central UI-font default. Both shipped themes used "Inter 11"; with the
    // 2-theme model the font is a shared non-colour default, not per-theme.
    fn default() -> Self {
        Self {
            ui: "Inter 11".to_string(),
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
    fn test_theme_colors_default_is_dark_palette() {
        // Defaults derive from the single source `Palette::DARK`.
        let colors = ThemeColors::default();
        assert_eq!(colors.background, Color::rgb(0x14, 0x17, 0x1b));
        assert_eq!(colors.surface, Color::rgb(0x20, 0x25, 0x2b));
        assert_eq!(colors.surface_alt, Color::rgb(0x1b, 0x1f, 0x24));
        assert_eq!(colors.accent, Color::rgb(0x4e, 0x99, 0xf3));
        assert_eq!(colors.accent_alt, Color::rgb(0x43, 0x83, 0xce));
        assert_eq!(colors.text, Color::rgb(0xdc, 0xde, 0xe1));
        assert_eq!(colors.text_dim, Color::rgb(0x88, 0x8d, 0x93));
        assert_eq!(colors.border, Color::rgb(0x35, 0x3a, 0x40));
        assert_eq!(colors.error, Color::rgb(0xb5, 0x68, 0x5c));
        assert_eq!(colors.warning, Color::rgb(0xb8, 0x9a, 0x6a));
        assert_eq!(colors.success, Color::rgb(0x6f, 0xa0, 0x8c));
    }

    #[test]
    fn test_decorations_default_central_glass() {
        // The one place non-colour decoration values live. Glass tuned to the
        // "andeutung" target (mockup): near-opaque fill, faint blur+tint.
        let decorations = Decorations::default();
        assert_eq!(decorations.border_width, 1);
        assert_eq!(decorations.corner_radius, 10);
        assert!(decorations.shadow);
        assert_eq!(decorations.shadow_radius, 22);
        assert_eq!(decorations.shadow_radius_top, 11);
        assert_eq!(decorations.shadow_alpha, 0.35);
        assert_eq!(decorations.shadow_offset_y, 3);
        assert_eq!(decorations.gap, 8);
        assert!(decorations.glass);
        assert_eq!(decorations.glass_alpha, 0.92);
        assert_eq!(decorations.glass_blur_radius, 20.0);
        assert_eq!(decorations.glass_divider_alpha, 0.30);
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
        assert_eq!(config.colors.surface, Color::rgb(0x20, 0x25, 0x2b));
        assert_eq!(config.colors.surface_alt, Color::rgb(0x1b, 0x1f, 0x24));
        assert_eq!(config.colors.accent, Color::rgb(0x4e, 0x99, 0xf3));
        assert_eq!(config.colors.accent_alt, Color::rgb(0x43, 0x83, 0xce));
        assert_eq!(config.colors.text, Color::rgb(0xdc, 0xde, 0xe1));
        assert_eq!(config.colors.text_dim, Color::rgb(0x88, 0x8d, 0x93));
        assert_eq!(config.colors.border, Color::rgb(0x35, 0x3a, 0x40));
        assert_eq!(config.colors.error, Color::rgb(0xb5, 0x68, 0x5c));
        assert_eq!(config.colors.warning, Color::rgb(0xb8, 0x9a, 0x6a));
        assert_eq!(config.colors.success, Color::rgb(0x6f, 0xa0, 0x8c));
        assert_eq!(config.decorations.border_width, 1);
        assert_eq!(config.decorations.corner_radius, 10);
        assert!(config.decorations.shadow);
        assert_eq!(config.decorations.shadow_radius, 22);
        assert_eq!(config.decorations.shadow_radius_top, 11);
        assert_eq!(config.decorations.shadow_alpha, 0.35);
        assert_eq!(config.decorations.shadow_offset_y, 3);
        assert_eq!(config.decorations.gap, 8);
    }

    #[test]
    fn surface_treatment_uses_theme_radius_and_glass_alpha() {
        let decorations = Decorations {
            corner_radius: 10,
            glass: true,
            glass_alpha: 0.5,
            glass_frame_alpha: 0.25,
            glass_blur_radius: 12.0,
            ..Decorations::default()
        };

        let modal = decorations.surface_treatment(ThemeSurface::Modal);
        assert_eq!(modal.radius, 14.0);
        assert_eq!(modal.fill_alpha, 128);
        assert_eq!(modal.frame_alpha, 64);
        // Tint solidity is driven by glass_alpha (the one knob), not a tint field.
        assert_eq!(modal.tint_amount, 0.5);
        assert_eq!(modal.blur_radius, 12.0);
        assert_eq!(decorations.surface_radius(ThemeSurface::Control), 8.0);

        let panel = decorations.surface_treatment(ThemeSurface::Panel);
        assert_eq!(panel.radius, 12.0);
        // Tint, blur and fill are uniform across every surface (only the radius
        // differs) — panel == modal == popup == titlebar.
        assert_eq!(panel.fill_alpha, 128);
        assert_eq!(panel.tint_amount, 0.5);
        assert_eq!(panel.blur_radius, 12.0);

        let popup = decorations.surface_treatment(ThemeSurface::Popup);
        assert_eq!(popup.radius, 14.0);
        assert_eq!(popup.tint_amount, 0.5);
        assert_eq!(popup.blur_radius, 12.0);
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
    fn test_fonts_default_uses_inter() {
        let fonts = Fonts::default();
        assert_eq!(fonts.ui, "Inter 11");
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
