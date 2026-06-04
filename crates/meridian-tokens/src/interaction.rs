//! Interaction-state tokens: the canonical hover / pressed / accent-cushion
//! treatments. Before this, each widget hand-rolled its own lighten fraction
//! and overlay alpha (white@30, black@56, lerp 0.10/0.14/0.16/0.18 …); they
//! now route through one `Interaction` so a state treatment is tuned in one
//! place. See the design-tokens audit (2026-06-04).

use crate::Color;

const WHITE: Color = Color::rgb(0xff, 0xff, 0xff);
const BLACK: Color = Color::rgb(0x00, 0x00, 0x00);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interaction {
    /// Hover: lerp the base colour this far towards white.
    pub hover_lighten: f32,
    /// Pressed: lerp the base colour this far towards black.
    pub pressed_darken: f32,
    /// Translucent hover overlay for neutral controls drawn over glass
    /// (no opaque base to lighten — the overlay floats on the blur).
    pub neutral_hover: Color,
    /// Translucent pressed overlay for the same neutral controls.
    pub neutral_pressed: Color,
    /// Accent cushion alpha for an active / focused control at rest.
    pub accent_idle_alpha: u8,
    /// Accent cushion alpha for an active / focused control on hover.
    pub accent_hover_alpha: u8,
}

impl Interaction {
    pub const DEFAULT: Interaction = Interaction {
        hover_lighten: 0.10,
        pressed_darken: 0.12,
        neutral_hover: Color::rgba(0xff, 0xff, 0xff, 30),
        neutral_pressed: Color::rgba(0x00, 0x00, 0x00, 56),
        accent_idle_alpha: 54,
        accent_hover_alpha: 80,
    };

    /// Base colour lightened for the hover state.
    pub fn hover(&self, base: Color) -> Color {
        base.lerp(WHITE, self.hover_lighten)
    }

    /// Base colour darkened for the pressed state.
    pub fn pressed(&self, base: Color) -> Color {
        base.lerp(BLACK, self.pressed_darken)
    }

    /// Translucent accent cushion at rest (keeps the glass visible behind).
    pub fn accent_idle(&self, accent: Color) -> Color {
        Color::rgba(accent.r, accent.g, accent.b, self.accent_idle_alpha)
    }

    /// Translucent accent cushion on hover.
    pub fn accent_hover(&self, accent: Color) -> Color {
        Color::rgba(accent.r, accent.g, accent.b, self.accent_hover_alpha)
    }
}

impl Default for Interaction {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_lightens_pressed_darkens() {
        let base = Color::rgb(0x40, 0x40, 0x40);
        assert!(Interaction::DEFAULT.hover(base).r > base.r);
        assert!(Interaction::DEFAULT.pressed(base).r < base.r);
    }

    #[test]
    fn accent_cushion_keeps_rgb_sets_alpha() {
        let accent = Color::rgb(0x7a, 0xa2, 0xf7);
        let idle = Interaction::DEFAULT.accent_idle(accent);
        assert_eq!((idle.r, idle.g, idle.b), (0x7a, 0xa2, 0xf7));
        assert_eq!(idle.a, 54);
        assert_eq!(Interaction::DEFAULT.accent_hover(accent).a, 80);
    }

    #[test]
    fn canonical_values_locked() {
        // Guards the values signed off in Phase 3. Changing them is fine —
        // but it WILL move every hover/pressed/cushion in the shell.
        let i = Interaction::DEFAULT;
        assert_eq!(i.hover_lighten, 0.10);
        assert_eq!(i.pressed_darken, 0.12);
        assert_eq!(i.neutral_hover, Color::rgba(0xff, 0xff, 0xff, 30));
        assert_eq!(i.neutral_pressed, Color::rgba(0x00, 0x00, 0x00, 56));
        assert_eq!(i.accent_idle_alpha, 54);
        assert_eq!(i.accent_hover_alpha, 80);
    }
}
