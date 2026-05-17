const SHADOW_SIGMA: f32 = 0.4;

fn alpha_to_u8(alpha: f32) -> u8 {
    (alpha.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub(crate) fn gaussian_falloff(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let s = SHADOW_SIGMA;
    let raw = (-t * t / (2.0 * s * s)).exp();
    let tail = (-1.0 / (2.0 * s * s)).exp();
    ((raw - tail) / (1.0 - tail)).clamp(0.0, 1.0)
}

pub(crate) fn rasterize_shadow_corner_with_frame(
    shadow_radius_px: u32,
    frame_radius_px: u32,
    base_alpha: f32,
) -> (Vec<u8>, u32) {
    if shadow_radius_px == 0 {
        return (Vec::new(), 0);
    }

    let sr = shadow_radius_px as f32;
    let cr = frame_radius_px as f32;
    let size = shadow_radius_px + frame_radius_px;
    let frame_cx = (size - 1) as f32;
    let frame_cy = (size - 1) as f32;
    let mut out = vec![0u8; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - frame_cx;
            let dy = y as f32 - frame_cy;
            let d = (dx * dx + dy * dy).sqrt();
            if d <= cr {
                continue;
            }
            let t = ((d - cr) / sr).clamp(0.0, 1.0);
            if t >= 1.0 {
                continue;
            }
            let alpha = base_alpha * gaussian_falloff(t);
            if alpha <= 0.0 {
                continue;
            }
            let a = alpha_to_u8(alpha);
            let off = ((y * size + x) * 4) as usize;
            out[off] = 0;
            out[off + 1] = 0;
            out[off + 2] = 0;
            out[off + 3] = a;
        }
    }

    (out, size)
}

pub(crate) fn rasterize_edge_top(radius_px: u32, base_alpha: f32) -> Vec<u8> {
    if radius_px == 0 {
        return Vec::new();
    }

    let sr = radius_px as f32;
    let mut out = vec![0u8; (radius_px * 4) as usize];
    for y in 0..radius_px {
        let t = 1.0 - (y as f32 / (sr - 1.0).max(1.0));
        let alpha = base_alpha * gaussian_falloff(t);
        let a = alpha_to_u8(alpha);
        let off = (y * 4) as usize;
        out[off] = 0;
        out[off + 1] = 0;
        out[off + 2] = 0;
        out[off + 3] = a;
    }
    out
}

pub(crate) fn rasterize_edge_left(radius_px: u32, base_alpha: f32) -> Vec<u8> {
    if radius_px == 0 {
        return Vec::new();
    }

    let sr = radius_px as f32;
    let mut out = vec![0u8; (radius_px * 4) as usize];
    for x in 0..radius_px {
        let t = 1.0 - (x as f32 / (sr - 1.0).max(1.0));
        let alpha = base_alpha * gaussian_falloff(t);
        let a = alpha_to_u8(alpha);
        let off = (x * 4) as usize;
        out[off] = 0;
        out[off + 1] = 0;
        out[off + 2] = 0;
        out[off + 3] = a;
    }
    out
}

pub(crate) fn flip_horizontal(pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = vec![0u8; pixels.len()];
    for y in 0..height {
        for x in 0..width {
            let src = ((y * width + x) * 4) as usize;
            let dst = ((y * width + (width - 1 - x)) * 4) as usize;
            out[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
        }
    }
    out
}

pub(crate) fn flip_vertical(pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = vec![0u8; pixels.len()];
    for y in 0..height {
        for x in 0..width {
            let src = ((y * width + x) * 4) as usize;
            let dst = (((height - 1 - y) * width + x) * 4) as usize;
            out[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{gaussian_falloff, rasterize_edge_top, rasterize_shadow_corner_with_frame};

    #[test]
    fn test_gaussian_falloff_unit_interval() {
        assert_eq!(gaussian_falloff(0.0), 1.0);
        assert_eq!(gaussian_falloff(1.0), 0.0);
    }

    #[test]
    fn test_rasterize_edge_top_gradient_descends() {
        let r = 16u32;
        let pixels = rasterize_edge_top(r, 0.5);
        let top = pixels[3];
        let near_frame = pixels[((r - 1) * 4 + 3) as usize];
        assert!(top < near_frame);
    }

    #[test]
    fn test_rasterize_shadow_corner_with_frame_inside_frame_is_zero() {
        let r = 24u32;
        let (pixels, size) = rasterize_shadow_corner_with_frame(r, 0, 0.5);
        assert_eq!(size, r);
        let off = ((r * r - 1) * 4 + 3) as usize;
        assert_eq!(pixels[off], 0);
    }

    #[test]
    fn test_rasterize_shadow_corner_with_frame_outer_pixel_transparent() {
        let (pixels, size) = rasterize_shadow_corner_with_frame(24, 12, 0.5);
        assert_eq!(pixels.len(), (size * size * 4) as usize);
        assert_eq!(size, 36);
        assert_eq!(pixels[3], 0);
    }

    #[test]
    fn test_rasterize_shadow_corner_with_frame_curve_is_continuous() {
        let (pixels, _) = rasterize_shadow_corner_with_frame(24, 12, 0.5);
        assert!(pixels.chunks_exact(4).any(|px| px[3] > 0));
    }
}
