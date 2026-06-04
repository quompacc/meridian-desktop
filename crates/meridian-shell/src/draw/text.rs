//! Shell text rendering. The glyph source is fontdue, via the shared
//! `meridian_ui::ui_font()` — the same engine the widget views use — so the
//! whole shell rasterises through one library. Each glyph coverage sample is
//! blended into the premultiplied BGRA `Painter` buffer through the shared
//! `meridian_ui::blend_text_sample`, so popups and widgets use the identical
//! gamma / stem-darkening blend.
//!
//! `TextRenderer` is now a thin handle carrying the pixel size. The font
//! family in the theme string is not yet honoured (the embedded Adwaita Sans
//! is always used); that lands with the theme-font work. Keeping the type and
//! its `Option` plumbing avoids churning the popup call sites for this step.

use meridian_config::Color;
use meridian_ui::{blend_text_sample, ui_font, TextInk};

use super::painter::Painter;

pub struct TextRenderer {
    size_px: f32,
}

impl TextRenderer {
    /// `_pattern` (the theme font family) is intentionally ignored for now;
    /// only `pixels` (the render size) is used. Returns `Some` unconditionally
    /// so the bitmap fallback in `Painter::text_clipped` only triggers when no
    /// renderer is present at all.
    pub fn new(_pattern: &str, pixels: u32) -> Option<Self> {
        Some(Self {
            size_px: pixels as f32,
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
        let font = ui_font();
        let ink = TextInk::new(color);
        let (w, h) = (painter.width, painter.height);
        let end_x = x + max_w;
        let mut pen_x = x as f32;
        let mut drew = false;

        for ch in text.chars() {
            if pen_x.round() as i32 >= end_x {
                break;
            }
            let (metrics, bitmap) = font.rasterize(ch, self.size_px);
            let left = pen_x.round() as i32 + metrics.xmin;
            // fontdue's ymin is the offset of the glyph bottom from the
            // baseline; the top edge sits height+ymin above it.
            let top = baseline - metrics.height as i32 - metrics.ymin;
            for gy in 0..metrics.height {
                let dy = top + gy as i32;
                if dy < 0 || dy >= h {
                    continue;
                }
                for gx in 0..metrics.width {
                    let dx = left + gx as i32;
                    if dx < 0 || dx >= w {
                        continue;
                    }
                    let alpha = bitmap[gy * metrics.width + gx];
                    if alpha == 0 {
                        continue;
                    }
                    let idx = (dy as usize * w as usize + dx as usize) * 4;
                    // BGRA byte order: R, G, B live at offsets 2, 1, 0.
                    blend_text_sample(&mut painter.data[idx..idx + 4], ink, alpha, [2, 1, 0]);
                    drew = true;
                }
            }
            pen_x += metrics.advance_width;
        }

        drew
    }

    pub fn measure_text(&mut self, text: &str) -> i32 {
        let font = ui_font();
        let mut width = 0.0f32;
        for ch in text.chars() {
            width += font.metrics(ch, self.size_px).advance_width;
        }
        width.round() as i32
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
