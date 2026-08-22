use std::collections::HashMap;

use smithay::backend::renderer::{
    element::{memory::MemoryRenderBufferRenderElement, solid::SolidColorRenderElement},
    gles::{element::PixelShaderElement, GlesPixelProgram, GlesRenderer},
};
use smithay::reexports::wayland_server::{
    backend::ObjectId, protocol::wl_surface::WlSurface, Resource,
};

pub mod icons;
mod model;
mod render;

use crate::backend::drm::glass::GlassTitlebarInfo;

pub use model::HoveredButton;
use model::WindowDecoration;
use render::icon_cache::IconCache;

const WINDOW_CHROME: meridian_tokens::WindowChrome = meridian_tokens::WindowChrome::DEFAULT;
pub const TITLE_BAR_HEIGHT: i32 = WINDOW_CHROME.titlebar_height;
pub const BUTTON_WIDTH: i32 = WINDOW_CHROME.button_width;
pub const BUTTON_ICON_PX: u32 = WINDOW_CHROME.button_icon_size;
pub const BUTTON_STROKE_WIDTH: f32 = WINDOW_CHROME.button_icon_stroke;
pub const BUTTON_HOVER_INSET: i32 = WINDOW_CHROME.button_hover_inset;
pub const BUTTON_HOVER_RADIUS: f32 = WINDOW_CHROME.button_hover_radius;
pub const TITLE_SEPARATOR_HEIGHT: i32 = WINDOW_CHROME.separator_height;

// CLAUDE.md-Regel 4 verbietet Heap-Alloc im Render-Loop. `render_elements()`
// baut pro Frame eine SmallVec<DecorationRenderElement; 32> auf - Box um die
// Icon-Variante würde 3-6 Heap-Allokationen pro Frame verursachen. Wir
// nehmen den Größenunterschied stattdessen bewusst in Kauf (Stack/SmallVec
// dimensioniert großzügig).
#[allow(clippy::large_enum_variant)]
pub enum DecorationRenderElement {
    Solid(SolidColorRenderElement),
    Icon(MemoryRenderBufferRenderElement<GlesRenderer>),
    /// Shader-driven rounded-quad chrome (titlebar fill, border, frame,
    /// button veils). Drawn in front of the window content.
    PixelShader(PixelShaderElement),
    /// Shader-driven soft drop shadow. Drawn *behind* the window content
    /// so an opaque client is never dimmed by it.
    DropShadow(PixelShaderElement),
    /// Liquid-glass titlebar placeholder; the backend turns this into a
    /// textured blur element once the frame's blur texture exists.
    Glass(GlassTitlebarInfo),
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
    /// Lazily-compiled rounded-box soft-shadow pixel shader.
    shadow_shader: Option<GlesPixelProgram>,
    /// Lazily-compiled rounded-rectangle fill/outline pixel shader, used for
    /// the rounded titlebar (fill) and the rounded window border (outline).
    rounded_quad_shader: Option<GlesPixelProgram>,
    /// Lazily-compiled liquid-glass titlebar shader: translucent tinted fill
    /// with a vertical sheen and a specular top edge.
    glass_shader: Option<GlesPixelProgram>,
}

impl DecorationManager {
    pub fn new() -> Self {
        Self {
            decorations: HashMap::new(),
            icon_cache: IconCache::new(BUTTON_ICON_PX, BUTTON_STROKE_WIDTH),
            shadow_shader: None,
            rounded_quad_shader: None,
            glass_shader: None,
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

    pub fn update_hover_button(
        &mut self,
        surface: &WlSurface,
        hovered: Option<HoveredButton>,
    ) -> bool {
        let d = self
            .decorations
            .entry(Self::key(surface))
            .or_insert_with(WindowDecoration::new);
        Self::set_hover_and_mark_dirty(d, hovered)
    }

    /// Clear every decoration's hover EXCEPT `keep`'s, in one pass. Returns true
    /// if any *other* decoration actually changed.
    ///
    /// Pairs with `update_hover_button(keep, ...)`: by leaving the hovered
    /// surface untouched here, resting the pointer on a titlebar button is no
    /// longer reported as a change on every motion tick. The previous
    /// clear-ALL-then-set sequence flipped the hovered deco None<->Some each
    /// frame, so `hover_changed` was always true — forcing a full-output repaint
    /// and an `info!` log on every pointer motion over a button (LOG-1).
    pub fn clear_hover_buttons_except(&mut self, keep: Option<&WlSurface>) -> bool {
        let keep_key = keep.map(Self::key);
        let mut any_changed = false;
        for (id, deco) in self.decorations.iter_mut() {
            if keep_key.as_ref() == Some(id) {
                continue;
            }
            if Self::set_hover_and_mark_dirty(deco, None) {
                any_changed = true;
            }
        }
        any_changed
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
