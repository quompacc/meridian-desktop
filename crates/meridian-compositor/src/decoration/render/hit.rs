use meridian_config::Decorations;
use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Size},
};

use super::super::{
    DecorationHit, DecorationManager, BUTTON_MARGIN, BUTTON_SIZE, TITLE_BAR_HEIGHT,
};

impl DecorationManager {
    pub fn hit_test(
        &self,
        surface: &WlSurface,
        pointer_pos: Point<f64, Logical>,
        window_loc: Point<i32, Logical>,
        content_size: Size<i32, Logical>,
        theme: &Decorations,
    ) -> Option<DecorationHit> {
        let deco = self.decorations.get(&Self::key(surface))?;
        if !deco.should_draw() || !deco.should_draw_title_bar() {
            return None;
        }

        let bw = deco.border_width(theme);
        let px = pointer_pos.x as i32;
        let py = pointer_pos.y as i32;
        let wx = window_loc.x;
        let wy = window_loc.y;
        let total_w = content_size.w + bw * 2;

        if px < wx || py < wy || px >= wx + total_w || py >= wy + TITLE_BAR_HEIGHT + bw {
            return None;
        }

        let close_x = wx + total_w - BUTTON_SIZE - BUTTON_MARGIN;
        let close_y = wy + (TITLE_BAR_HEIGHT - BUTTON_SIZE) / 2 + bw;
        let max_x = close_x - BUTTON_SIZE - BUTTON_MARGIN / 2;
        let min_x = max_x - BUTTON_SIZE - BUTTON_MARGIN / 2;
        let btn_bot = close_y + BUTTON_SIZE;

        if px >= close_x && px < close_x + BUTTON_SIZE && py >= close_y && py < btn_bot {
            return Some(DecorationHit::CloseButton);
        }
        if px >= max_x && px < max_x + BUTTON_SIZE && py >= close_y && py < btn_bot {
            return Some(DecorationHit::MaximizeButton);
        }
        if px >= min_x && px < min_x + BUTTON_SIZE && py >= close_y && py < btn_bot {
            return Some(DecorationHit::MinimizeButton);
        }

        if bw > 0 && (px < wx + bw || px >= wx + total_w - bw) {
            return Some(DecorationHit::Border);
        }

        Some(DecorationHit::TitleBar)
    }
}
