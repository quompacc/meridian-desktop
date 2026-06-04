//! Elevation tokens: the soft drop-shadow recipe per shell surface level.
//!
//! Before this, each client-drawn surface passed its own blur/alpha/offset
//! literals to `soft_shadow::draw_soft_shadow` (panel 13/0.16/0, popups
//! 13/0.12/3, launcher 18/0.16/4). They now name an `Elevation` level so a
//! surface's shadow is tuned in one place. See the design-tokens audit
//! (2026-06-04).
//!
//! NOTE: the *window* drop shadow is deliberately NOT here — it is
//! theme-driven (`meridian_config::Decorations.shadow_*`) and rendered by the
//! compositor's SDF shader with focus modulation, so themes can tune it.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Elevation {
    /// Shadow blur radius, in logical pixels.
    pub blur: f32,
    /// Shadow opacity at full strength (0.0..1.0).
    pub alpha: f32,
    /// Vertical drop offset, in logical pixels.
    pub offset_y: i32,
}

impl Elevation {
    /// The floating panel island — subtle, no drop (cast straight out).
    pub const PANEL: Elevation = Elevation {
        blur: 13.0,
        alpha: 0.16,
        offset_y: 0,
    };

    /// Tray popups, the context menu and other cards — light, slight drop.
    pub const POPUP: Elevation = Elevation {
        blur: 13.0,
        alpha: 0.12,
        offset_y: 3,
    };

    /// The launcher — sits highest, the most pronounced shadow.
    pub const LAUNCHER: Elevation = Elevation {
        blur: 18.0,
        alpha: 0.16,
        offset_y: 4,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_match_phase4_values() {
        // Guards the 1:1 mapping from the former scattered literals.
        assert_eq!(
            (Elevation::PANEL.blur, Elevation::PANEL.alpha, Elevation::PANEL.offset_y),
            (13.0, 0.16, 0)
        );
        assert_eq!(
            (Elevation::POPUP.blur, Elevation::POPUP.alpha, Elevation::POPUP.offset_y),
            (13.0, 0.12, 3)
        );
        assert_eq!(
            (Elevation::LAUNCHER.blur, Elevation::LAUNCHER.alpha, Elevation::LAUNCHER.offset_y),
            (18.0, 0.16, 4)
        );
    }

    #[test]
    fn launcher_sits_above_popup() {
        assert!(Elevation::LAUNCHER.blur >= Elevation::POPUP.blur);
        assert!(Elevation::LAUNCHER.offset_y >= Elevation::POPUP.offset_y);
    }
}
