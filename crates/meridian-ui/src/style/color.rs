//! Color primitives and the embedded Meridian Tokyo-Night-Metro palette.

/// 8-bit-per-channel RGBA color. `Copy` so it passes through the render
/// loop without heap traffic.
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
}

/// Resolved palette. `Copy` - pass by value, never clone in the render loop.
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
    fn metro_palette_anchors_match_spec() {
        let p = Palette::TOKYO_NIGHT_METRO;
        assert_eq!(p.accent, Color::rgb(0x7a, 0xa2, 0xf7));
        assert_eq!(p.background, Color::rgb(0x1a, 0x1b, 0x26));
        assert_eq!(p.text, Color::rgb(0xc0, 0xca, 0xf5));
        assert_eq!(p.error, Color::rgb(0xf7, 0x76, 0x8e));
    }
}
