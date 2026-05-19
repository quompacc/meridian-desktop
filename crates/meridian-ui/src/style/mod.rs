//! Style tokens - colors, spacing, radius - plus the `Theme` aggregate
//! that bundles them. All token structs are `Copy`; `Theme` is `Copy` too,
//! so it passes through the render loop by value without heap traffic
//! or `Clone` calls.

pub mod color;
pub mod radius;
pub mod spacing;

pub use color::{Color, Palette};
pub use radius::Radius;
pub use spacing::Spacing;

/// Bundle of all token kinds. `Copy` - pass it by value, do not clone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub palette: Palette,
    pub spacing: Spacing,
    pub radius: Radius,
}

impl Theme {
    /// Tokyo-Night-Metro defaults - fully embedded, no I/O.
    pub const TOKYO_NIGHT_METRO: Theme = Theme {
        palette: Palette::TOKYO_NIGHT_METRO,
        spacing: Spacing::DEFAULT,
        radius: Radius::METRO,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_bundles_metro_tokens() {
        let t = Theme::TOKYO_NIGHT_METRO;
        assert_eq!(t.palette.accent, Color::rgb(0x7a, 0xa2, 0xf7));
        assert_eq!(t.spacing.md, 8);
        assert_eq!(t.radius.lg, 0);
    }

    #[test]
    fn token_types_are_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<Color>();
        assert_copy::<Palette>();
        assert_copy::<Spacing>();
        assert_copy::<Radius>();
        assert_copy::<Theme>();
    }
}
