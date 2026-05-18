const SHADOW_SIGMA: f32 = 0.55;
/// Fraction of the corner's smaller dimension that stays at full
/// base_alpha (rounded inner contour). Beyond this, gaussian falloff
/// applies to the remaining distance.
const SHADOW_CORNER_INNER_FRACTION: f32 = 0.35;
/// Cap so very large corners don't get an absurdly wide flat core.
const SHADOW_CORNER_INNER_MAX_PX: f32 = 6.0;

fn corner_inner_inset(width_px: u32, height_px: u32) -> f32 {
    let smaller = width_px.min(height_px) as f32;
    (smaller * SHADOW_CORNER_INNER_FRACTION).min(SHADOW_CORNER_INNER_MAX_PX)
}

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

    let inner_x = (radius_px - 1) as f32;
    let inner_y = (radius_px - 1) as f32;
    let inset = corner_inner_inset(radius_px, radius_px);
    let outer_diag = (inner_x * inner_x + inner_y * inner_y).sqrt();
    let falloff_range = (outer_diag - inset).max(1.0);
    let mut out = vec![0u8; (radius_px * radius_px * 4) as usize];

    for y in 0..radius_px {
        for x in 0..radius_px {
            let dx = inner_x - x as f32;
            let dy = inner_y - y as f32;
            let d = (dx * dx + dy * dy).sqrt();
            let effective = (d - inset).max(0.0);
            let t = (effective / falloff_range).min(1.0);
            let alpha = base_alpha * gaussian_falloff(t);
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

pub(crate) fn rasterize_corner_rect(width_px: u32, height_px: u32, base_alpha: f32) -> Vec<u8> {
    if width_px == 0 || height_px == 0 {
        return Vec::new();
    }
    if width_px == height_px {
        return rasterize_corner(width_px, base_alpha);
    }

    let inner_x = (width_px - 1) as f32;
    let inner_y = (height_px - 1) as f32;
    let inset = corner_inner_inset(width_px, height_px);
    let outer_diag = (inner_x * inner_x + inner_y * inner_y).sqrt();
    let falloff_range = (outer_diag - inset).max(1.0);
    let mut out = vec![0u8; (width_px * height_px * 4) as usize];

    for y in 0..height_px {
        for x in 0..width_px {
            let dx = inner_x - x as f32;
            let dy = inner_y - y as f32;
            let d = (dx * dx + dy * dy).sqrt();
            let effective = (d - inset).max(0.0);
            let t = (effective / falloff_range).min(1.0);
            let alpha = base_alpha * gaussian_falloff(t);
            let a = alpha_to_u8(alpha);
            let off = ((y * width_px + x) * 4) as usize;
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
    use super::{
        flip_vertical, gaussian_falloff, rasterize_corner, rasterize_corner_rect,
        rasterize_edge_top,
    };

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
    fn test_rasterize_corner_rect_matches_square_when_equal_dims() {
        let r = 12u32;
        let square = rasterize_corner(r, 0.5);
        let rect = rasterize_corner_rect(r, r, 0.5);
        assert_eq!(square, rect);
    }

    #[test]
    fn test_rasterize_corner_rect_inner_corner_is_max_alpha() {
        let w = 40u32;
        let h = 12u32;
        let pixels = rasterize_corner_rect(w, h, 0.5);
        let off = (((h - 1) * w + (w - 1)) * 4 + 3) as usize;
        assert_eq!(pixels[off], 128);
    }

    #[test]
    fn test_rasterize_corner_rect_outer_corner_is_transparent() {
        let pixels = rasterize_corner_rect(40, 12, 0.5);
        assert_eq!(pixels[3], 0);
    }

    #[test]
    fn flip_vertical_of_corner_rect_places_max_alpha_at_top_row() {
        let w = 40u32;
        let h = 40u32;
        let pixels = rasterize_corner_rect(w, h, 1.0);
        let flipped = flip_vertical(&pixels, w, h);

        let top_right_alpha_off = ((w - 1) * 4 + 3) as usize;
        let bottom_right_alpha_off = (((h - 1) * w + (w - 1)) * 4 + 3) as usize;

        assert!(
            flipped[top_right_alpha_off] >= 250,
            "expected ~255, got {}",
            flipped[top_right_alpha_off]
        );
        assert!(
            flipped[bottom_right_alpha_off] <= 100,
            "expected low alpha, got {}",
            flipped[bottom_right_alpha_off]
        );
    }

    #[test]
    fn test_rasterize_corner_rounded_core_extends_inward() {
        let r = 16u32;
        let pixels = rasterize_corner(r, 1.0);
        let inner_off = (((r - 1) * r + (r - 1)) * 4 + 3) as usize;
        assert_eq!(pixels[inner_off], 255);
        let near_off = ((12 * r + 12) * 4 + 3) as usize;
        assert_eq!(
            pixels[near_off], 255,
            "pixel within inset should be max alpha"
        );
        let outer_off = ((r + 1) * 4 + 3) as usize;
        assert!(
            pixels[outer_off] < 30,
            "pixel near outer corner should be near zero, got {}",
            pixels[outer_off]
        );
    }

    #[test]
    fn test_rasterize_corner_rect_top_keeps_rounded_core_for_asymmetric() {
        let pixels = rasterize_corner_rect(16, 8, 1.0);
        let inner_off = ((7 * 16 + 15) * 4 + 3) as usize;
        assert_eq!(pixels[inner_off], 255);
        let near_off = ((7 * 16 + 13) * 4 + 3) as usize;
        assert_eq!(pixels[near_off], 255);
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
