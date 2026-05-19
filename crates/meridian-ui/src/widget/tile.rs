//! Metro tile widget.
//!
//! The `label` is stored for future text rendering but is not painted yet.
//! This implementation rebuilds rounded-rect paths in `paint`; that allocates
//! in the render path and is an accepted step-5 trade-off until path caching
//! is introduced.

use taffy::prelude::{span, Style};
use tiny_skia::PixmapMut;

use crate::{
    effect::{paint_fill, rounded_rect_path},
    paint::Rect,
    style::{Color, Theme},
};

use super::Widget;

pub const TILE_BASE_SIZE: i32 = 96;
pub const TILE_SMALL_WIDTH: i32 = TILE_BASE_SIZE;
pub const TILE_SMALL_HEIGHT: i32 = TILE_BASE_SIZE;
pub const TILE_MEDIUM_WIDTH: i32 = TILE_BASE_SIZE * 2;
pub const TILE_MEDIUM_HEIGHT: i32 = TILE_BASE_SIZE * 2;
pub const TILE_WIDE_WIDTH: i32 = TILE_BASE_SIZE * 4;
pub const TILE_WIDE_HEIGHT: i32 = TILE_BASE_SIZE * 2;
pub const TILE_LARGE_WIDTH: i32 = TILE_BASE_SIZE * 4;
pub const TILE_LARGE_HEIGHT: i32 = TILE_BASE_SIZE * 4;
pub const STRIPE_HEIGHT: i32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileSize {
    Small,
    Medium,
    Wide,
    Large,
}

impl TileSize {
    pub fn dimensions(self) -> (i32, i32) {
        match self {
            TileSize::Small => (TILE_SMALL_WIDTH, TILE_SMALL_HEIGHT),
            TileSize::Medium => (TILE_MEDIUM_WIDTH, TILE_MEDIUM_HEIGHT),
            TileSize::Wide => (TILE_WIDE_WIDTH, TILE_WIDE_HEIGHT),
            TileSize::Large => (TILE_LARGE_WIDTH, TILE_LARGE_HEIGHT),
        }
    }

    pub fn cell_span(self) -> (i32, i32) {
        match self {
            TileSize::Small => (1, 1),
            TileSize::Medium => (2, 2),
            TileSize::Wide => (4, 2),
            TileSize::Large => (4, 4),
        }
    }
}

pub struct Tile {
    label: &'static str,
    accent: Color,
    size: TileSize,
}

impl Tile {
    pub fn new(label: &'static str, accent: Color, size: TileSize) -> Self {
        Self {
            label,
            accent,
            size,
        }
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn accent(&self) -> Color {
        self.accent
    }

    pub fn size(&self) -> TileSize {
        self.size
    }
}

impl Widget for Tile {
    fn style(&self) -> Style {
        let (col_span, row_span) = self.size.cell_span();
        Style {
            grid_column: span(col_span as u16),
            grid_row: span(row_span as u16),
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

    use super::{
        Tile, TileSize, STRIPE_HEIGHT, TILE_LARGE_HEIGHT, TILE_LARGE_WIDTH, TILE_MEDIUM_HEIGHT,
        TILE_MEDIUM_WIDTH, TILE_SMALL_HEIGHT, TILE_SMALL_WIDTH, TILE_WIDE_HEIGHT, TILE_WIDE_WIDTH,
    };
    use crate::{paint::Rect, style::Palette, widget::Widget, Theme};

    #[test]
    fn tile_size_dimensions_match_win10_scale() {
        assert_eq!(
            TileSize::Small.dimensions(),
            (TILE_SMALL_WIDTH, TILE_SMALL_HEIGHT)
        );
        assert_eq!(
            TileSize::Medium.dimensions(),
            (TILE_MEDIUM_WIDTH, TILE_MEDIUM_HEIGHT)
        );
        assert_eq!(
            TileSize::Wide.dimensions(),
            (TILE_WIDE_WIDTH, TILE_WIDE_HEIGHT)
        );
        assert_eq!(
            TileSize::Large.dimensions(),
            (TILE_LARGE_WIDTH, TILE_LARGE_HEIGHT)
        );
    }

    #[test]
    fn tile_size_cell_span_matches_win10_scale() {
        assert_eq!(TileSize::Small.cell_span(), (1, 1));
        assert_eq!(TileSize::Medium.cell_span(), (2, 2));
        assert_eq!(TileSize::Wide.cell_span(), (4, 2));
        assert_eq!(TileSize::Large.cell_span(), (4, 4));
    }

    #[test]
    fn tile_new_stores_label_accent_and_size() {
        let tile = Tile::new(
            "hello",
            Palette::TOKYO_NIGHT_METRO.accent_alt,
            TileSize::Large,
        );
        assert_eq!(tile.label(), "hello");
        assert_eq!(tile.accent(), Palette::TOKYO_NIGHT_METRO.accent_alt);
        assert_eq!(tile.size(), TileSize::Large);
    }

    #[test]
    fn tile_style_forwards_cell_spans() {
        let small = Tile::new("small", Palette::TOKYO_NIGHT_METRO.accent, TileSize::Small).style();
        let medium =
            Tile::new("medium", Palette::TOKYO_NIGHT_METRO.accent, TileSize::Medium).style();
        let wide = Tile::new("wide", Palette::TOKYO_NIGHT_METRO.accent, TileSize::Wide).style();
        let large = Tile::new("large", Palette::TOKYO_NIGHT_METRO.accent, TileSize::Large).style();

        assert_eq!(small.grid_column, taffy::prelude::span(1));
        assert_eq!(small.grid_row, taffy::prelude::span(1));
        assert_eq!(medium.grid_column, taffy::prelude::span(2));
        assert_eq!(medium.grid_row, taffy::prelude::span(2));
        assert_eq!(wide.grid_column, taffy::prelude::span(4));
        assert_eq!(wide.grid_row, taffy::prelude::span(2));
        assert_eq!(large.grid_column, taffy::prelude::span(4));
        assert_eq!(large.grid_row, taffy::prelude::span(4));
    }

    #[test]
    fn tile_paint_draws_stripe_and_body_for_wide_tile() {
        let tile = Tile::new(
            "hello",
            Palette::TOKYO_NIGHT_METRO.accent_alt,
            TileSize::Wide,
        );
        let (width, height) = TileSize::Wide.dimensions();
        let mut pixmap = Pixmap::new(width as u32, height as u32).expect("pixmap");
        let mut canvas = pixmap.as_mut();
        tile.paint(
            Rect {
                x: 0,
                y: 0,
                width,
                height,
            },
            &mut canvas,
            &Theme::TOKYO_NIGHT_METRO,
        );
        drop(canvas);

        let stripe_px = pixmap.pixel((width / 2) as u32, 1).expect("stripe pixel");
        let body_px = pixmap
            .pixel((width / 2) as u32, (height / 2) as u32)
            .expect("body pixel");

        assert!(stripe_px.alpha() > 0);
        assert!(body_px.alpha() > 0);
        assert!(stripe_px.red() > body_px.red());
        assert!(stripe_px.blue() > body_px.blue());
        assert!(stripe_px.green() < stripe_px.blue());
        assert_eq!(STRIPE_HEIGHT, 4);
    }
}
