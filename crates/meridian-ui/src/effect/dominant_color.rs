use tiny_skia::Pixmap;

use crate::style::Color;

/// Extract the dominant (most visually prominent) color from an icon pixmap.
///
/// Weights each opaque pixel by saturation² × alpha, returns a weighted
/// average. Falls back to `fallback` for grayscale or transparent icons.
pub fn dominant_color(pixmap: &Pixmap, fallback: Color) -> Color {
    let mut weight_sum: f32 = 0.0;
    let mut r_sum: f32 = 0.0;
    let mut g_sum: f32 = 0.0;
    let mut b_sum: f32 = 0.0;

    for px in pixmap.pixels() {
        let a = px.alpha();
        if a < 32 {
            continue;
        }
        let af = a as f32 * (1.0 / 255.0);
        // demultiply premultiplied alpha channels
        let inv_a = 1.0 / af;
        let r = (px.red() as f32 * (1.0 / 255.0) * inv_a).min(1.0);
        let g = (px.green() as f32 * (1.0 / 255.0) * inv_a).min(1.0);
        let b = (px.blue() as f32 * (1.0 / 255.0) * inv_a).min(1.0);

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let saturation = max - min;

        // Weight by saturation² × alpha — strongly prefers vivid, opaque pixels
        let weight = saturation * saturation * af;
        if weight < 1e-5 {
            continue;
        }
        weight_sum += weight;
        r_sum += r * weight;
        g_sum += g * weight;
        b_sum += b * weight;
    }

    if weight_sum < 1e-5 {
        return fallback;
    }

    let r = ((r_sum / weight_sum) * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = ((g_sum / weight_sum) * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = ((b_sum / weight_sum) * 255.0).round().clamp(0.0, 255.0) as u8;

    // Reject near-black results (icon had color but overall too dark)
    if r.max(g).max(b) < 60 {
        return fallback;
    }

    Color::rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use tiny_skia::{Pixmap, PremultipliedColorU8};

    use super::dominant_color;
    use crate::style::Color;

    fn solid_pixmap(r: u8, g: u8, b: u8, a: u8) -> Pixmap {
        let mut pm = Pixmap::new(4, 4).unwrap();
        let pr = (r as u16 * a as u16 / 255) as u8;
        let pg = (g as u16 * a as u16 / 255) as u8;
        let pb = (b as u16 * a as u16 / 255) as u8;
        for px in pm.pixels_mut() {
            *px = PremultipliedColorU8::from_rgba(pr, pg, pb, a).unwrap();
        }
        pm
    }

    const FALLBACK: Color = Color::rgb(0x7a, 0xa2, 0xf7);

    #[test]
    fn solid_red_returns_red() {
        let pm = solid_pixmap(220, 40, 40, 255);
        let c = dominant_color(&pm, FALLBACK);
        assert!(c.r > 180, "expected red-dominant, got {:?}", c);
        assert!(c.g < 80);
        assert!(c.b < 80);
    }

    #[test]
    fn solid_blue_returns_blue() {
        let pm = solid_pixmap(30, 60, 210, 255);
        let c = dominant_color(&pm, FALLBACK);
        assert!(c.b > 180, "expected blue-dominant, got {:?}", c);
        assert!(c.r < 80);
    }

    #[test]
    fn grayscale_returns_fallback() {
        let pm = solid_pixmap(128, 128, 128, 255);
        let c = dominant_color(&pm, FALLBACK);
        assert_eq!(c, FALLBACK, "grayscale should return fallback");
    }

    #[test]
    fn transparent_returns_fallback() {
        let pm = solid_pixmap(255, 0, 0, 0);
        let c = dominant_color(&pm, FALLBACK);
        assert_eq!(c, FALLBACK, "transparent should return fallback");
    }

    #[test]
    fn near_black_returns_fallback() {
        let pm = solid_pixmap(20, 5, 15, 255);
        let c = dominant_color(&pm, FALLBACK);
        assert_eq!(c, FALLBACK, "near-black should return fallback");
    }
}
