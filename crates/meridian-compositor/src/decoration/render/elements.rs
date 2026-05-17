use smallvec::SmallVec;

use meridian_config::{Decorations, ThemeColors};
use smithay::{
    backend::renderer::{
        element::{memory::MemoryRenderBufferRenderElement, solid::SolidColorRenderElement, Kind},
        gles::GlesRenderer,
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Physical, Point, Scale, Size},
};

use super::{
    super::{
        icons::{IconTint, WindowIcon},
        model::HoveredButton,
        DecorationManager, DecorationRenderElement, BUTTON_HEIGHT, BUTTON_ICON_PX, BUTTON_WIDTH,
        TITLE_BAR_HEIGHT,
    },
    buffers::update_buffers,
    geometry::{SsdChromeMetrics, SsdFrameMetrics},
};

impl DecorationManager {
    // Keep explicit render inputs to make ordering-sensitive decoration composition obvious.
    #[allow(clippy::too_many_arguments)]
    pub fn render_elements(
        &mut self,
        renderer: &mut GlesRenderer,
        surface: &WlSurface,
        window_loc: Point<i32, Logical>,
        content_size: Size<i32, Logical>,
        theme: &Decorations,
        colors: &ThemeColors,
        scale: Scale<f64>,
    ) -> SmallVec<[DecorationRenderElement; 16]> {
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
        let mut elements: SmallVec<[DecorationRenderElement; 16]> = SmallVec::new();

        let phys = |lx: i32, ly: i32| -> Point<i32, Physical> {
            Point::from(((lx as f64 * ps) as i32, (ly as f64 * ps) as i32))
        };
        let phys_f64 = |lx: i32, ly: i32| phys(lx, ly).to_f64();

        if theme.shadow && bw > 0 {
            let sr = theme.shadow_radius as i32;
            elements.push(
                SolidColorRenderElement::from_buffer(
                    &deco.buffers.shadow,
                    phys(x - sr, y - sr),
                    scale,
                    1.0,
                    Kind::Unspecified,
                )
                .into(),
            );
        }

        if show_title {
            let chrome = SsdChromeMetrics::new(SsdFrameMetrics::from_frame_origin(
                window_loc,
                content_size,
                bw,
                title_h,
            ));
            let buttons = chrome
                .button_metrics()
                .expect("titlebar buttons should exist when titlebar is shown");

            let close_tint = if deco.hovered_button() == Some(HoveredButton::Close) {
                IconTint::OnAccentRed
            } else {
                IconTint::OnSurface
            };
            let max_kind = if deco.is_maximized {
                WindowIcon::Restore
            } else {
                WindowIcon::Maximize
            };

            let icon_pos = |rect: smithay::utils::Rectangle<i32, Logical>| {
                let icon_x = rect.loc.x + (BUTTON_WIDTH - BUTTON_ICON_PX as i32) / 2;
                let icon_y = rect.loc.y + (BUTTON_HEIGHT - BUTTON_ICON_PX as i32) / 2;
                (icon_x, icon_y)
            };

            let (close_icon_x, close_icon_y) = icon_pos(buttons.close_rect);
            if let Ok(icon) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                phys_f64(close_icon_x, close_icon_y),
                self.icon_cache
                    .get_or_build(WindowIcon::Close, close_tint, colors),
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(DecorationRenderElement::Icon(icon));
            }
            elements.push(DecorationRenderElement::Solid(
                SolidColorRenderElement::from_buffer(
                    &deco.buffers.close_bg,
                    phys(buttons.close_rect.loc.x, buttons.close_rect.loc.y),
                    scale,
                    1.0,
                    Kind::Unspecified,
                ),
            ));

            let (maximize_icon_x, maximize_icon_y) = icon_pos(buttons.maximize_rect);
            if let Ok(icon) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                phys_f64(maximize_icon_x, maximize_icon_y),
                self.icon_cache
                    .get_or_build(max_kind, IconTint::OnSurface, colors),
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(DecorationRenderElement::Icon(icon));
            }
            elements.push(DecorationRenderElement::Solid(
                SolidColorRenderElement::from_buffer(
                    &deco.buffers.maximize_bg,
                    phys(buttons.maximize_rect.loc.x, buttons.maximize_rect.loc.y),
                    scale,
                    1.0,
                    Kind::Unspecified,
                ),
            ));

            let (minimize_icon_x, minimize_icon_y) = icon_pos(buttons.minimize_rect);
            if let Ok(icon) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                phys_f64(minimize_icon_x, minimize_icon_y),
                self.icon_cache
                    .get_or_build(WindowIcon::Minimize, IconTint::OnSurface, colors),
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(DecorationRenderElement::Icon(icon));
            }
            elements.push(DecorationRenderElement::Solid(
                SolidColorRenderElement::from_buffer(
                    &deco.buffers.minimize_bg,
                    phys(buttons.minimize_rect.loc.x, buttons.minimize_rect.loc.y),
                    scale,
                    1.0,
                    Kind::Unspecified,
                ),
            ));
            elements.push(DecorationRenderElement::Solid(
                SolidColorRenderElement::from_buffer(
                    &deco.buffers.titlebar,
                    phys(x, y),
                    scale,
                    1.0,
                    Kind::Unspecified,
                ),
            ));
        }

        if bw > 0 {
            if !show_title {
                elements.push(DecorationRenderElement::Solid(
                    SolidColorRenderElement::from_buffer(
                        &deco.buffers.border_top,
                        phys(x, y),
                        scale,
                        1.0,
                        Kind::Unspecified,
                    ),
                ));
            }
            elements.push(DecorationRenderElement::Solid(
                SolidColorRenderElement::from_buffer(
                    &deco.buffers.border_left,
                    phys(x, y + title_h),
                    scale,
                    1.0,
                    Kind::Unspecified,
                ),
            ));
            elements.push(DecorationRenderElement::Solid(
                SolidColorRenderElement::from_buffer(
                    &deco.buffers.border_right,
                    phys(x + bw + cw, y + title_h),
                    scale,
                    1.0,
                    Kind::Unspecified,
                ),
            ));
            elements.push(DecorationRenderElement::Solid(
                SolidColorRenderElement::from_buffer(
                    &deco.buffers.border_bottom,
                    phys(x, y + title_h + bw + ch),
                    scale,
                    1.0,
                    Kind::Unspecified,
                ),
            ));
        }

        elements
    }
}

impl From<SolidColorRenderElement> for DecorationRenderElement {
    fn from(value: SolidColorRenderElement) -> Self {
        Self::Solid(value)
    }
}
