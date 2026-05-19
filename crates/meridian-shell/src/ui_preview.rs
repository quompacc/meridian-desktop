//! UI preview sandbox renderer for `--ui-preview`.
//!
//! This path allocates a fresh `tiny_skia::Pixmap` per draw call and is intended
//! for low-frequency sandbox previewing. If this is promoted to steady-state UI,
//! pixmap reuse should be added.

use meridian_ui::{
    compute_layout, render,
    style::Palette,
    widget::{tile::TILE_BASE_SIZE, Button, Container, Widget},
    PixelSize, Theme, Tile, TileSize, WidgetState,
};
use tiny_skia::Pixmap;

const FOOTER_HEIGHT: i32 = 56;
const FOOTER_PADDING_X: i32 = 28;
const FOOTER_CLUSTER_GAP: i32 = 8;
const FOOTER_SWITCH_WIDTH: i32 = 144;
const FOOTER_SWITCH_HEIGHT: i32 = 48;
const FOOTER_POWER_BUTTON_SIZE: i32 = 48;

pub(crate) fn build_ui_preview_widget_tree(width: u32, height: u32) -> Box<dyn Widget> {
    let theme = Theme::TOKYO_NIGHT_METRO;
    let gap = theme.spacing.md;
    let pal = Palette::TOKYO_NIGHT_METRO;
    let tiles: Vec<Box<dyn Widget>> = vec![
        Box::new(Tile::new("Mail", pal.accent_alt, TileSize::Large)),
        Box::new(Tile::new("Edge", pal.accent, TileSize::Wide)),
        Box::new(Tile::new("OneDrive", pal.warning, TileSize::Wide)),
        Box::new(Tile::new("Photos", pal.accent, TileSize::Small)),
        Box::new(Tile::new("Music", pal.accent_alt, TileSize::Small)),
        Box::new(Tile::new("Maps", pal.success, TileSize::Small)),
        Box::new(Tile::new("News", pal.warning, TileSize::Small)),
        Box::new(Tile::new("Store", pal.error, TileSize::Small)),
        Box::new(Tile::new("Calendar", pal.accent, TileSize::Small)),
        Box::new(Tile::new("Weather", pal.accent_alt, TileSize::Small)),
        Box::new(Tile::new("Notes", pal.success, TileSize::Small)),
    ];
    let mosaic_height = height.saturating_sub(FOOTER_HEIGHT.max(0) as u32);
    let mosaic_grid = Container::grid(TILE_BASE_SIZE, 8, gap, width, mosaic_height, tiles);
    let mosaic_section = Container::centered_viewport(
        width,
        mosaic_height,
        vec![Box::new(mosaic_grid) as Box<dyn Widget>],
    );

    let footer_left = vec![Box::new(Button::new(
        "Apps",
        pal.accent,
        FOOTER_SWITCH_WIDTH,
        FOOTER_SWITCH_HEIGHT,
    )) as Box<dyn Widget>];
    let footer_right = vec![
        Box::new(Button::new(
            "Off",
            pal.error,
            FOOTER_POWER_BUTTON_SIZE,
            FOOTER_POWER_BUTTON_SIZE,
        )) as Box<dyn Widget>,
        Box::new(Button::new(
            "Rst",
            pal.warning,
            FOOTER_POWER_BUTTON_SIZE,
            FOOTER_POWER_BUTTON_SIZE,
        )) as Box<dyn Widget>,
        Box::new(Button::new(
            "Zzz",
            pal.accent,
            FOOTER_POWER_BUTTON_SIZE,
            FOOTER_POWER_BUTTON_SIZE,
        )) as Box<dyn Widget>,
        Box::new(Button::new(
            "Lock",
            pal.accent_alt,
            FOOTER_POWER_BUTTON_SIZE,
            FOOTER_POWER_BUTTON_SIZE,
        )) as Box<dyn Widget>,
        Box::new(Button::new(
            "Out",
            pal.success,
            FOOTER_POWER_BUTTON_SIZE,
            FOOTER_POWER_BUTTON_SIZE,
        )) as Box<dyn Widget>,
    ];
    let footer = Container::footer_row(
        width,
        FOOTER_HEIGHT,
        FOOTER_PADDING_X,
        FOOTER_CLUSTER_GAP,
        footer_left,
        footer_right,
    );

    Box::new(Container::column(
        0,
        vec![
            Box::new(mosaic_section) as Box<dyn Widget>,
            Box::new(footer) as Box<dyn Widget>,
        ],
    ))
}

pub(crate) fn draw_ui_preview_sandbox(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    state_fn: &dyn Fn(&[usize]) -> WidgetState,
) {
    let expected_len = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if canvas.len() != expected_len {
        return;
    }

    let Some(mut pixmap) = Pixmap::new(width, height) else {
        return;
    };

    let theme = Theme::TOKYO_NIGHT_METRO;
    pixmap.fill(to_tiny_skia_color(theme.palette.background));

    let root = build_ui_preview_widget_tree(width, height);

    if let Ok(layout) = compute_layout(&*root, PixelSize { width, height }) {
        let mut pixmap_canvas = pixmap.as_mut();
        let _ = render(&*root, &layout, &mut pixmap_canvas, &theme, state_fn);
    }

    blit_rgba_to_argb(pixmap.data(), canvas);
}

fn blit_rgba_to_argb(src: &[u8], dst: &mut [u8]) {
    if src.len() != dst.len() || !src.len().is_multiple_of(4) {
        return;
    }

    for (rgba, argb) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        argb[0] = rgba[2];
        argb[1] = rgba[1];
        argb[2] = rgba[0];
        argb[3] = rgba[3];
    }
}

fn to_tiny_skia_color(color: meridian_ui::style::Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(color.r, color.g, color.b, color.a)
}

#[cfg(test)]
mod tests {
    use meridian_ui::WidgetState;

    use super::{blit_rgba_to_argb, build_ui_preview_widget_tree, draw_ui_preview_sandbox};

    #[test]
    fn blit_rgba_to_argb_swaps_red_and_blue() {
        let src = [0x12, 0x34, 0x56, 0x78];
        let mut dst = [0_u8; 4];

        blit_rgba_to_argb(&src, &mut dst);

        assert_eq!(dst, [0x56, 0x34, 0x12, 0x78]);
    }

    #[test]
    fn blit_twice_roundtrips_to_original() {
        let src = [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0];
        let mut mid = [0_u8; 8];
        let mut dst = [0_u8; 8];

        blit_rgba_to_argb(&src, &mut mid);
        blit_rgba_to_argb(&mid, &mut dst);

        assert_eq!(dst, src);
    }

    #[test]
    fn draw_ui_preview_sandbox_smoke_modifies_canvas() {
        let width = 128;
        let height = 96;
        let mut canvas = vec![0_u8; (width * height * 4) as usize];

        draw_ui_preview_sandbox(&mut canvas, width, height, &|_| WidgetState::Idle);

        assert!(canvas.iter().any(|byte| *byte != 0));
    }

    #[test]
    fn build_ui_preview_widget_tree_has_root_column_with_two_sections() {
        let tree = build_ui_preview_widget_tree(880, 620);
        let children = tree.children();
        assert_eq!(children.len(), 2, "root column should have 2 children");
        // Mosaic section
        let mosaic = &children[0];
        assert!(!mosaic.children().is_empty(), "mosaic should contain grid");
        // Footer
        let footer = &children[1];
        assert!(
            !footer.children().is_empty(),
            "footer should contain left/right clusters"
        );
    }
}
