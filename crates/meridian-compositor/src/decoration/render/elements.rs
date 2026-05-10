use smallvec::SmallVec;

use meridian_config::{Decorations, ThemeColors};
use smithay::{
    backend::renderer::element::{solid::SolidColorRenderElement, Kind},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Physical, Point, Scale, Size},
};

use super::{
    super::{DecorationManager, BUTTON_MARGIN, BUTTON_SIZE, TITLE_BAR_HEIGHT},
    buffers::update_buffers,
};

impl DecorationManager {
    pub fn render_elements(
        &mut self,
        surface: &WlSurface,
        window_loc: Point<i32, Logical>,
        content_size: Size<i32, Logical>,
        theme: &Decorations,
        colors: &ThemeColors,
        scale: Scale<f64>,
    ) -> SmallVec<[SolidColorRenderElement; 8]> {
        let deco = match self.decorations.get_mut(&Self::key(surface)) {
            Some(d) => d,
            None => return SmallVec::new(),
        };

        if !deco.should_draw() {
            return SmallVec::new();
        }

        let bw = deco.border_width(theme);
        let show_title = deco.should_draw_title_bar();
        let title_h = if show_title { TITLE_BAR_HEIGHT } else { 0 };
        let cw = content_size.w;
        let ch = content_size.h;
        let total_w = cw + bw * 2;

        let size_changed = deco.last_content_size != (cw, ch) || deco.last_bw != bw;
        if deco.dirty || size_changed {
            update_buffers(
                deco, theme, colors, show_title, bw, total_w, ch, title_h, cw,
            );
        }

        let x = window_loc.x;
        let y = window_loc.y;
        let ps = scale.x;
        let mut elements: SmallVec<[SolidColorRenderElement; 8]> = SmallVec::new();

        let phys = |lx: i32, ly: i32| -> Point<i32, Physical> {
            Point::from(((lx as f64 * ps) as i32, (ly as f64 * ps) as i32))
        };

        if theme.shadow && bw > 0 {
            let sr = theme.shadow_radius as i32;
            elements.push(SolidColorRenderElement::from_buffer(
                &deco.buffers.shadow,
                phys(x - sr, y - sr),
                scale,
                1.0,
                Kind::Unspecified,
            ));
        }

        if show_title {
            let btn_y = bw + (TITLE_BAR_HEIGHT - BUTTON_SIZE) / 2;
            let close_x = total_w - BUTTON_SIZE - BUTTON_MARGIN;
            let max_x = close_x - BUTTON_SIZE - BUTTON_MARGIN / 2;
            let min_x = max_x - BUTTON_SIZE - BUTTON_MARGIN / 2;

            elements.push(SolidColorRenderElement::from_buffer(
                &deco.buffers.titlebar,
                phys(x, y),
                scale,
                1.0,
                Kind::Unspecified,
            ));
            elements.push(SolidColorRenderElement::from_buffer(
                &deco.buffers.close_btn,
                phys(x + close_x, y + btn_y),
                scale,
                1.0,
                Kind::Unspecified,
            ));
            elements.push(SolidColorRenderElement::from_buffer(
                &deco.buffers.maximize_btn,
                phys(x + max_x, y + btn_y),
                scale,
                1.0,
                Kind::Unspecified,
            ));
            elements.push(SolidColorRenderElement::from_buffer(
                &deco.buffers.minimize_btn,
                phys(x + min_x, y + btn_y),
                scale,
                1.0,
                Kind::Unspecified,
            ));
        }

        if bw > 0 {
            if !show_title {
                elements.push(SolidColorRenderElement::from_buffer(
                    &deco.buffers.border_top,
                    phys(x, y),
                    scale,
                    1.0,
                    Kind::Unspecified,
                ));
            }
            elements.push(SolidColorRenderElement::from_buffer(
                &deco.buffers.border_left,
                phys(x, y + title_h),
                scale,
                1.0,
                Kind::Unspecified,
            ));
            elements.push(SolidColorRenderElement::from_buffer(
                &deco.buffers.border_right,
                phys(x + bw + cw, y + title_h),
                scale,
                1.0,
                Kind::Unspecified,
            ));
            elements.push(SolidColorRenderElement::from_buffer(
                &deco.buffers.border_bottom,
                phys(x, y + title_h + bw + ch),
                scale,
                1.0,
                Kind::Unspecified,
            ));
        }

        elements
    }
}
