//! Chrome tokens: scrollbar, launcher-overlay and mask opacities — named once
//! so a global look change touches ONE place. Before this, the shell carried a
//! pile of `LAUNCHER_*_ALPHA` / `DIM_ALPHA` magic constants per file.
//!
//! These are plain `u8` opacities applied over the theme's own colours (the
//! colour always comes from `Palette`; only the *opacity* is named here), so
//! they stay theme-agnostic and identical between the light and dark themes.

/// Scrollbar track/thumb opacity (drawn over glass).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scrollbar {
    /// Track (the groove) — uses `Palette::text`.
    pub track_alpha: u8,
    /// Thumb (the draggable bar) — uses `Palette::accent`.
    pub thumb_alpha: u8,
}

impl Scrollbar {
    pub const DEFAULT: Scrollbar = Scrollbar {
        track_alpha: 25,
        thumb_alpha: 180,
    };
}

impl Default for Scrollbar {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Launcher surface-overlay opacities (each applied over a `Palette` colour).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Launcher {
    /// Outer band behind the grid (0 = fully transparent: the compositor glass shows).
    pub band_alpha: u8,
    /// Resting cell background (0 = transparent).
    pub cell_alpha: u8,
    /// Hovered cell overlay (over `Palette::surface`).
    pub hover_alpha: u8,
    /// Selected cell overlay (over `surface` blended toward `accent`).
    pub selected_alpha: u8,
    /// Search-field fill (over `Palette::surface`).
    pub search_field_alpha: u8,
    /// Bento accent strip (over `Palette::accent`).
    pub bento_accent_alpha: u8,
    /// Power button when "armed" (over `Palette::error`).
    pub power_armed_alpha: u8,
    /// Section divider hairline (over `Palette::accent`).
    pub divider_alpha: u8,
}

impl Launcher {
    pub const DEFAULT: Launcher = Launcher {
        band_alpha: 0,
        cell_alpha: 0,
        hover_alpha: 42,
        selected_alpha: 56,
        search_field_alpha: 24,
        bento_accent_alpha: 105,
        power_armed_alpha: 46,
        divider_alpha: 44,
    };
}

impl Default for Launcher {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Dimming mask opacity (e.g. the region picker darkens the screen behind it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mask {
    pub dim_alpha: u8,
}

impl Mask {
    pub const DEFAULT: Mask = Mask { dim_alpha: 160 };
}

impl Default for Mask {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_locked() {
        // Changing these is fine — but it WILL move the look everywhere they
        // are used, so the change is deliberate and visible in one diff.
        assert_eq!(Scrollbar::DEFAULT.track_alpha, 25);
        assert_eq!(Scrollbar::DEFAULT.thumb_alpha, 180);
        assert_eq!(Launcher::DEFAULT.hover_alpha, 42);
        assert_eq!(Launcher::DEFAULT.selected_alpha, 56);
        assert_eq!(Launcher::DEFAULT.divider_alpha, 44);
        assert_eq!(Mask::DEFAULT.dim_alpha, 160);
    }

    #[test]
    fn defaults_via_default_trait_match_consts() {
        assert_eq!(Scrollbar::default(), Scrollbar::DEFAULT);
        assert_eq!(Launcher::default(), Launcher::DEFAULT);
        assert_eq!(Mask::default(), Mask::DEFAULT);
    }
}
