use meridian_config::{Color, Decorations};
use smithay::backend::renderer::element::solid::SolidColorBuffer;

use super::{BUTTON_HEIGHT, BUTTON_WIDTH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoveredButton {
    Close,
    Maximize,
    Minimize,
}

pub(super) struct DecorationBuffers {
    pub(super) titlebar: SolidColorBuffer,
    pub(super) close_bg: SolidColorBuffer,
    pub(super) maximize_bg: SolidColorBuffer,
    pub(super) minimize_bg: SolidColorBuffer,
    pub(super) border_top: SolidColorBuffer,
    pub(super) border_left: SolidColorBuffer,
    pub(super) border_right: SolidColorBuffer,
    pub(super) border_bottom: SolidColorBuffer,
    pub(super) shadow: SolidColorBuffer,
}

impl DecorationBuffers {
    pub(super) fn new() -> Self {
        let z = [0.0f32; 4];
        Self {
            titlebar: SolidColorBuffer::new((1, 1), z),
            close_bg: SolidColorBuffer::new((BUTTON_WIDTH, BUTTON_HEIGHT), z),
            maximize_bg: SolidColorBuffer::new((BUTTON_WIDTH, BUTTON_HEIGHT), z),
            minimize_bg: SolidColorBuffer::new((BUTTON_WIDTH, BUTTON_HEIGHT), z),
            border_top: SolidColorBuffer::new((1, 1), z),
            border_left: SolidColorBuffer::new((1, 1), z),
            border_right: SolidColorBuffer::new((1, 1), z),
            border_bottom: SolidColorBuffer::new((1, 1), z),
            shadow: SolidColorBuffer::new((1, 1), z),
        }
    }
}

pub(super) struct WindowDecoration {
    pub(super) has_ssd: bool,
    pub(super) is_focused: bool,
    pub(super) is_maximized: bool,
    pub(super) is_tiled: bool,
    pub(super) is_fullscreen: bool,
    pub(super) hovered_button: Option<HoveredButton>,
    pub(super) dirty: bool,
    pub(super) last_content_size: (i32, i32),
    pub(super) last_bw: i32,
    pub(super) buffers: DecorationBuffers,
}

impl WindowDecoration {
    pub(super) fn new() -> Self {
        Self {
            has_ssd: true,
            is_focused: false,
            is_maximized: false,
            is_tiled: false,
            is_fullscreen: false,
            hovered_button: None,
            dirty: true,
            last_content_size: (0, 0),
            last_bw: 0,
            buffers: DecorationBuffers::new(),
        }
    }

    pub(super) fn should_draw(&self) -> bool {
        self.has_ssd && !self.is_fullscreen
    }

    pub(super) fn should_draw_title_bar(&self) -> bool {
        self.should_draw() && !self.is_tiled
    }

    pub(super) fn border_width(&self, theme: &Decorations) -> i32 {
        if self.is_maximized || self.is_fullscreen {
            0
        } else if self.is_tiled {
            1
        } else {
            theme.border_width as i32
        }
    }

    pub(super) fn hovered_button(&self) -> Option<HoveredButton> {
        self.hovered_button
    }

    pub(super) fn set_hover(&mut self, hovered: Option<HoveredButton>) -> bool {
        if self.hovered_button == hovered {
            return false;
        }
        self.hovered_button = hovered;
        true
    }
}

pub(super) fn opaque(c: Color) -> [f32; 4] {
    [
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        1.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::{HoveredButton, WindowDecoration};

    #[test]
    fn set_hover_reports_transitions_only_when_value_changes() {
        let mut deco = WindowDecoration::new();
        assert!(deco.set_hover(Some(HoveredButton::Close)));
        assert!(!deco.set_hover(Some(HoveredButton::Close)));
        assert!(deco.set_hover(None));
    }

    #[test]
    fn clear_hover_returns_true_iff_some_deco_was_hovered() {
        let mut a = WindowDecoration::new();
        let mut b = WindowDecoration::new();
        assert!(a.set_hover(Some(HoveredButton::Close)));
        let any = [&mut a, &mut b]
            .into_iter()
            .map(|deco| deco.set_hover(None))
            .fold(false, |acc, changed| acc || changed);
        assert!(any);
    }
}
