use std::collections::HashMap;

use smithay::backend::renderer::{
    element::{memory::MemoryRenderBufferRenderElement, solid::SolidColorRenderElement},
    gles::GlesRenderer,
};
use smithay::reexports::wayland_server::{
    backend::ObjectId, protocol::wl_surface::WlSurface, Resource,
};

pub mod icons;
mod model;
mod render;

pub use model::HoveredButton;
use model::WindowDecoration;
use render::icon_cache::IconCache;

pub const TITLE_BAR_HEIGHT: i32 = 32;
pub const BUTTON_WIDTH: i32 = 28;
pub const BUTTON_HEIGHT: i32 = 24;
pub const BUTTON_ICON_PX: u32 = 16;
pub const BUTTON_STROKE_WIDTH: f32 = 1.5;
pub const BUTTON_MARGIN: i32 = 8;
const SHADOW_ALPHA: f32 = 0.35;

pub enum DecorationRenderElement {
    Solid(SolidColorRenderElement),
    Icon(MemoryRenderBufferRenderElement<GlesRenderer>),
}

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
    icon_cache: IconCache,
}

impl DecorationManager {
    pub fn new() -> Self {
        Self {
            decorations: HashMap::new(),
            icon_cache: IconCache::new(BUTTON_ICON_PX, BUTTON_STROKE_WIDTH),
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

    fn set_hover_and_mark_dirty(
        deco: &mut WindowDecoration,
        hovered: Option<HoveredButton>,
    ) -> bool {
        if deco.set_hover(hovered) {
            deco.dirty = true;
            return true;
        }
        false
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

    pub fn update_hover_button(&mut self, surface: &WlSurface, hovered: Option<HoveredButton>) {
        let d = self
            .decorations
            .entry(Self::key(surface))
            .or_insert_with(WindowDecoration::new);
        let _ = Self::set_hover_and_mark_dirty(d, hovered);
    }

    pub fn clear_hover_buttons(&mut self) {
        for deco in self.decorations.values_mut() {
            let _ = Self::set_hover_and_mark_dirty(deco, None);
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

#[cfg(test)]
mod tests {
    use super::{model::HoveredButton, DecorationManager, WindowDecoration};

    #[test]
    fn update_hover_button_marks_dirty_only_on_state_transition() {
        let mut deco = WindowDecoration::new();
        deco.dirty = false;
        assert!(DecorationManager::set_hover_and_mark_dirty(
            &mut deco,
            Some(HoveredButton::Close)
        ));
        assert!(deco.dirty);

        deco.dirty = false;
        assert!(!DecorationManager::set_hover_and_mark_dirty(
            &mut deco,
            Some(HoveredButton::Close)
        ));
        assert!(!deco.dirty);
    }
}
