//! Text rendering via fontdue.
//!
//! The Adwaita Sans Regular font is embedded at compile time. `ui_font()`
//! parses it on first access (OnceLock) and hands out a `&'static fontdue::Font`
//! thereafter.
//!
//! `paint_text` rasterizes each glyph on demand and alpha-blends it onto the
//! canvas. Allocation per call (fontdue's rasterize returns a fresh Vec<u8>
//! per glyph) — acceptable for the current low-frequency render path; a
//! glyph-cache wrapper is a later optimization.

use std::sync::OnceLock;

use fontdue::{Font, FontSettings};
use tiny_skia::PixmapMut;

use crate::style::Color;

const UI_FONT_DATA: &[u8] = include_bytes!("../../assets/fonts/AdwaitaSans-Regular.ttf");

/// Glyph coverage exponent (< 1.0 thickens strokes slightly). macOS-style text
/// renders a touch heavier than the raw outline coverage; this "stem darkening"
/// together with the gamma-correct blend below is what stops light text on dark
/// backgrounds from looking thin and muddy.
const COVERAGE_GAMMA: f32 = 0.82;

#[inline]
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

static UI_FONT: OnceLock<Font> = OnceLock::new();

pub fn ui_font() -> &'static Font {
    UI_FONT.get_or_init(|| {
        Font::from_bytes(UI_FONT_DATA, FontSettings::default())
            .expect("embedded Adwaita Sans Regular parses")
    })
}

pub fn measure_text(text: &str, size_px: f32) -> (i32, i32) {
    if text.is_empty() {
        return (0, 0);
    }
    let font = ui_font();
    let mut width: f32 = 0.0;
    let mut max_above: i32 = 0;
    let mut max_below: i32 = 0;
    for c in text.chars() {
        let metrics = font.metrics(c, size_px);
        width += metrics.advance_width;
        let above = (metrics.height as i32 + metrics.ymin).max(0);
        let below = (-metrics.ymin).max(0);
        if above > max_above {
            max_above = above;
        }
        if below > max_below {
            max_below = below;
        }
    }
    (width.round() as i32, max_above + max_below)
}

pub fn paint_text(
    canvas: &mut PixmapMut<'_>,
    text: &str,
    x: i32,
    baseline: i32,
    size_px: f32,
    color: Color,
) {
    if text.is_empty() {
        return;
    }
    let font = ui_font();
    let canvas_w = canvas.width() as i32;
    let canvas_h = canvas.height() as i32;
    let stride = canvas_w as usize * 4;
    let data = canvas.data_mut();

    // Glyph colour in linear light (constant across the run).
    let color_a = color.a as f32 / 255.0;
    let src_lin = [
        srgb_to_linear(color.r as f32 / 255.0),
        srgb_to_linear(color.g as f32 / 255.0),
        srgb_to_linear(color.b as f32 / 255.0),
    ];

    let mut pen_x = x as f32;
    for c in text.chars() {
        let (metrics, bitmap) = font.rasterize(c, size_px);
        let left = pen_x.round() as i32 + metrics.xmin;
        let top = baseline - metrics.height as i32 - metrics.ymin;

        let gw = metrics.width;
        let gh = metrics.height;
        for gy in 0..gh {
            let dy = top + gy as i32;
            if dy < 0 || dy >= canvas_h {
                continue;
            }
            for gx in 0..gw {
                let dx = left + gx as i32;
                if dx < 0 || dx >= canvas_w {
                    continue;
                }
                let alpha = bitmap[gy * gw + gx];
                if alpha == 0 {
                    continue;
                }
                // Gamma-correct "over": composite in linear light, not in raw
                // sRGB bytes (the latter darkens antialiased edges and is what
                // made small text read as muddy/blurry). The canvas is
                // premultiplied sRGB, so un-premultiply the destination, blend
                // straight in linear, then re-premultiply.
                let cov = (alpha as f32 / 255.0).powf(COVERAGE_GAMMA) * color_a;
                let idx = dy as usize * stride + dx as usize * 4;
                let dst_a = data[idx + 3] as f32 / 255.0;
                let dst_lin = if dst_a <= 0.0 {
                    [0.0f32; 3]
                } else {
                    [
                        srgb_to_linear(((data[idx] as f32 / 255.0) / dst_a).min(1.0)),
                        srgb_to_linear(((data[idx + 1] as f32 / 255.0) / dst_a).min(1.0)),
                        srgb_to_linear(((data[idx + 2] as f32 / 255.0) / dst_a).min(1.0)),
                    ]
                };
                let keep = dst_a * (1.0 - cov);
                let out_a = cov + keep;
                if out_a <= 0.0 {
                    continue;
                }
                for off in 0..3 {
                    let lin = (src_lin[off] * cov + dst_lin[off] * keep) / out_a;
                    data[idx + off] =
                        (linear_to_srgb(lin).clamp(0.0, 1.0) * out_a * 255.0).round() as u8;
                }
                data[idx + 3] = (out_a.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }

        pen_x += metrics.advance_width;
    }
}

pub fn truncate_to_fit(text: &str, max_w: i32, font_size: f32) -> String {
    if max_w <= 0 {
        return String::new();
    }
    let (w, _) = measure_text(text, font_size);
    if w <= max_w {
        return text.to_owned();
    }
    let ellipsis = "…";
    let ew = measure_text(ellipsis, font_size).0;
    let budget = max_w - ew;
    if budget <= 0 {
        return ellipsis.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let s: String = chars[..mid].iter().collect();
        if measure_text(&s, font_size).0 <= budget {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let mut result: String = chars[..lo].iter().collect();
    result.push_str(ellipsis);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::Pixmap;

    #[test]
    fn font_parses_with_line_metrics() {
        let font = ui_font();
        assert!(font.horizontal_line_metrics(14.0).is_some());
    }

    #[test]
    fn measure_text_returns_positive_for_non_empty() {
        let (w, h) = measure_text("Hello", 14.0);
        assert!(w > 0);
        assert!(h > 0);
    }

    #[test]
    fn measure_text_empty_is_zero() {
        let (w, h) = measure_text("", 14.0);
        assert_eq!(w, 0);
        assert_eq!(h, 0);
    }

    #[test]
    fn paint_text_writes_pixels() {
        let mut pixmap = Pixmap::new(64, 32).expect("pixmap");
        let mut canvas = pixmap.as_mut();
        let color = Color::rgb(0xff, 0xff, 0xff);
        paint_text(&mut canvas, "X", 8, 22, 14.0, color);
        let any_drawn = (0..32)
            .any(|y| (0..64).any(|x| pixmap.pixel(x, y).map(|p| p.alpha() > 0).unwrap_or(false)));
        assert!(any_drawn, "paint_text must touch at least one pixel");
    }

    #[test]
    fn paint_text_empty_string_is_noop() {
        let mut pixmap = Pixmap::new(32, 16).expect("pixmap");
        let before = pixmap.data().to_vec();
        let mut canvas = pixmap.as_mut();
        paint_text(&mut canvas, "", 0, 12, 14.0, Color::rgb(0xff, 0xff, 0xff));
        assert_eq!(pixmap.data(), before.as_slice());
    }
}
