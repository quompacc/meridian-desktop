//! Metro tile widget.
//!
//! The `label` is stored for future text rendering but is not painted yet.
//! This implementation rebuilds rounded-rect paths in `paint`; that allocates
//! in the render path and is an accepted step-5 trade-off until path caching
//! is introduced.

use taffy::prelude::{length, Size, Style};
use tiny_skia::PixmapMut;

use crate::{
    effect::{paint_fill, rounded_rect_path},
    paint::Rect,
    style::{Color, Theme},
};

use super::Widget;

pub const TILE_SIZE: i32 = 96;
pub const STRIPE_HEIGHT: i32 = 4;

pub struct Tile {
    label: &'static str,
    accent: Color,
}

impl Tile {
    pub fn new(label: &'static str, accent: Color) -> Self {
        Self { label, accent }
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn accent(&self) -> Color {
        self.accent
    }
}

impl Widget for Tile {
    fn style(&self) -> Style {
        Style {
            size: Size {
                width: length(TILE_SIZE as f32),
                height: length(TILE_SIZE as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme) {
        if let Some(body_path) = rounded_rect_path(area, theme.radius.lg) {
            paint_fill(canvas, &body_path, theme.palette.surface);
        }

        let stripe_height = STRIPE_HEIGHT.max(0).min(area.height);
        if stripe_height <= 0 {
            return;
        }

        let stripe_rect = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: stripe_height,
        };
        if let Some(stripe_path) = rounded_rect_path(stripe_rect, 0) {
            paint_fill(canvas, &stripe_path, self.accent);
        }
    }
}

#[cfg(test)]
mod tests {
    use tiny_skia::Pixmap;

    use super::{Tile, STRIPE_HEIGHT, TILE_SIZE};
    use crate::{paint::Rect, style::Palette, widget::Widget, Theme};

    #[test]
    fn tile_new_stores_label_and_accent() {
        let tile = Tile::new("hello", Palette::TOKYO_NIGHT_METRO.accent_alt);
        assert_eq!(tile.label(), "hello");
        assert_eq!(tile.accent(), Palette::TOKYO_NIGHT_METRO.accent_alt);
    }

    #[test]
    fn tile_style_is_fixed_square() {
        let tile = Tile::new("hello", Palette::TOKYO_NIGHT_METRO.accent);
        let style = tile.style();
        assert_eq!(style.size.width, taffy::prelude::length(TILE_SIZE as f32));
        assert_eq!(style.size.height, taffy::prelude::length(TILE_SIZE as f32));
    }

    #[test]
    fn tile_paint_draws_stripe_and_body() {
        let tile = Tile::new("hello", Palette::TOKYO_NIGHT_METRO.accent_alt);
        let mut pixmap = Pixmap::new(TILE_SIZE as u32, TILE_SIZE as u32).expect("pixmap");
        let mut canvas = pixmap.as_mut();
        tile.paint(
            Rect {
                x: 0,
                y: 0,
                width: TILE_SIZE,
                height: TILE_SIZE,
            },
            &mut canvas,
            &Theme::TOKYO_NIGHT_METRO,
        );
        drop(canvas);

        let stripe_px = pixmap.pixel(50, 1).expect("stripe pixel");
        let body_px = pixmap.pixel(50, 50).expect("body pixel");

        assert!(stripe_px.alpha() > 0);
        assert!(body_px.alpha() > 0);
        assert!(stripe_px.red() > body_px.red());
        assert!(stripe_px.blue() > body_px.blue());
        assert!(stripe_px.green() < stripe_px.blue());
        assert_eq!(STRIPE_HEIGHT, 4);
    }
}
