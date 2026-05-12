use std::collections::HashMap;

use smithay::reexports::wayland_server::{
    backend::ObjectId, protocol::wl_surface::WlSurface, Resource,
};

mod model;
mod render;

use model::WindowDecoration;

pub const TITLE_BAR_HEIGHT: i32 = 32;
pub const BUTTON_SIZE: i32 = 16;
pub const BUTTON_MARGIN: i32 = 8;
const SHADOW_ALPHA: f32 = 0.35;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationResizeEdge {
    Top,
    Left,
    Right,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationHit {
    TitleBar,
    CloseButton,
    MaximizeButton,
    MinimizeButton,
    Resize(DecorationResizeEdge),
}

pub struct DecorationManager {
    decorations: HashMap<ObjectId, WindowDecoration>,
}

impl DecorationManager {
    pub fn new() -> Self {
        Self {
            decorations: HashMap::new(),
        }
    }

    pub(super) fn key(surface: &WlSurface) -> ObjectId {
        surface.id()
    }

    pub fn set_ssd(&mut self, surface: &WlSurface, ssd: bool) {
        let d = self
            .decorations
            .entry(Self::key(surface))
            .or_insert_with(WindowDecoration::new);
        if d.has_ssd != ssd {
            d.has_ssd = ssd;
            d.dirty = true;
        }
    }

    pub fn set_focused(&mut self, surface: &WlSurface, focused: bool) {
        if let Some(d) = self.decorations.get_mut(&Self::key(surface)) {
            if d.is_focused != focused {
                d.is_focused = focused;
                d.dirty = true;
            }
        }
    }

    pub fn set_maximized(&mut self, surface: &WlSurface, maximized: bool) {
        let d = self
            .decorations
            .entry(Self::key(surface))
            .or_insert_with(WindowDecoration::new);
        if d.is_maximized != maximized {
            d.is_maximized = maximized;
            d.dirty = true;
        }
    }

    pub fn set_tiled(&mut self, surface: &WlSurface, tiled: bool) {
        let d = self
            .decorations
            .entry(Self::key(surface))
            .or_insert_with(WindowDecoration::new);
        if d.is_tiled != tiled {
            d.is_tiled = tiled;
            d.dirty = true;
        }
    }

    pub fn set_fullscreen(&mut self, surface: &WlSurface, fullscreen: bool) {
        let d = self
            .decorations
            .entry(Self::key(surface))
            .or_insert_with(WindowDecoration::new);
        if d.is_fullscreen != fullscreen {
            d.is_fullscreen = fullscreen;
            d.dirty = true;
        }
    }

    pub fn remove(&mut self, surface: &WlSurface) {
        self.decorations.remove(&Self::key(surface));
    }

    pub fn has_ssd(&self, surface: &WlSurface) -> bool {
        self.decorations
            .get(&Self::key(surface))
            .map(|d| d.should_draw())
            .unwrap_or(false)
    }
}

impl Default for DecorationManager {
    fn default() -> Self {
        Self::new()
    }
}
