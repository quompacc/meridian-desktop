//! Core colour primitive and the canonical Meridian palette.
//!
//! This is the single source of truth: `meridian-config` (theme deserialization)
//! and `meridian-ui` (render tokens) both re-export `Color`/`Palette` from here.
//! There is no second copy to hand-sync — see the design-tokens audit (2026-06-04).
//!
//! guard:allow-file: kanonische Single-Source der Palette; die rohen Hex-Farben
//! der Themes (DARK/LIGHT/default) werden hier *definiert*.

use std::{fmt, str::FromStr};

use serde::Deserialize;

/// 8-bit-per-channel RGBA colour. `Copy` so it passes through the render loop
/// without heap traffic. `Hash`/`Eq` so it can key caches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 0xff)
    }

    /// Channels as linear-ish 0..1 floats (r, g, b, a) for shader/GL paths.
    pub fn as_f32_array(self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }

    /// `#rrggbb` (opaque) or `#rrggbbaa` (translucent) hex string.
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    /// Linear interpolation towards `other` by `t` in [0,1]. Alpha is taken
    /// from `self` (the base), matching the render-loop tinting convention.
    pub fn lerp(&self, other: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let lerp_u8 = |x: u8, y: u8| -> u8 {
            (x as f32 + (y as f32 - x as f32) * t)
                .round()
                .clamp(0.0, 255.0) as u8
        };
        Color {
            r: lerp_u8(self.r, other.r),
            g: lerp_u8(self.g, other.g),
            b: lerp_u8(self.b, other.b),
            a: self.a,
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
        // `is_ascii()` guard: byte() slices `&s[i..i+2]` by byte index, which
        // would panic on a non-char-boundary. Hex digits are ASCII, so for any
        // ASCII string byte length == char count and the slices are always on
        // boundaries; non-ASCII input falls through to the graceful Err arm
        // (e.g. a config value like "#€€" is 6 bytes but not hex).
        match s.len() {
            6 if s.is_ascii() => Ok(Self::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 if s.is_ascii() => Ok(Self::rgba(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            _ => Err(format!(
                "invalid color \"#{}\": expected 6 or 8 hex digits",
                s
            )),
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

/// Resolved colour palette. `Copy` - pass by value, never clone in the render
/// loop. `meridian-ui` re-exports this as `style::Palette`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
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

impl Palette {
    /// The single source of truth for the **dark** desktop theme. Sampled from
    /// the dark reference mockup (`assets/arch_desktop_mockup.png`); a neutral
    /// grey-blue. `meridian_config::ThemeColors::default()` derives from this,
    /// and `themes/dark/theme.toml` mirrors these values for human editing.
    /// The light theme (`LIGHT`) differs **only** in these colours — geometry,
    /// radii and glass treatment are shared central defaults.
    pub const DARK: Palette = Palette {
        background: Color::rgb(0x14, 0x17, 0x1b),
        surface: Color::rgb(0x20, 0x25, 0x2b),
        surface_alt: Color::rgb(0x1b, 0x1f, 0x24),
        accent: Color::rgb(0x4e, 0x99, 0xf3),
        accent_alt: Color::rgb(0x43, 0x83, 0xce),
        text: Color::rgb(0xdc, 0xde, 0xe1),
        text_dim: Color::rgb(0x88, 0x8d, 0x93),
        border: Color::rgb(0x35, 0x3a, 0x40),
        error: Color::rgb(0xb5, 0x68, 0x5c),
        warning: Color::rgb(0xb8, 0x9a, 0x6a),
        success: Color::rgb(0x6f, 0xa0, 0x8c),
    };

    /// The single source of truth for the **light** desktop theme. Sampled from
    /// the light reference mockup (`assets/arch_desktop_mockup_hell.png`). The
    /// only thing that differs from [`Palette::DARK`] is the colour table —
    /// see `themes/light/theme.toml`.
    pub const LIGHT: Palette = Palette {
        background: Color::rgb(0xec, 0xe4, 0xd3),
        surface: Color::rgb(0xf4, 0xef, 0xe3),
        surface_alt: Color::rgb(0xe3, 0xd9, 0xc4),
        accent: Color::rgb(0x2f, 0x62, 0x99),
        accent_alt: Color::rgb(0x24, 0x4f, 0x7d),
        text: Color::rgb(0x07, 0x11, 0x1c),
        text_dim: Color::rgb(0x26, 0x37, 0x46),
        border: Color::rgb(0xc9, 0xbc, 0xa0),
        error: Color::rgb(0x9a, 0x46, 0x36),
        warning: Color::rgb(0x8a, 0x6a, 0x30),
        success: Color::rgb(0x3f, 0x7d, 0x5e),
    };

    /// The historical Tokyo-Night-Metro palette. No longer a shipped theme;
    /// retained only as a fixture for `meridian-ui` render tests. Production
    /// defaults come from [`Palette::DARK`].
    pub const TOKYO_NIGHT_METRO: Palette = Palette {
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
    };

    /// Near-black foreground for placing text/icons on a LIGHT background or
    /// accent. Named once so the contrast pair isn't a magic value per call site.
    pub const TEXT_ON_LIGHT: Color = Color::rgb(0x05, 0x08, 0x0c);
    /// Near-white foreground for placing text/icons on a DARK background/accent.
    pub const TEXT_ON_DARK: Color = Color::rgb(0xf6, 0xf9, 0xff);
}

/// Perceived luminance (0..=255) via the Rec.601 weights. The ONE formula for
/// the whole codebase — `meridian-config` (`appearance_is_light`) and the shell
/// (`accent_foreground`) both used to hand-roll this.
pub fn relative_luminance(c: Color) -> f32 {
    0.299 * c.r as f32 + 0.587 * c.g as f32 + 0.114 * c.b as f32
}

/// Luminance above which a colour reads as "light" and wants dark foreground.
pub const LIGHT_LUMINANCE_THRESHOLD: f32 = 150.0;

/// The contrast foreground ([`Palette::TEXT_ON_LIGHT`]/[`Palette::TEXT_ON_DARK`])
/// for text/icons drawn on top of `bg`.
pub fn contrast_text(bg: Color) -> Color {
    if relative_luminance(bg) > LIGHT_LUMINANCE_THRESHOLD {
        Palette::TEXT_ON_LIGHT
    } else {
        Palette::TEXT_ON_DARK
    }
}

impl Default for Palette {
    /// The dark desktop theme is the built-in fallback.
    fn default() -> Self {
        Self::DARK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_sets_alpha_opaque() {
        let c = Color::rgb(0x12, 0x34, 0x56);
        assert_eq!(c.a, 0xff);
        assert_eq!((c.r, c.g, c.b), (0x12, 0x34, 0x56));
    }

    #[test]
    fn rgba_preserves_alpha() {
        assert_eq!(Color::rgba(0, 0, 0, 0x80).a, 0x80);
    }

    #[test]
    fn luminance_orders_dark_below_light() {
        assert!(
            relative_luminance(Color::rgb(0, 0, 0)) < relative_luminance(Color::rgb(255, 255, 255))
        );
        assert!(relative_luminance(Palette::DARK.background) < LIGHT_LUMINANCE_THRESHOLD);
        assert!(relative_luminance(Palette::LIGHT.background) > LIGHT_LUMINANCE_THRESHOLD);
    }

    #[test]
    fn contrast_text_picks_dark_on_light_and_white_on_dark() {
        assert_eq!(
            contrast_text(Color::rgb(0xff, 0xff, 0xff)),
            Palette::TEXT_ON_LIGHT
        );
        assert_eq!(
            contrast_text(Color::rgb(0x00, 0x00, 0x00)),
            Palette::TEXT_ON_DARK
        );
    }

    #[test]
    fn lerp_midpoint_keeps_base_alpha() {
        let base = Color::rgba(0, 0, 0, 0x40);
        let mixed = base.lerp(Color::rgb(0xff, 0xff, 0xff), 0.5);
        assert_eq!(mixed.r, 128);
        assert_eq!(mixed.a, 0x40);
    }

    #[test]
    fn from_str_parses_rgb_and_rgba() {
        assert_eq!(
            "#112233".parse::<Color>().unwrap(),
            Color::rgb(0x11, 0x22, 0x33)
        );
        assert_eq!(
            "#11223344".parse::<Color>().unwrap(),
            Color::rgba(0x11, 0x22, 0x33, 0x44)
        );
    }

    #[test]
    fn from_str_rejects_multibyte_without_panicking() {
        assert!("#\u{20ac}\u{20ac}".parse::<Color>().is_err());
        assert!("\u{20ac}\u{20ac}".parse::<Color>().is_err());
        assert!("\u{20ac}\u{20ac}xx".parse::<Color>().is_err());
    }

    #[test]
    fn from_str_rejects_wrong_length() {
        assert!("#fff".parse::<Color>().is_err());
        assert!("#xyzxyz".parse::<Color>().is_err());
    }

    #[test]
    fn to_hex_roundtrips() {
        assert_eq!(Color::rgb(0x1a, 0x1b, 0x26).to_hex(), "#1a1b26");
        assert_eq!(Color::rgba(0x05, 0x08, 0x0c, 110).to_hex(), "#05080c6e");
    }

    #[test]
    fn metro_palette_anchors_match_spec() {
        let p = Palette::TOKYO_NIGHT_METRO;
        assert_eq!(p.accent, Color::rgb(0x7a, 0xa2, 0xf7));
        assert_eq!(p.background, Color::rgb(0x1a, 0x1b, 0x26));
        assert_eq!(p.text, Color::rgb(0xc0, 0xca, 0xf5));
        assert_eq!(p.error, Color::rgb(0xf7, 0x76, 0x8e));
    }

    #[test]
    fn default_palette_is_dark() {
        assert_eq!(Palette::default(), Palette::DARK);
    }

    #[test]
    fn dark_palette_anchors_match_mockup() {
        let p = Palette::DARK;
        assert_eq!(p.background, Color::rgb(0x14, 0x17, 0x1b));
        assert_eq!(p.surface, Color::rgb(0x20, 0x25, 0x2b));
        assert_eq!(p.accent, Color::rgb(0x4e, 0x99, 0xf3));
        assert_eq!(p.text, Color::rgb(0xdc, 0xde, 0xe1));
    }

    #[test]
    fn light_palette_anchors_match_mockup() {
        let p = Palette::LIGHT;
        assert_eq!(p.background, Color::rgb(0xec, 0xe4, 0xd3));
        assert_eq!(p.surface, Color::rgb(0xf4, 0xef, 0xe3));
        assert_eq!(p.accent, Color::rgb(0x2f, 0x62, 0x99));
        assert_eq!(p.text, Color::rgb(0x07, 0x11, 0x1c));
    }

    #[test]
    fn dark_and_light_differ_only_in_being_distinct_tables() {
        // Sanity: the two shipped palettes are genuinely different colour
        // tables (the only thing a theme switch swaps).
        assert_ne!(Palette::DARK, Palette::LIGHT);
        assert_ne!(Palette::DARK.background, Palette::LIGHT.background);
    }
}
