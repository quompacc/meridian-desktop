//! Metro-styled button widget used by preview footer controls.

use taffy::prelude::{length, Size, Style};
use tiny_skia::{Pixmap, PixmapMut, PixmapPaint, Transform};

use crate::{
    effect::{paint_metro_surface, paint_text},
    event::WidgetState,
    paint::Rect,
    style::{Color, Theme},
};

use super::{tile::STRIPE_HEIGHT, Widget};

pub const BUTTON_DEFAULT_WIDTH: i32 = 48;
pub const BUTTON_DEFAULT_HEIGHT: i32 = 48;
pub const BUTTON_LABEL_PADDING_X: i32 = 6;
pub const BUTTON_LABEL_BASELINE_FROM_BOTTOM: i32 = 8;
pub const BUTTON_LABEL_FONT_PX: f32 = 11.0;

pub struct Button {
    label: &'static str,
    accent: Color,
    width: i32,
    height: i32,
    id: Option<&'static str>,
    icon: Option<Pixmap>,
}

impl Button {
    pub fn new(label: &'static str, accent: Color, width: i32, height: i32) -> Self {
        Self {
            label,
            accent,
            width,
            height,
            id: None,
            icon: None,
        }
    }

    pub fn with_id(
        id: &'static str,
        label: &'static str,
        accent: Color,
        width: i32,
        height: i32,
    ) -> Self {
        Self {
            label,
            accent,
            width,
            height,
            id: Some(id),
            icon: None,
        }
    }

    pub fn with_id_and_icon(
        id: &'static str,
        label: &'static str,
        accent: Color,
        width: i32,
        height: i32,
        icon: Option<Pixmap>,
    ) -> Self {
        Self {
            label,
            accent,
            width,
            height,
            id: Some(id),
            icon,
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
    fn id(&self) -> Option<&'static str> {
        self.id
    }

    fn style(&self) -> Style {
        Style {
            size: Size {
                width: length(self.width.max(0) as f32),
                height: length(self.height.max(0) as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let body_color = match state {
            WidgetState::Idle => theme.palette.surface,
            WidgetState::Hovered => theme
                .palette
                .surface
                .lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.15),
            WidgetState::Pressed => theme.palette.surface.lerp(Color::rgb(0, 0, 0), 0.18),
        };
        paint_metro_surface(canvas, area, body_color, self.accent, theme, STRIPE_HEIGHT);
        paint_text(
            canvas,
            self.label,
            area.x + BUTTON_LABEL_PADDING_X,
            area.y + area.height - BUTTON_LABEL_BASELINE_FROM_BOTTOM,
            BUTTON_LABEL_FONT_PX,
            theme.palette.text,
        );

        if let Some(ref icon) = self.icon {
            let iw = icon.width() as i32;
            let ih = icon.height() as i32;
            let x = area.x + (area.width - iw) / 2;
            let y = area.y + (area.height - ih) / 2;
            canvas.draw_pixmap(
                x,
                y,
                icon.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use tiny_skia::Pixmap;

    use super::{Button, BUTTON_DEFAULT_HEIGHT, BUTTON_DEFAULT_WIDTH};
    use crate::{event::WidgetState, paint::Rect, style::Palette, widget::Widget, Theme};

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
            WidgetState::Idle,
        );
        drop(canvas);

        assert!(pixmap.pixel(36, 1).expect("stripe").alpha() > 0);
        assert!(pixmap.pixel(36, 20).expect("body").alpha() > 0);
    }

    #[test]
    fn button_with_id_and_icon_none_does_not_panic() {
        let button = Button::with_id_and_icon(
            "test-btn",
            "Test",
            Palette::TOKYO_NIGHT_METRO.accent,
            48,
            48,
            None,
        );
        let mut pixmap = Pixmap::new(48, 48).expect("pixmap");
        let mut canvas = pixmap.as_mut();
        button.paint(
            Rect {
                x: 0,
                y: 0,
                width: 48,
                height: 48,
            },
            &mut canvas,
            &Theme::TOKYO_NIGHT_METRO,
            WidgetState::Idle,
        );
        drop(canvas);
    }
}
