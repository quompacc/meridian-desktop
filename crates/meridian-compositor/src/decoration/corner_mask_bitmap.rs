fn alpha_to_u8(alpha: f32) -> u8 {
    (alpha.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub(crate) fn rasterize_corner_mask(radius_px: u32, color_rgba: [u8; 4]) -> (Vec<u8>, u32) {
    if radius_px == 0 {
        return (Vec::new(), 0);
    }

    const SUPERSAMPLE_FACTOR: u32 = 2;
    const SAMPLES: u32 = 8;
    let internal_size = radius_px * SUPERSAMPLE_FACTOR;
    let r = internal_size as f32;
    let center_x = (internal_size - 1) as f32;
    let center_y = (internal_size - 1) as f32;
    let r_sq = r * r;
    let src_alpha = color_rgba[3] as f32 / 255.0;
    let mut out = vec![0u8; (internal_size * internal_size * 4) as usize];

    for y in 0..internal_size {
        for x in 0..internal_size {
            let mut covered = 0u32;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let fx = x as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
                    let fy = y as f32 + (sy as f32 + 0.5) / SAMPLES as f32;
                    let dx = fx - center_x;
                    let dy = fy - center_y;
                    if dx * dx + dy * dy <= r_sq {
                        covered += 1;
                    }
                }
            }

            if covered == 0 {
                continue;
            }

            let coverage = covered as f32 / (SAMPLES * SAMPLES) as f32;
            let alpha = coverage * src_alpha;
            let off = ((y * internal_size + x) * 4) as usize;
            out[off] = (color_rgba[0] as f32 * alpha).round() as u8;
            out[off + 1] = (color_rgba[1] as f32 * alpha).round() as u8;
            out[off + 2] = (color_rgba[2] as f32 * alpha).round() as u8;
            out[off + 3] = alpha_to_u8(alpha);
        }
    }

    (out, internal_size)
}

pub(crate) fn rasterize_rounded_rect(
    width_px: u32,
    height_px: u32,
    corner_radius: u32,
    color_rgba: [u8; 4],
) -> Vec<u8> {
    if width_px == 0 || height_px == 0 {
        return Vec::new();
    }

    const SAMPLES: u32 = 8;
    let max_corner = (width_px.min(height_px) / 2).max(1);
    let cr = corner_radius.min(max_corner);
    if cr == 0 {
        let mut out = vec![0u8; (width_px * height_px * 4) as usize];
        let alpha = color_rgba[3] as f32 / 255.0;
        let a = alpha_to_u8(alpha);
        for chunk in out.chunks_exact_mut(4) {
            chunk[0] = (color_rgba[0] as f32 * alpha).round() as u8;
            chunk[1] = (color_rgba[1] as f32 * alpha).round() as u8;
            chunk[2] = (color_rgba[2] as f32 * alpha).round() as u8;
            chunk[3] = a;
        }
        return out;
    }

    let src_alpha = color_rgba[3] as f32 / 255.0;
    let r = cr as f32;
    let r_sq = r * r;
    let left = cr;
    let right = width_px.saturating_sub(cr);
    let top = cr;
    let bottom = height_px.saturating_sub(cr);

    let mut out = vec![0u8; (width_px * height_px * 4) as usize];
    for y in 0..height_px {
        for x in 0..width_px {
            let mut covered = 0u32;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let fx = x as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
                    let fy = y as f32 + (sy as f32 + 0.5) / SAMPLES as f32;
                    let inside = if x < left && y < top {
                        let cx = cr as f32;
                        let cy = cr as f32;
                        let dx = fx - cx;
                        let dy = fy - cy;
                        dx * dx + dy * dy <= r_sq
                    } else if x >= right && y < top {
                        let cx = width_px as f32 - cr as f32 - 1.0;
                        let cy = cr as f32;
                        let dx = fx - cx;
                        let dy = fy - cy;
                        dx * dx + dy * dy <= r_sq
                    } else if x < left && y >= bottom {
                        let cx = cr as f32;
                        let cy = height_px as f32 - cr as f32 - 1.0;
                        let dx = fx - cx;
                        let dy = fy - cy;
                        dx * dx + dy * dy <= r_sq
                    } else if x >= right && y >= bottom {
                        let cx = width_px as f32 - cr as f32 - 1.0;
                        let cy = height_px as f32 - cr as f32 - 1.0;
                        let dx = fx - cx;
                        let dy = fy - cy;
                        dx * dx + dy * dy <= r_sq
                    } else {
                        true
                    };

                    if inside {
                        covered += 1;
                    }
                }
            }

            if covered == 0 {
                continue;
            }

            let coverage = covered as f32 / (SAMPLES * SAMPLES) as f32;
            let alpha = coverage * src_alpha;
            let off = ((y * width_px + x) * 4) as usize;
            out[off] = (color_rgba[0] as f32 * alpha).round() as u8;
            out[off + 1] = (color_rgba[1] as f32 * alpha).round() as u8;
            out[off + 2] = (color_rgba[2] as f32 * alpha).round() as u8;
            out[off + 3] = alpha_to_u8(alpha);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{rasterize_corner_mask, rasterize_rounded_rect};

    #[test]
    fn test_corner_mask_outer_corner_transparent() {
        let (pixels, _) = rasterize_corner_mask(12, [100, 150, 200, 255]);
        assert_eq!(pixels[3], 0);
    }

    #[test]
    fn test_corner_mask_inner_corner_opaque() {
        let radius = 12u32;
        let alpha = 200u8;
        let (pixels, internal_size) = rasterize_corner_mask(radius, [100, 150, 200, alpha]);
        let off = ((internal_size * internal_size - 1) * 4 + 3) as usize;
        assert_eq!(pixels[off], alpha);
    }

    #[test]
    fn test_corner_mask_color_premultiplied() {
        let (pixels, internal_size) = rasterize_corner_mask(12, [120, 80, 40, 128]);
        let off = (((internal_size - 1) * internal_size + (internal_size - 8)) * 4) as usize;
        let a = pixels[off + 3] as u32;
        if a > 0 {
            assert!(pixels[off] as u32 <= a);
            assert!(pixels[off + 1] as u32 <= a);
            assert!(pixels[off + 2] as u32 <= a);
        }
    }

    #[test]
    fn test_rounded_rect_corner_pixels_transparent() {
        let pixels = rasterize_rounded_rect(28, 24, 6, [255, 0, 0, 255]);
        let idx = |x: u32, y: u32| ((y * 28 + x) * 4 + 3) as usize;
        assert_eq!(pixels[idx(0, 0)], 0);
        assert_eq!(pixels[idx(27, 0)], 0);
        assert_eq!(pixels[idx(0, 23)], 0);
        assert_eq!(pixels[idx(27, 23)], 0);
    }

    #[test]
    fn test_rounded_rect_center_pixel_opaque() {
        let pixels = rasterize_rounded_rect(28, 24, 6, [10, 20, 30, 200]);
        let off = ((12 * 28 + 14) * 4) as usize;
        assert_eq!(pixels[off + 3], 200);
    }
}
