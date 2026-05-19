//! UI preview sandbox renderer for `--ui-preview`.
//!
//! This path allocates a fresh `tiny_skia::Pixmap` per draw call and is intended
//! for low-frequency sandbox previewing. If this is promoted to steady-state UI,
//! pixmap reuse should be added.

use meridian_ui::{
    compute_layout, render,
    style::Palette,
    widget::{Container, Widget},
    PixelSize, Theme, Tile,
};
use tiny_skia::Pixmap;

pub(crate) fn draw_ui_preview_sandbox(canvas: &mut [u8], width: u32, height: u32) {
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

    let root = Container::centered_viewport(
        width,
        height,
        vec![
            Box::new(Tile::new("hello", Palette::TOKYO_NIGHT_METRO.accent_alt)) as Box<dyn Widget>,
        ],
    );

    if let Ok(layout) = compute_layout(&root, PixelSize { width, height }) {
        let mut pixmap_canvas = pixmap.as_mut();
        let _ = render(&root, &layout, &mut pixmap_canvas, &theme);
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
    use super::{blit_rgba_to_argb, draw_ui_preview_sandbox};

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

        draw_ui_preview_sandbox(&mut canvas, width, height);

        assert!(canvas.iter().any(|byte| *byte != 0));
    }
}
