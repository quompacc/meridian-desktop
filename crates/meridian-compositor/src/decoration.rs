// PERFORMANCE RULES (do not violate):
// 1. No heap allocation in render_elements() — persistent SolidColorBuffer via .update()
// 2. render_elements() respects dirty flag — buffers updated only when dirty or size changed
// 3. SolidColorBuffer::update() increments commit counter only on actual change (damage tracking)
// 4. SmallVec<[SolidColorRenderElement; 8]> return type — no heap alloc for ≤8 elements
// 5. No Clone of theme data — only &references

use std::collections::HashMap;

use smallvec::SmallVec;

use meridian_config::{Color, Decorations, ThemeColors};
use smithay::{
    backend::renderer::element::{
        solid::{SolidColorBuffer, SolidColorRenderElement},
        Kind,
    },
    utils::{Logical, Physical, Point, Scale, Size},
};
use smithay::reexports::wayland_server::{backend::ObjectId, protocol::wl_surface::WlSurface, Resource};

pub const TITLE_BAR_HEIGHT: i32 = 32;
pub const BUTTON_SIZE: i32 = 16;
pub const BUTTON_MARGIN: i32 = 8;

const SHADOW_ALPHA: f32 = 0.35;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationHit {
    TitleBar,
    CloseButton,
    MaximizeButton,
    MinimizeButton,
    Border,
}

struct DecorationBuffers {
    titlebar:      SolidColorBuffer,
    close_btn:     SolidColorBuffer,
    maximize_btn:  SolidColorBuffer,
    minimize_btn:  SolidColorBuffer,
    border_top:    SolidColorBuffer,
    border_left:   SolidColorBuffer,
    border_right:  SolidColorBuffer,
    border_bottom: SolidColorBuffer,
    shadow:        SolidColorBuffer,
}

impl DecorationBuffers {
    fn new() -> Self {
        let z = [0.0f32; 4];
        Self {
            titlebar:      SolidColorBuffer::new((1, 1), z),
            close_btn:     SolidColorBuffer::new((BUTTON_SIZE, BUTTON_SIZE), z),
            maximize_btn:  SolidColorBuffer::new((BUTTON_SIZE, BUTTON_SIZE), z),
            minimize_btn:  SolidColorBuffer::new((BUTTON_SIZE, BUTTON_SIZE), z),
            border_top:    SolidColorBuffer::new((1, 1), z),
            border_left:   SolidColorBuffer::new((1, 1), z),
            border_right:  SolidColorBuffer::new((1, 1), z),
            border_bottom: SolidColorBuffer::new((1, 1), z),
            shadow:        SolidColorBuffer::new((1, 1), z),
        }
    }
}

struct WindowDecoration {
    has_ssd:           bool,
    is_focused:        bool,
    is_maximized:      bool,
    is_tiled:          bool,
    is_fullscreen:     bool,
    dirty:             bool,
    last_content_size: (i32, i32),
    last_bw:           i32,
    buffers:           DecorationBuffers,
}

impl WindowDecoration {
    fn new() -> Self {
        Self {
            has_ssd:           true,
            is_focused:        false,
            is_maximized:      false,
            is_tiled:          false,
            is_fullscreen:     false,
            dirty:             true,
            last_content_size: (0, 0),
            last_bw:           0,
            buffers:           DecorationBuffers::new(),
        }
    }

    fn should_draw(&self) -> bool {
        self.has_ssd && !self.is_fullscreen
    }

    fn should_draw_title_bar(&self) -> bool {
        self.should_draw() && !self.is_maximized && !self.is_tiled
    }

    fn border_width(&self, theme: &Decorations) -> i32 {
        if self.is_maximized || self.is_fullscreen {
            0
        } else if self.is_tiled {
            1
        } else {
            theme.border_width as i32
        }
    }
}

fn opaque(c: Color) -> [f32; 4] {
    [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, 1.0]
}

pub struct DecorationManager {
    decorations: HashMap<ObjectId, WindowDecoration>,
}

impl DecorationManager {
    pub fn new() -> Self {
        Self { decorations: HashMap::new() }
    }

    fn key(surface: &WlSurface) -> ObjectId {
        surface.id()
    }

    pub fn set_ssd(&mut self, surface: &WlSurface, ssd: bool) {
        let d = self.decorations.entry(Self::key(surface)).or_insert_with(WindowDecoration::new);
        if d.has_ssd != ssd { d.has_ssd = ssd; d.dirty = true; }
    }

    pub fn set_focused(&mut self, surface: &WlSurface, focused: bool) {
        let d = self.decorations.entry(Self::key(surface)).or_insert_with(WindowDecoration::new);
        if d.is_focused != focused { d.is_focused = focused; d.dirty = true; }
    }

    pub fn set_maximized(&mut self, surface: &WlSurface, maximized: bool) {
        let d = self.decorations.entry(Self::key(surface)).or_insert_with(WindowDecoration::new);
        if d.is_maximized != maximized { d.is_maximized = maximized; d.dirty = true; }
    }

    pub fn set_tiled(&mut self, surface: &WlSurface, tiled: bool) {
        let d = self.decorations.entry(Self::key(surface)).or_insert_with(WindowDecoration::new);
        if d.is_tiled != tiled { d.is_tiled = tiled; d.dirty = true; }
    }

    pub fn set_fullscreen(&mut self, surface: &WlSurface, fullscreen: bool) {
        let d = self.decorations.entry(Self::key(surface)).or_insert_with(WindowDecoration::new);
        if d.is_fullscreen != fullscreen { d.is_fullscreen = fullscreen; d.dirty = true; }
    }

    pub fn remove(&mut self, surface: &WlSurface) {
        self.decorations.remove(&Self::key(surface));
    }

    pub fn has_ssd(&self, surface: &WlSurface) -> bool {
        self.decorations.get(&Self::key(surface)).map(|d| d.should_draw()).unwrap_or(false)
    }

    pub fn decoration_offset(&self, surface: &WlSurface, theme: &Decorations) -> (i32, i32) {
        let Some(deco) = self.decorations.get(&Self::key(surface)) else { return (0, 0) };
        if !deco.should_draw() { return (0, 0); }
        let bw = deco.border_width(theme);
        let title_h = if deco.should_draw_title_bar() { TITLE_BAR_HEIGHT } else { 0 };
        (bw, title_h + bw)
    }

    pub fn decoration_inset(&self, surface: &WlSurface, theme: &Decorations) -> (i32, i32, i32, i32) {
        let Some(deco) = self.decorations.get(&Self::key(surface)) else { return (0, 0, 0, 0) };
        if !deco.should_draw() { return (0, 0, 0, 0); }
        let bw = deco.border_width(theme);
        let title_h = if deco.should_draw_title_bar() { TITLE_BAR_HEIGHT } else { 0 };
        (bw, title_h + bw, bw, bw)
    }

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
        let max_x   = close_x - BUTTON_SIZE - BUTTON_MARGIN / 2;
        let min_x   = max_x   - BUTTON_SIZE - BUTTON_MARGIN / 2;
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

        let bw         = deco.border_width(theme);
        let show_title = deco.should_draw_title_bar();
        let title_h    = if show_title { TITLE_BAR_HEIGHT } else { 0 };
        let cw         = content_size.w;
        let ch         = content_size.h;
        let total_w    = cw + bw * 2;

        let size_changed = deco.last_content_size != (cw, ch) || deco.last_bw != bw;
        if deco.dirty || size_changed {
            let border_f32 = opaque(if deco.is_focused { colors.accent } else { colors.border });
            let title_f32  = opaque(if deco.is_focused { colors.accent } else { colors.surface });
            let close_f32  = opaque(colors.error);
            let btn_f32: [f32; 4] = [
                colors.text.r as f32 / 255.0,
                colors.text.g as f32 / 255.0,
                colors.text.b as f32 / 255.0,
                0.6,
            ];

            if show_title {
                deco.buffers.titlebar.update((total_w, TITLE_BAR_HEIGHT + bw), title_f32);
                deco.buffers.close_btn.update((BUTTON_SIZE, BUTTON_SIZE), close_f32);
                deco.buffers.maximize_btn.update((BUTTON_SIZE, BUTTON_SIZE), btn_f32);
                deco.buffers.minimize_btn.update((BUTTON_SIZE, BUTTON_SIZE), btn_f32);
            }
            if bw > 0 {
                if !show_title {
                    deco.buffers.border_top.update((total_w.max(1), bw), border_f32);
                }
                deco.buffers.border_left.update((bw, ch.max(1)), border_f32);
                deco.buffers.border_right.update((bw, ch.max(1)), border_f32);
                deco.buffers.border_bottom.update((total_w.max(1), bw), border_f32);
            }
            if theme.shadow && bw > 0 {
                let sr = theme.shadow_radius as i32;
                let sw = (total_w + sr * 2).max(1);
                let sh = (ch + title_h + bw + sr * 2).max(1);
                deco.buffers.shadow.update((sw, sh), [0.0f32, 0.0, 0.0, SHADOW_ALPHA]);
            }

            deco.last_content_size = (cw, ch);
            deco.last_bw = bw;
            deco.dirty = false;
        }

        let x  = window_loc.x;
        let y  = window_loc.y;
        let ps = scale.x;
        let mut elements: SmallVec<[SolidColorRenderElement; 8]> = SmallVec::new();

        let phys = |lx: i32, ly: i32| -> Point<i32, Physical> {
            Point::from(((lx as f64 * ps) as i32, (ly as f64 * ps) as i32))
        };

        // Shadow (rendered behind content — lower z-order)
        if theme.shadow && bw > 0 {
            let sr = theme.shadow_radius as i32;
            elements.push(SolidColorRenderElement::from_buffer(
                &deco.buffers.shadow,
                phys(x - sr, y - sr),
                scale, 1.0, Kind::Unspecified,
            ));
        }

        // Title bar + buttons
        if show_title {
            let btn_y   = bw + (TITLE_BAR_HEIGHT - BUTTON_SIZE) / 2;
            let close_x = total_w - BUTTON_SIZE - BUTTON_MARGIN;
            let max_x   = close_x - BUTTON_SIZE - BUTTON_MARGIN / 2;
            let min_x   = max_x   - BUTTON_SIZE - BUTTON_MARGIN / 2;

            elements.push(SolidColorRenderElement::from_buffer(
                &deco.buffers.titlebar,
                phys(x, y),
                scale, 1.0, Kind::Unspecified,
            ));
            elements.push(SolidColorRenderElement::from_buffer(
                &deco.buffers.close_btn,
                phys(x + close_x, y + btn_y),
                scale, 1.0, Kind::Unspecified,
            ));
            elements.push(SolidColorRenderElement::from_buffer(
                &deco.buffers.maximize_btn,
                phys(x + max_x, y + btn_y),
                scale, 1.0, Kind::Unspecified,
            ));
            elements.push(SolidColorRenderElement::from_buffer(
                &deco.buffers.minimize_btn,
                phys(x + min_x, y + btn_y),
                scale, 1.0, Kind::Unspecified,
            ));
        }

        // Borders
        if bw > 0 {
            if !show_title {
                elements.push(SolidColorRenderElement::from_buffer(
                    &deco.buffers.border_top,
                    phys(x, y),
                    scale, 1.0, Kind::Unspecified,
                ));
            }
            elements.push(SolidColorRenderElement::from_buffer(
                &deco.buffers.border_left,
                phys(x, y + title_h),
                scale, 1.0, Kind::Unspecified,
            ));
            elements.push(SolidColorRenderElement::from_buffer(
                &deco.buffers.border_right,
                phys(x + bw + cw, y + title_h),
                scale, 1.0, Kind::Unspecified,
            ));
            elements.push(SolidColorRenderElement::from_buffer(
                &deco.buffers.border_bottom,
                phys(x, y + title_h + bw + ch),
                scale, 1.0, Kind::Unspecified,
            ));
        }

        elements
    }
}

impl Default for DecorationManager {
    fn default() -> Self {
        Self::new()
    }
}
