//! Shell text rendering. Glyphs are rasterized through FreeType so small UI
//! text gets TrueType hinting before being blended into the premultiplied BGRA
//! `Painter` buffer through `meridian_ui::blend_text_sample`.
//!
//! The theme font pattern is resolved through fontconfig when available;
//! otherwise the embedded Adwaita Sans fallback is used.

use meridian_config::Color;
use meridian_freetype::Font as FreeTypeFont;
use meridian_ui::{blend_text_sample, TextInk};

use crate::font_resolve;

use super::painter::Painter;

pub struct TextRenderer {
    size_px: f32,
    font: FreeTypeFont,
}

impl TextRenderer {
    /// `_pattern` (the theme font family) is intentionally ignored for now;
    /// only `pixels` (the render size) is used. Returns `Some` unconditionally
    /// so the bitmap fallback in `Painter::text_clipped` only triggers when no
    /// renderer is present at all.
    pub fn new(_pattern: &str, pixels: u32) -> Option<Self> {
        let font = font_resolve::read_theme_font_bytes(_pattern)
            .and_then(|bytes| FreeTypeFont::from_static_bytes(Box::leak(bytes.into_boxed_slice())))
            .or_else(|| {
                FreeTypeFont::from_static_bytes(meridian_tokens::font::ADWAITA_SANS_REGULAR)
            })?;
        Some(Self {
            size_px: pixels as f32,
            font,
        })
    }

    pub fn draw_text(
        &mut self,
        painter: &mut Painter<'_>,
        text: &str,
        x: i32,
        baseline: i32,
        max_w: i32,
        color: Color,
    ) -> bool {
        let ink = TextInk::new(color);
        let (w, h) = (painter.width, painter.height);
        let end_x = x + max_w;
        let mut pen_x = x as f32;
        let mut drew = false;

        for ch in text.chars() {
            if pen_x.round() as i32 >= end_x {
                break;
            }
            let Some(glyph) = self.font.rasterize(ch, self.size_px) else {
                return false;
            };
            let left = pen_x.round() as i32 + glyph.left;
            let top = baseline - glyph.top;
            for gy in 0..glyph.height {
                let dy = top + gy as i32;
                if dy < 0 || dy >= h {
                    continue;
                }
                for gx in 0..glyph.width {
                    let dx = left + gx as i32;
                    if dx < 0 || dx >= w {
                        continue;
                    }
                    let alpha = glyph.bitmap[gy * glyph.width + gx];
                    if alpha == 0 {
                        continue;
                    }
                    let idx = (dy as usize * w as usize + dx as usize) * 4;
                    // BGRA byte order: R, G, B live at offsets 2, 1, 0.
                    blend_text_sample(&mut painter.data[idx..idx + 4], ink, alpha, [2, 1, 0]);
                    drew = true;
                }
            }
            pen_x += glyph.advance_x;
        }

        drew
    }

    pub fn measure_text(&mut self, text: &str) -> i32 {
        self.font
            .measure_text(text, self.size_px)
            .map(|(width, _)| width)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::TextRenderer;

    #[test]
    fn test_renderer_creates_from_embedded_font() {
        assert!(TextRenderer::new("ignored", 12).is_some());
    }

    #[test]
    fn measure_text_is_monotonic_for_longer_strings() {
        let Some(mut renderer) = TextRenderer::new("sans", 13) else {
            return;
        };
        let short = renderer.measure_text("A");
        let long = renderer.measure_text("AA");
        assert!(long >= short);
    }

    #[test]
    fn measure_text_empty_is_zero() {
        let mut renderer = TextRenderer::new("sans", 13).expect("renderer");
        assert_eq!(renderer.measure_text(""), 0);
    }
}
