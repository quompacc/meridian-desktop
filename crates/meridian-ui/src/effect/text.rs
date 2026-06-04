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

use std::sync::{OnceLock, RwLock};

use fontdue::{Font, FontSettings};
use tiny_skia::PixmapMut;

use crate::style::Color;

const UI_FONT_DATA: &[u8] = meridian_tokens::font::ADWAITA_SANS_REGULAR;

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

/// Pre-tonemapped text colour, computed once per run and reused for each glyph
/// pixel by [`blend_text_sample`].
#[derive(Clone, Copy)]
pub struct TextInk {
    src_lin: [f32; 3],
    alpha: f32,
}

impl TextInk {
    pub fn new(color: Color) -> Self {
        Self {
            src_lin: [
                srgb_to_linear(color.r as f32 / 255.0),
                srgb_to_linear(color.g as f32 / 255.0),
                srgb_to_linear(color.b as f32 / 255.0),
            ],
            alpha: color.a as f32 / 255.0,
        }
    }
}

/// Blend one glyph coverage sample into a premultiplied 4-byte pixel, using the
/// UI text gamma + stem-darkening and a gamma-correct linear "over". `rgb` gives
/// the byte positions of the R, G, B channels (alpha is byte 3): `[0, 1, 2]` for
/// RGBA, `[2, 1, 0]` for BGRA. The single home of the text blend, shared by the
/// fontdue `paint_text` (tiny-skia RGBA) and the shell `Painter` glyph path
/// (premultiplied BGRA).
pub fn blend_text_sample(px: &mut [u8], ink: TextInk, coverage: u8, rgb: [usize; 3]) {
    if coverage == 0 {
        return;
    }
    let cov = (coverage as f32 / 255.0).powf(COVERAGE_GAMMA) * ink.alpha;
    let dst_a = px[3] as f32 / 255.0;
    let dst_lin = if dst_a <= 0.0 {
        [0.0f32; 3]
    } else {
        [
            srgb_to_linear(((px[rgb[0]] as f32 / 255.0) / dst_a).min(1.0)),
            srgb_to_linear(((px[rgb[1]] as f32 / 255.0) / dst_a).min(1.0)),
            srgb_to_linear(((px[rgb[2]] as f32 / 255.0) / dst_a).min(1.0)),
        ]
    };
    let keep = dst_a * (1.0 - cov);
    let out_a = cov + keep;
    if out_a <= 0.0 {
        return;
    }
    for i in 0..3 {
        let lin = (ink.src_lin[i] * cov + dst_lin[i] * keep) / out_a;
        px[rgb[i]] = (linear_to_srgb(lin).clamp(0.0, 1.0) * out_a * 255.0).round() as u8;
    }
    px[3] = (out_a.clamp(0.0, 1.0) * 255.0).round() as u8;
}

static UI_FONT_DEFAULT: OnceLock<Font> = OnceLock::new();
static UI_FONT_OVERRIDE: RwLock<Option<&'static Font>> = RwLock::new(None);

fn embedded_ui_font() -> &'static Font {
    UI_FONT_DEFAULT.get_or_init(|| {
        Font::from_bytes(UI_FONT_DATA, FontSettings::default())
            .expect("embedded Adwaita Sans Regular parses")
    })
}

/// The active UI font: the theme-resolved override if one is set, else the
/// embedded Adwaita Sans. Returns `&'static` so callers can hold it across a
/// draw without locking.
pub fn ui_font() -> &'static Font {
    if let Some(font) = *UI_FONT_OVERRIDE.read().expect("ui font override poisoned") {
        return font;
    }
    embedded_ui_font()
}

/// Set the active UI font from raw TTF/OTF bytes (a fontconfig-resolved
/// `theme.fonts.ui` family). Returns false (keeping the current font) if the
/// bytes do not parse. The font is leaked for a `'static` reference; intended
/// for the rare theme-font change, not per-frame use.
pub fn set_ui_font(bytes: &[u8]) -> bool {
    match Font::from_bytes(bytes, FontSettings::default()) {
        Ok(font) => {
            *UI_FONT_OVERRIDE.write().expect("ui font override poisoned") =
                Some(Box::leak(Box::new(font)));
            true
        }
        Err(_) => false,
    }
}

/// Drop any override, reverting to the embedded Adwaita Sans.
pub fn clear_ui_font() {
    *UI_FONT_OVERRIDE.write().expect("ui font override poisoned") = None;
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

/// (ascent, descent) of the active UI font at `size_px`, in pixels. Descent is
/// negative (below the baseline), per the usual font-metrics convention. Used
/// by client-drawn surfaces (lock screen, polkit) that lay text out by hand.
pub fn ui_line_metrics(size_px: f32) -> (f32, f32) {
    match ui_font().horizontal_line_metrics(size_px) {
        Some(m) => (m.ascent, m.descent),
        None => (size_px * 0.8, -size_px * 0.2),
    }
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

    let ink = TextInk::new(color);

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
                let idx = dy as usize * stride + dx as usize * 4;
                blend_text_sample(&mut data[idx..idx + 4], ink, alpha, [0, 1, 2]);
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
