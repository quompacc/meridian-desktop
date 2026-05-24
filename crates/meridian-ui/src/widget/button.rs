//! Metro-styled button widget used by preview footer controls.

use taffy::prelude::{length, Size, Style};
use tiny_skia::{Pixmap, PixmapMut, PixmapPaint, Transform};

use crate::{
    effect::{measure_text, paint_metro_surface, paint_text},
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
pub const BUTTON_ARMED_LABEL_FONT_PX: f32 = 14.0;
const DEFAULT_ARMED_LABEL: &str = "OK?";

pub struct Button {
    label: &'static str,
    armed_label: Option<&'static str>,
    accent: Color,
    width: i32,
    height: i32,
    id: Option<&'static str>,
    icon: Option<Pixmap>,
    /// Countdown-arm progress: 0.0 = just armed (full ring), 1.0 = about to time out
    /// (empty ring). `None` = idle (no ring drawn).
    armed_progress: Option<f32>,
}

impl Button {
    pub fn new(label: &'static str, accent: Color, width: i32, height: i32) -> Self {
        Self {
            label,
            armed_label: None,
            accent,
            width,
            height,
            id: None,
            icon: None,
            armed_progress: None,
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
            armed_label: None,
            accent,
            width,
            height,
            id: Some(id),
            icon: None,
            armed_progress: None,
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
            armed_label: None,
            accent,
            width,
            height,
            id: Some(id),
            icon,
            armed_progress: None,
        }
    }

    /// Mark the button as armed (1st click of a destructive confirm-twice
    /// action). `progress` runs 0.0 (just armed, full ring) → 1.0 (timeout,
    /// empty ring). `None` resets to idle.
    pub fn with_armed_progress(mut self, progress: Option<f32>) -> Self {
        self.armed_progress = progress.map(|p| p.clamp(0.0, 1.0));
        self
    }

    /// Override the short confirmation label shown while the button is armed.
    pub fn with_armed_label(mut self, label: &'static str) -> Self {
        self.armed_label = Some(label);
        self
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
        let body_color = if self.armed_progress.is_some() {
            // Armed: shift the body toward the accent so the button reads as
            // "hot — second click commits". Mix with black to keep contrast
            // against the icon.
            self.accent.lerp(Color::rgb(0x10, 0x10, 0x10), 0.40)
        } else {
            match state {
                WidgetState::Idle => theme.palette.surface,
                WidgetState::Hovered => theme
                    .palette
                    .surface
                    .lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.15),
                WidgetState::Pressed => theme.palette.surface.lerp(Color::rgb(0, 0, 0), 0.18),
            }
        };
        paint_metro_surface(canvas, area, body_color, self.accent, theme, STRIPE_HEIGHT);

        if self.armed_progress.is_some() {
            let label = self.armed_label.unwrap_or(DEFAULT_ARMED_LABEL);
            let (text_w, text_h) = measure_text(label, BUTTON_ARMED_LABEL_FONT_PX);
            let x = area.x + (area.width - text_w) / 2;
            let baseline = area.y + (area.height + text_h) / 2 - 2;
            paint_text(
                canvas,
                label,
                x,
                baseline,
                BUTTON_ARMED_LABEL_FONT_PX,
                theme.palette.text,
            );
        } else if self.icon.is_none() {
            paint_text(
                canvas,
                self.label,
                area.x + BUTTON_LABEL_PADDING_X,
                area.y + area.height - BUTTON_LABEL_BASELINE_FROM_BOTTOM,
                BUTTON_LABEL_FONT_PX,
                theme.palette.text,
            );
        }

        if self.armed_progress.is_none() {
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

        if let Some(progress) = self.armed_progress {
            paint_progress_ring(canvas, area, self.accent, progress);
        }
    }
}

/// Draw a circular progress arc inside `area`, starting at 12 o'clock and
/// sweeping clockwise. `progress` runs 0.0 (full circle) → 1.0 (no arc), so
/// it visualises a countdown that drains.
fn paint_progress_ring(canvas: &mut PixmapMut<'_>, area: Rect, color: Color, progress: f32) {
    use std::f32::consts::PI;
    use tiny_skia::{LineCap, Paint, PathBuilder, Stroke};

    if progress >= 1.0 {
        return;
    }
    let radius = (area.width.min(area.height) as f32 / 2.0) - 3.0;
    if radius <= 0.0 {
        return;
    }
    let cx = area.x as f32 + area.width as f32 / 2.0;
    let cy = area.y as f32 + area.height as f32 / 2.0;
    let sweep = (1.0 - progress.max(0.0)) * 2.0 * PI;
    let start = -PI / 2.0; // 12 o'clock

    let mut pb = PathBuilder::new();
    pb.move_to(cx + radius * start.cos(), cy + radius * start.sin());
    let segments = 64;
    for i in 1..=segments {
        let t = i as f32 / segments as f32;
        let angle = start + sweep * t;
        pb.line_to(cx + radius * angle.cos(), cy + radius * angle.sin());
    }
    let Some(path) = pb.finish() else { return };

    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    paint.set_color(tiny_skia::Color::from_rgba8(
        color.r, color.g, color.b, 0xFF,
    ));
    let stroke = Stroke {
        width: 3.0,
        line_cap: LineCap::Round,
        ..Stroke::default()
    };
    canvas.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
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

    #[test]
    fn armed_icon_button_draws_confirmation_label() {
        let mut icon = Pixmap::new(16, 16).expect("icon");
        icon.fill(tiny_skia::Color::from_rgba8(255, 255, 255, 255));
        let button = Button::with_id_and_icon(
            "power-off",
            "Aus",
            Palette::TOKYO_NIGHT_METRO.error,
            48,
            48,
            Some(icon),
        )
        .with_armed_progress(Some(0.0))
        .with_armed_label("OK?");
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

        assert!(pixmap.pixel(24, 24).expect("center").alpha() > 0);
        assert!(pixmap.pixel(24, 4).expect("ring").alpha() > 0);
    }
}
