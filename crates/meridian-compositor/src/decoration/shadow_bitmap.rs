const SHADOW_SIGMA: f32 = 0.55;

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

pub(crate) fn rasterize_corner(radius_px: u32, base_alpha: f32) -> Vec<u8> {
    if radius_px == 0 {
        return Vec::new();
    }

    let sr = radius_px as f32;
    let inner_x = (radius_px - 1) as f32;
    let inner_y = (radius_px - 1) as f32;
    let mut out = vec![0u8; (radius_px * radius_px * 4) as usize];

    for y in 0..radius_px {
        for x in 0..radius_px {
            let dx = inner_x - x as f32;
            let dy = inner_y - y as f32;
            let dist = (dx * dx + dy * dy).sqrt() / sr;
            let alpha = base_alpha * gaussian_falloff(dist.min(1.0));
            let a = alpha_to_u8(alpha);
            let off = ((y * radius_px + x) * 4) as usize;
            out[off] = 0;
            out[off + 1] = 0;
            out[off + 2] = 0;
            out[off + 3] = a;
        }
    }

    out
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
    use super::{gaussian_falloff, rasterize_corner, rasterize_edge_top};

    #[test]
    fn test_gaussian_falloff_unit_interval() {
        assert_eq!(gaussian_falloff(0.0), 1.0);
        assert_eq!(gaussian_falloff(1.0), 0.0);
    }

    #[test]
    fn test_rasterize_corner_inner_corner_is_max_alpha() {
        let r = 24u32;
        let pixels = rasterize_corner(r, 0.5);
        let off = ((r * r - 1) * 4 + 3) as usize;
        assert_eq!(pixels[off], 128);
    }

    #[test]
    fn test_rasterize_corner_outer_corner_is_transparent() {
        let pixels = rasterize_corner(24, 0.5);
        assert_eq!(pixels[3], 0);
    }

    #[test]
    fn test_rasterize_corner_diagonal_symmetry() {
        let r = 24u32;
        let pixels = rasterize_corner(r, 0.5);
        let p1 = ((2 * r + 6) * 4 + 3) as usize;
        let p2 = ((6 * r + 2) * 4 + 3) as usize;
        assert_eq!(pixels[p1], pixels[p2]);
    }

    #[test]
    fn test_rasterize_edge_top_gradient_descends() {
        let r = 24u32;
        let pixels = rasterize_edge_top(r, 0.5);
        let top = pixels[3];
        let near_frame = pixels[((r - 1) * 4 + 3) as usize];
        assert!(top < near_frame);
    }
}
