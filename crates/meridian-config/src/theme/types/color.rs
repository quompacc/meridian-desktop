use std::{fmt, str::FromStr};

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        Self::rgba(r, g, b, 255)
    }

    pub fn as_f32_array(self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }

    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
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

#[cfg(test)]
mod tests {
    use super::Color;

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
        // Two euro signs are 6 bytes (3 each) but not a char-boundary-safe hex
        // string; byte-slicing must not split a char. Regression for a config
        // color value that would otherwise panic theme loading.
        assert!("#\u{20ac}\u{20ac}".parse::<Color>().is_err());
        assert!("\u{20ac}\u{20ac}".parse::<Color>().is_err());
        // An 8-byte multibyte string must also fall through gracefully.
        assert!("\u{20ac}\u{20ac}xx".parse::<Color>().is_err());
    }

    #[test]
    fn from_str_rejects_wrong_length() {
        assert!("#fff".parse::<Color>().is_err());
        assert!("#xyzxyz".parse::<Color>().is_err());
    }
}
