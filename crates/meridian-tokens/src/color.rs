//! Core colour primitive and the canonical Meridian palette.
//!
//! This is the single source of truth: `meridian-config` (theme deserialization)
//! and `meridian-ui` (render tokens) both re-export `Color`/`Palette` from here.
//! There is no second copy to hand-sync — see the design-tokens audit (2026-06-04).

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
    /// The canonical Tokyo-Night-Metro palette. THE source of truth for the
    /// built-in defaults; `meridian_config::ThemeColors::default()` derives
    /// from this rather than re-listing the values.
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
}

impl Default for Palette {
    fn default() -> Self {
        Self::TOKYO_NIGHT_METRO
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
}
