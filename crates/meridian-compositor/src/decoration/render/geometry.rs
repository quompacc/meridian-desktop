use meridian_config::Decorations;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

use super::super::{DecorationManager, TITLE_BAR_HEIGHT};

impl DecorationManager {
    pub fn decoration_offset(&self, surface: &WlSurface, theme: &Decorations) -> (i32, i32) {
        let Some(deco) = self.decorations.get(&Self::key(surface)) else {
            return (0, 0);
        };
        if !deco.should_draw() {
            return (0, 0);
        }
        let bw = deco.border_width(theme);
        let title_h = if deco.should_draw_title_bar() {
            TITLE_BAR_HEIGHT
        } else {
            0
        };
        (bw, title_h + bw)
    }

    pub fn decoration_inset(
        &self,
        surface: &WlSurface,
        theme: &Decorations,
    ) -> (i32, i32, i32, i32) {
        let Some(deco) = self.decorations.get(&Self::key(surface)) else {
            return (0, 0, 0, 0);
        };
        if !deco.should_draw() {
            return (0, 0, 0, 0);
        }
        let bw = deco.border_width(theme);
        let title_h = if deco.should_draw_title_bar() {
            TITLE_BAR_HEIGHT
        } else {
            0
        };
        (bw, title_h + bw, bw, bw)
    }
}
