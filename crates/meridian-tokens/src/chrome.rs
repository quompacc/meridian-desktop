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

/// Geometry shared by the panel surface and popovers anchored beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panel {
    pub height: u32,
    pub bottom_gap: u32,
    pub side_margin: u32,
    pub top_shadow: u32,
    /// Height of the quiet interactive rail inside the panel.
    pub control_height: u32,
    /// Raster size requested for pinned application icons.
    pub app_icon_size: i32,
    /// Compact raster size for system and status-notifier icons.
    pub status_icon_size: i32,
    /// Resting control width for launcher and pinned applications.
    pub control_width: u32,
    /// Wider status control used by the clock.
    pub clock_width: u32,
    /// Neutral hover overlay over `Palette::surface_alt`.
    pub hover_alpha: u8,
    /// Focus/running indicator overlay over `Palette::accent`.
    pub active_alpha: u8,
    /// Hairline/group separator over `Palette::text`.
    pub divider_alpha: u8,
}

impl Panel {
    pub const DEFAULT: Panel = Panel {
        height: 42,
        bottom_gap: 8,
        side_margin: 12,
        top_shadow: 16,
        control_height: 32,
        app_icon_size: 22,
        status_icon_size: 18,
        control_width: 40,
        clock_width: 88,
        hover_alpha: 30,
        active_alpha: 38,
        divider_alpha: 46,
    };

    pub const fn surface_height(self) -> u32 {
        self.top_shadow + self.height + self.bottom_gap
    }

    /// Screen edge occupied by the visible panel island and its bottom gap.
    /// Maximized windows may extend into the transparent top-shadow canvas,
    /// but must stop at the island itself.
    pub const fn window_reservation(self) -> u32 {
        self.height + self.bottom_gap
    }
}

impl Default for Panel {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Geometry for compositor-owned server-side window decorations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowChrome {
    pub titlebar_height: i32,
    pub button_width: i32,
    pub button_icon_size: u32,
    pub button_icon_stroke: f32,
    pub button_hover_inset: i32,
    pub button_hover_radius: f32,
    pub separator_height: i32,
    pub resize_handle: i32,
}

impl WindowChrome {
    pub const DEFAULT: WindowChrome = WindowChrome {
        titlebar_height: 34,
        button_width: 38,
        button_icon_size: 13,
        button_icon_stroke: 1.25,
        button_hover_inset: 3,
        button_hover_radius: 6.0,
        separator_height: 1,
        resize_handle: 8,
    };
}

impl Default for WindowChrome {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Launcher surface-overlay opacities (each applied over a `Palette` colour).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Launcher {
    /// Canonical launcher surface width in logical pixels.
    pub width: i32,
    /// Canonical launcher surface height in logical pixels.
    pub height: i32,
    /// Gap between the launcher and the panel reservation.
    pub panel_gap: i32,
    /// Raster size requested for application icons in logical pixels.
    pub app_icon_size: i32,
    pub header_height: i32,
    pub footer_height: i32,
    pub sidebar_width: i32,
    pub outer_pad: i32,
    pub content_pad: i32,
    pub search_height: i32,
    pub sidebar_heading_height: i32,
    pub favorite_row_height: i32,
    pub app_heading_height: i32,
    pub app_card_height: i32,
    pub grid_gap: i32,
    pub grid_columns: i32,
    pub footer_button_size: i32,
    pub footer_button_gap: i32,
    /// Outer band behind the grid (0 = fully transparent: the compositor glass shows).
    pub band_alpha: u8,
    /// Resting cell background (0 = transparent).
    pub cell_alpha: u8,
    /// Hovered cell overlay (over `Palette::surface`).
    pub hover_alpha: u8,
    /// Selected cell overlay over the neutral `surface_alt` colour.
    pub selected_alpha: u8,
    /// Search-field fill (over `Palette::surface`).
    pub search_field_alpha: u8,
    /// Quiet accent hairline while the launcher search owns keyboard focus.
    pub search_focus_alpha: u8,
    /// Bento accent strip (over `Palette::accent`).
    pub bento_accent_alpha: u8,
    /// Power button when "armed" (over `Palette::error`).
    pub power_armed_alpha: u8,
    /// Section divider hairline (over `Palette::accent`).
    pub divider_alpha: u8,
}

impl Launcher {
    pub const DEFAULT: Launcher = Launcher {
        width: 880,
        height: 620,
        panel_gap: 2,
        app_icon_size: 32,
        header_height: 76,
        footer_height: 64,
        sidebar_width: 224,
        outer_pad: 16,
        content_pad: 24,
        search_height: 44,
        sidebar_heading_height: 36,
        favorite_row_height: 48,
        app_heading_height: 56,
        app_card_height: 68,
        grid_gap: 8,
        grid_columns: 2,
        footer_button_size: 36,
        footer_button_gap: 4,
        band_alpha: 0,
        cell_alpha: 0,
        hover_alpha: 42,
        selected_alpha: 56,
        search_field_alpha: 24,
        search_focus_alpha: 52,
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

/// Geometry for the compact system-controls popover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuickSettings {
    pub width: i32,
    pub height: i32,
    pub panel_gap: i32,
    pub outer_pad: i32,
    pub header_height: i32,
    pub tile_height: i32,
    pub tile_gap: i32,
    pub section_gap: i32,
    pub audio_height: i32,
    pub status_height: i32,
    pub footer_height: i32,
    pub control_radius: i32,
    pub slider_height: i32,
    pub slider_thumb_size: i32,
    pub icon_size: i32,
}

impl QuickSettings {
    pub const DEFAULT: QuickSettings = QuickSettings {
        width: 384,
        height: 468,
        panel_gap: 2,
        outer_pad: 18,
        header_height: 50,
        tile_height: 74,
        tile_gap: 10,
        section_gap: 12,
        audio_height: 126,
        status_height: 70,
        footer_height: 46,
        control_radius: 10,
        slider_height: 5,
        slider_thumb_size: 14,
        icon_size: 20,
    };
}

impl Default for QuickSettings {
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
        assert_eq!(Launcher::DEFAULT.width, 880);
        assert_eq!(Launcher::DEFAULT.height, 620);
        assert_eq!(Launcher::DEFAULT.panel_gap, 2);
        assert_eq!(Launcher::DEFAULT.app_icon_size, 32);
        assert_eq!(Launcher::DEFAULT.sidebar_width, 224);
        assert_eq!(Launcher::DEFAULT.grid_columns, 2);
        assert_eq!(Panel::DEFAULT.surface_height(), 66);
        assert_eq!(Panel::DEFAULT.window_reservation(), 50);
        assert_eq!(Panel::DEFAULT.control_height, 32);
        assert_eq!(Panel::DEFAULT.app_icon_size, 22);
        assert_eq!(Panel::DEFAULT.status_icon_size, 18);
        assert_eq!(WindowChrome::DEFAULT.titlebar_height, 34);
        assert_eq!(WindowChrome::DEFAULT.button_width, 38);
        assert_eq!(WindowChrome::DEFAULT.button_icon_size, 13);
        assert_eq!(WindowChrome::DEFAULT.separator_height, 1);
        assert_eq!(Launcher::DEFAULT.selected_alpha, 56);
        assert_eq!(Launcher::DEFAULT.search_focus_alpha, 52);
        assert_eq!(Launcher::DEFAULT.divider_alpha, 44);
        assert_eq!(Mask::DEFAULT.dim_alpha, 160);
        assert_eq!(QuickSettings::DEFAULT.width, 384);
        assert_eq!(QuickSettings::DEFAULT.height, 468);
        assert_eq!(QuickSettings::DEFAULT.slider_thumb_size, 14);
    }

    #[test]
    fn defaults_via_default_trait_match_consts() {
        assert_eq!(Scrollbar::default(), Scrollbar::DEFAULT);
        assert_eq!(Launcher::default(), Launcher::DEFAULT);
        assert_eq!(Mask::default(), Mask::DEFAULT);
    }
}
