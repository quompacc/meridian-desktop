//! Metro-styled button widget used by preview footer controls.

use taffy::prelude::{length, Size, Style};
use tiny_skia::PixmapMut;

use crate::{
    effect::paint_metro_surface,
    paint::Rect,
    style::{Color, Theme},
};

use super::{tile::STRIPE_HEIGHT, Widget};

pub const BUTTON_DEFAULT_WIDTH: i32 = 48;
pub const BUTTON_DEFAULT_HEIGHT: i32 = 48;

pub struct Button {
    label: &'static str,
    accent: Color,
    width: i32,
    height: i32,
}

impl Button {
    pub fn new(label: &'static str, accent: Color, width: i32, height: i32) -> Self {
        Self {
            label,
            accent,
            width,
            height,
        }
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn accent(&self) -> Color {
        self.accent
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }
}

impl Widget for Button {
    fn style(&self) -> Style {
        Style {
            size: Size {
                width: length(self.width.max(0) as f32),
                height: length(self.height.max(0) as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme) {
        paint_metro_surface(canvas, area, self.accent, theme, STRIPE_HEIGHT);
    }
}

#[cfg(test)]
mod tests {
    use tiny_skia::Pixmap;

    use super::{Button, BUTTON_DEFAULT_HEIGHT, BUTTON_DEFAULT_WIDTH};
    use crate::{paint::Rect, style::Palette, widget::Widget, Theme};

    #[test]
    fn button_new_stores_fields() {
        let button = Button::new("power", Palette::TOKYO_NIGHT_METRO.error, 72, 40);
        assert_eq!(button.label(), "power");
        assert_eq!(button.accent(), Palette::TOKYO_NIGHT_METRO.error);
        assert_eq!(button.width(), 72);
        assert_eq!(button.height(), 40);
    }

    #[test]
    fn button_style_uses_explicit_size() {
        let button = Button::new(
            "default",
            Palette::TOKYO_NIGHT_METRO.accent,
            BUTTON_DEFAULT_WIDTH,
            BUTTON_DEFAULT_HEIGHT,
        );
        let style = button.style();
        assert_eq!(
            style.size.width,
            taffy::prelude::length(BUTTON_DEFAULT_WIDTH as f32)
        );
        assert_eq!(
            style.size.height,
            taffy::prelude::length(BUTTON_DEFAULT_HEIGHT as f32)
        );
    }

    #[test]
    fn button_paint_smoke() {
        let button = Button::new("power", Palette::TOKYO_NIGHT_METRO.warning, 72, 40);
        let mut pixmap = Pixmap::new(72, 40).expect("pixmap");
        let mut canvas = pixmap.as_mut();
        button.paint(
            Rect {
                x: 0,
                y: 0,
                width: 72,
                height: 40,
            },
            &mut canvas,
            &Theme::TOKYO_NIGHT_METRO,
        );
        drop(canvas);

        assert!(pixmap.pixel(36, 1).expect("stripe").alpha() > 0);
        assert!(pixmap.pixel(36, 20).expect("body").alpha() > 0);
    }
}
