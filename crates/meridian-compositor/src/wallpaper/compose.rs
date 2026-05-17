use image::{imageops::FilterType, DynamicImage, Rgba, RgbaImage};
use meridian_config::WallpaperMode;
use tracing::warn;

const DEFAULT_WALLPAPER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/default_wallpaper.png"));

pub(super) fn load_default_image() -> DynamicImage {
    match image::load_from_memory(DEFAULT_WALLPAPER) {
        Ok(img) => img,
        Err(err) => {
            warn!("Failed to decode built-in default wallpaper: {err}");
            // TODO Phase 5: aus ThemeManager beziehen.
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([0x1a, 0x1b, 0x26, 0xff])))
        }
    }
}

pub(super) fn compose_for_size(
    image: Option<&RgbaImage>,
    mode: WallpaperMode,
    out_w: u32,
    out_h: u32,
) -> Vec<u8> {
    let Some(img) = image else {
        return solid_fallback(out_w, out_h);
    };

    let (src_w, src_h) = (img.width(), img.height());
    match mode {
        WallpaperMode::Fill => {
            image::imageops::resize(img, out_w, out_h, FilterType::Lanczos3).into_raw()
        }
        WallpaperMode::Fit => {
            let scale = (out_w as f32 / src_w as f32).min(out_h as f32 / src_h as f32);
            let sw = ((src_w as f32 * scale) as u32).max(1);
            let sh = ((src_h as f32 * scale) as u32).max(1);
            let scaled = image::imageops::resize(img, sw, sh, FilterType::Lanczos3);
            let mut canvas = RgbaImage::new(out_w, out_h);
            let ox = (out_w.saturating_sub(sw)) / 2;
            let oy = (out_h.saturating_sub(sh)) / 2;
            image::imageops::overlay(&mut canvas, &scaled, ox as i64, oy as i64);
            canvas.into_raw()
        }
        WallpaperMode::Center => {
            let mut canvas = RgbaImage::new(out_w, out_h);
            let ox = (out_w as i64 - src_w as i64) / 2;
            let oy = (out_h as i64 - src_h as i64) / 2;
            image::imageops::overlay(&mut canvas, img, ox, oy);
            canvas.into_raw()
        }
        WallpaperMode::Tile => {
            let mut canvas = RgbaImage::new(out_w, out_h);
            let mut y = 0i64;
            while y < out_h as i64 {
                let mut x = 0i64;
                while x < out_w as i64 {
                    image::imageops::overlay(&mut canvas, img, x, y);
                    x += src_w as i64;
                }
                y += src_h as i64;
            }
            canvas.into_raw()
        }
    }
}

fn solid_fallback(w: u32, h: u32) -> Vec<u8> {
    let pixel = [0x1a_u8, 0x1b, 0x26, 0xff];
    pixel
        .iter()
        .copied()
        .cycle()
        .take((w * h * 4) as usize)
        .collect()
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};
    use meridian_config::WallpaperMode;

    use super::compose_for_size;

    fn pixel(raw: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let idx = ((y * width + x) * 4) as usize;
        [raw[idx], raw[idx + 1], raw[idx + 2], raw[idx + 3]]
    }

    #[test]
    fn compose_without_image_uses_expected_solid_fallback() {
        let out = compose_for_size(None, WallpaperMode::Fill, 3, 2);

        assert_eq!(out.len(), 3 * 2 * 4);
        for chunk in out.chunks_exact(4) {
            assert_eq!(chunk, [0x1a, 0x1b, 0x26, 0xff]);
        }
    }

    #[test]
    fn center_places_single_pixel_in_canvas_center() {
        let mut src = RgbaImage::new(1, 1);
        src.put_pixel(0, 0, Rgba([10, 20, 30, 40]));

        let out = compose_for_size(Some(&src), WallpaperMode::Center, 3, 3);

        assert_eq!(out.len(), 3 * 3 * 4);
        assert_eq!(pixel(&out, 3, 1, 1), [10, 20, 30, 40]);
        assert_eq!(pixel(&out, 3, 0, 0), [0, 0, 0, 0]);
        assert_eq!(pixel(&out, 3, 2, 2), [0, 0, 0, 0]);
    }

    #[test]
    fn tile_repeats_source_pattern() {
        let mut src = RgbaImage::new(2, 2);
        src.put_pixel(0, 0, Rgba([1, 2, 3, 255]));
        src.put_pixel(1, 0, Rgba([4, 5, 6, 255]));
        src.put_pixel(0, 1, Rgba([7, 8, 9, 255]));
        src.put_pixel(1, 1, Rgba([10, 11, 12, 255]));

        let out = compose_for_size(Some(&src), WallpaperMode::Tile, 3, 3);

        assert_eq!(out.len(), 3 * 3 * 4);
        assert_eq!(pixel(&out, 3, 0, 0), [1, 2, 3, 255]);
        assert_eq!(pixel(&out, 3, 1, 0), [4, 5, 6, 255]);
        assert_eq!(pixel(&out, 3, 2, 0), [1, 2, 3, 255]);
        assert_eq!(pixel(&out, 3, 0, 2), [1, 2, 3, 255]);
        assert_eq!(pixel(&out, 3, 2, 2), [1, 2, 3, 255]);
    }

    #[test]
    fn fill_and_fit_modes_preserve_expected_uniform_and_letterbox_behavior() {
        let mut src = RgbaImage::new(2, 2);
        for y in 0..2 {
            for x in 0..2 {
                src.put_pixel(x, y, Rgba([33, 44, 55, 255]));
            }
        }

        let fill = compose_for_size(Some(&src), WallpaperMode::Fill, 5, 3);
        let fit = compose_for_size(Some(&src), WallpaperMode::Fit, 5, 3);

        assert_eq!(fill.len(), 5 * 3 * 4);
        assert_eq!(fit.len(), 5 * 3 * 4);

        for chunk in fill.chunks_exact(4) {
            assert_eq!(chunk, [33, 44, 55, 255]);
        }

        // Fit keeps aspect ratio, so a 2:2 source in a 5x3 target is centered
        // with transparent left/right padding columns.
        for y in 0..3 {
            assert_eq!(pixel(&fit, 5, 0, y), [0, 0, 0, 0]);
            assert_eq!(pixel(&fit, 5, 4, y), [0, 0, 0, 0]);
            for x in 1..4 {
                assert_eq!(pixel(&fit, 5, x, y), [33, 44, 55, 255]);
            }
        }
    }
}
