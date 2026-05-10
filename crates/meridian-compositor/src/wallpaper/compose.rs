use image::{imageops::FilterType, DynamicImage, Rgba, RgbaImage};
use meridian_config::WallpaperMode;
use tracing::warn;

const DEFAULT_WALLPAPER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/default_wallpaper.png"));

pub(super) fn load_default_image() -> DynamicImage {
    match image::load_from_memory(DEFAULT_WALLPAPER) {
        Ok(img) => img,
        Err(err) => {
            warn!("Failed to decode built-in default wallpaper: {err}");
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([0x1e, 0x1e, 0x2e, 0xff])))
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
    let pixel = [0x1e_u8, 0x1e, 0x2e, 0xff];
    pixel
        .iter()
        .copied()
        .cycle()
        .take((w * h * 4) as usize)
        .collect()
}
