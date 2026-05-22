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
    buffers::{
        effective_shadow_alpha, effective_shadow_radius, effective_shadow_radius_top,
        update_buffers,
    },
    geometry::{SsdChromeMetrics, SsdFrameMetrics},
};

impl DecorationManager {
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
    ) -> SmallVec<[DecorationRenderElement; 32]> {
        let key = Self::key(surface);
        let deco = match self.decorations.get_mut(&key) {
            Some(d) => d,
            None => {
                static MISS_LOGGED: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(0);
                let n = MISS_LOGGED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if n < 5 {
                    tracing::warn!(
                        "decoration render: no entry for surface={:?} (known keys: {:?})",
                        key,
                        self.decorations.keys().collect::<Vec<_>>()
                    );
                }
                return SmallVec::new();
            }
        };

        if !deco.should_draw() {
            static SKIP_LOGGED: std::sync::atomic::AtomicUsize =
                std::sync::atomic::AtomicUsize::new(0);
            let n = SKIP_LOGGED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if n < 5 {
                tracing::warn!(
                    "decoration render: skip surface={:?} has_ssd={} is_fullscreen={}",
                    key,
                    deco.has_ssd,
                    deco.is_fullscreen
                );
            }
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
        let mut elements: SmallVec<[DecorationRenderElement; 32]> = SmallVec::new();

        let phys = |lx: i32, ly: i32| -> Point<i32, Physical> {
            Point::from(((lx as f64 * ps) as i32, (ly as f64 * ps) as i32))
        };
        let phys_f64 = |lx: i32, ly: i32| phys(lx, ly).to_f64();

        let chrome = SsdChromeMetrics::new(SsdFrameMetrics::from_frame_origin(
            window_loc,
            content_size,
            bw,
            title_h,
        ));

        if show_title {
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

            if deco.is_focused {
                elements.push(DecorationRenderElement::Solid(
                    SolidColorRenderElement::from_buffer(
                        &deco.buffers.title_separator,
                        phys(x, y + TITLE_BAR_HEIGHT + bw - 2),
                        scale,
                        1.0,
                        Kind::Unspecified,
                    ),
                ));
            }

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
            elements.push(DecorationRenderElement::Solid(
                SolidColorRenderElement::from_buffer(
                    &deco.buffers.border_top,
                    phys(x, y),
                    scale,
                    1.0,
                    Kind::Unspecified,
                ),
            ));
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

        if theme.shadow && bw > 0 {
            let sr = effective_shadow_radius(theme.shadow_radius as i32, deco.is_focused);
            let srt = effective_shadow_radius_top(theme.shadow_radius_top as i32, deco.is_focused);
            let alpha = effective_shadow_alpha(theme.shadow_alpha, deco.is_focused);
            if let Some(layout) = chrome.shadow_layout(sr, srt, theme.shadow_offset_y) {
                let shadow = self.shadow_cache.get_for(sr as u32, srt as u32, alpha);

                for (rect, buffer) in [
                    (layout.corner_tl, shadow.corner_tl),
                    (layout.corner_tr, shadow.corner_tr),
                    (layout.corner_bl, shadow.corner_bl),
                    (layout.corner_br, shadow.corner_br),
                ] {
                    if let Ok(element) = MemoryRenderBufferRenderElement::from_buffer(
                        renderer,
                        phys_f64(rect.loc.x, rect.loc.y),
                        buffer,
                        None,
                        None,
                        Some(rect.size),
                        Kind::Unspecified,
                    ) {
                        elements.push(DecorationRenderElement::Icon(element));
                    }
                }

                for (rect, buffer) in [
                    (layout.edge_top, shadow.edge_top),
                    (layout.edge_bottom, shadow.edge_bottom),
                    (layout.edge_left, shadow.edge_left),
                    (layout.edge_right, shadow.edge_right),
                ] {
                    if let Ok(element) = MemoryRenderBufferRenderElement::from_buffer(
                        renderer,
                        phys_f64(rect.loc.x, rect.loc.y),
                        buffer,
                        None,
                        None,
                        Some(rect.size),
                        Kind::Unspecified,
                    ) {
                        elements.push(DecorationRenderElement::Icon(element));
                    }
                }
            }
        }

        elements
    }
}

impl From<SolidColorRenderElement> for DecorationRenderElement {
    fn from(value: SolidColorRenderElement) -> Self {
        Self::Solid(value)
    }
}
