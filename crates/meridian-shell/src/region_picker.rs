//! Fullscreen overlay that lets the user pick a screenshot region with a
//! drag rectangle. Built on top of `wlr-layer-shell` (an `Overlay` layer
//! anchored on all four edges so it stretches to fill the output).
//!
//! UX (Spectacle-style two-step):
//! - Mouse press starts a new drag (drops any previously pending selection).
//! - Mouse motion updates the live rectangle.
//! - Mouse release freezes the rectangle as `pending`; the user can still
//!   start a new drag to redo it.
//! - Enter confirms the pending rectangle.
//! - Esc cancels the picker entirely.
//!
//! The overlay paints the framebuffer with semi-transparent black outside the
//! selection so the rest of the desktop is dimmed but visible; the selection
//! itself stays fully transparent (alpha = 0) so the underlying pixels show
//! through unaltered, with a 2-pixel solid white border on top.

use meridian_config::ThemeConfig;
use meridian_ipc::ScreenshotRegion;
use meridian_ui::effect::paint_text;
use tiny_skia::Pixmap;

use crate::ui::tokens::palette_from_config;

/// Width/height of the "ink" area for the helper status string at the top
/// center of the overlay. Drawn through a small Pixmap and blitted on top of
/// the dim overlay.
const STATUS_W: i32 = 700;
const STATUS_H: i32 = 36;
const STATUS_Y: i32 = 28;

/// Semi-transparent black used to dim the desktop outside the selection.
/// 160/255 ≈ 63% opacity — strong enough that the picker is unmistakable
/// as a modal overlay (the previous 96/255 was easy to miss on a
/// uniformly dark desktop background).
const DIM_ALPHA: u8 = 160;

/// Border color + thickness for the selection rectangle. 3 px makes the
/// selection rectangle obvious against any desktop background.
const BORDER_THICKNESS: i32 = 3;

/// Local axis-aligned rectangle in overlay (= output) coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RegionRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl RegionRect {
    pub(crate) fn to_screenshot_region(self) -> ScreenshotRegion {
        ScreenshotRegion {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
}

/// Compute the axis-aligned bounding rect from a drag's anchor and current
/// pointer position. Returns `None` if the rectangle is degenerate (zero
/// width or height) — that case is treated as "no selection".
pub(crate) fn rect_from_drag(
    start: (i32, i32),
    current: (i32, i32),
    canvas_w: u32,
    canvas_h: u32,
) -> Option<RegionRect> {
    let x0 = start.0.min(current.0).max(0);
    let y0 = start.1.min(current.1).max(0);
    let x1 = start.0.max(current.0).min(canvas_w as i32);
    let y1 = start.1.max(current.1).min(canvas_h as i32);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some(RegionRect {
        x: x0,
        y: y0,
        width: (x1 - x0) as u32,
        height: (y1 - y0) as u32,
    })
}

/// Paint the overlay onto a BGRA canvas of `canvas_w` x `canvas_h`.
///
/// `selection` is the rectangle to expose (None = nothing selected yet:
/// everything is dimmed). The overlay also blits a small status text at the
/// top center; if `theme_config` is unavailable the text is omitted.
pub(crate) fn draw_region_picker_overlay(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    selection: Option<RegionRect>,
    theme_config: &ThemeConfig,
) {
    fill_dim(canvas, canvas_w, canvas_h);
    if let Some(rect) = selection {
        clear_rect_interior(canvas, canvas_w, canvas_h, rect);
        draw_rect_border(canvas, canvas_w, canvas_h, rect, BORDER_THICKNESS);
    }
    paint_status(canvas, canvas_w, canvas_h, theme_config);
}

fn fill_dim(canvas: &mut [u8], w: u32, h: u32) {
    let total = (w as usize) * (h as usize) * 4;
    if canvas.len() < total {
        return;
    }
    // BGRA, premultiplied: dim_alpha * (B=G=R=0) = 0, alpha = DIM_ALPHA.
    for px in canvas[..total].chunks_exact_mut(4) {
        px[0] = 0;
        px[1] = 0;
        px[2] = 0;
        px[3] = DIM_ALPHA;
    }
}

fn clear_rect_interior(canvas: &mut [u8], cw: u32, ch: u32, rect: RegionRect) {
    let x0 = rect.x.clamp(0, cw as i32);
    let y0 = rect.y.clamp(0, ch as i32);
    let x1 = (rect.x + rect.width as i32).clamp(0, cw as i32);
    let y1 = (rect.y + rect.height as i32).clamp(0, ch as i32);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let stride = (cw as usize) * 4;
    for y in y0..y1 {
        let row = (y as usize) * stride;
        let start = row + (x0 as usize) * 4;
        let end = row + (x1 as usize) * 4;
        if end > canvas.len() {
            break;
        }
        // Fully transparent — the desktop pixels behind the overlay show
        // through unaltered; this is the "see-through" hole.
        for px in canvas[start..end].chunks_exact_mut(4) {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
            px[3] = 0;
        }
    }
}

fn draw_rect_border(canvas: &mut [u8], cw: u32, ch: u32, rect: RegionRect, thickness: i32) {
    let x0 = rect.x.clamp(0, cw as i32);
    let y0 = rect.y.clamp(0, ch as i32);
    let x1 = (rect.x + rect.width as i32).clamp(0, cw as i32);
    let y1 = (rect.y + rect.height as i32).clamp(0, ch as i32);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let stride = (cw as usize) * 4;
    let put = |canvas: &mut [u8], x: i32, y: i32| {
        if x < 0 || y < 0 || x >= cw as i32 || y >= ch as i32 {
            return;
        }
        let idx = (y as usize) * stride + (x as usize) * 4;
        if idx + 3 < canvas.len() {
            // Solid white, opaque.
            canvas[idx] = 0xFF;
            canvas[idx + 1] = 0xFF;
            canvas[idx + 2] = 0xFF;
            canvas[idx + 3] = 0xFF;
        }
    };
    for t in 0..thickness {
        // Top + bottom edges.
        for x in x0..x1 {
            put(canvas, x, y0 + t);
            put(canvas, x, y1 - 1 - t);
        }
        // Left + right edges.
        for y in y0..y1 {
            put(canvas, x0 + t, y);
            put(canvas, x1 - 1 - t, y);
        }
    }
}

fn paint_status(canvas: &mut [u8], cw: u32, ch: u32, theme_config: &ThemeConfig) {
    use meridian_ui::effect::{paint_fill, rounded_rect_path};
    use meridian_ui::paint::Rect;
    let w = STATUS_W as u32;
    let h = STATUS_H as u32;
    let Some(mut pm) = Pixmap::new(w, h) else {
        return;
    };
    let pal = palette_from_config(theme_config);
    // Filled rounded card so the helper text reads against the dim overlay
    // regardless of what's behind it.
    let card = Rect {
        x: 0,
        y: 0,
        width: STATUS_W,
        height: STATUS_H,
    };
    if let Some(path) = rounded_rect_path(card, 10) {
        paint_fill(&mut pm.as_mut(), &path, pal.surface_alt);
    }
    paint_text(
        &mut pm.as_mut(),
        "Bereich auswählen — Drag = ziehen — Enter = bestätigen — Esc = abbrechen",
        12,
        24,
        13.0,
        pal.text,
    );
    let pw = pm.width() as i32;
    let _ph = pm.height() as i32;
    let dst_x = ((cw as i32 - pw) / 2).max(0);
    let dst_y = STATUS_Y;
    blit_text(canvas, cw as i32, ch as i32, &pm, dst_x, dst_y);
}

/// Alpha-blend a premultiplied Pixmap onto the BGRA canvas at `(dst_x, dst_y)`.
/// Unlike the consent modal's blit this preserves the destination alpha (the
/// overlay needs to stay translucent / transparent in places).
fn blit_text(canvas: &mut [u8], cw: i32, ch: i32, pm: &Pixmap, dst_x: i32, dst_y: i32) {
    let pw = pm.width() as i32;
    let ph = pm.height() as i32;
    let src = pm.pixels();
    for my in 0..ph {
        let dy = dst_y + my;
        if dy < 0 || dy >= ch {
            continue;
        }
        for mx in 0..pw {
            let dx = dst_x + mx;
            if dx < 0 || dx >= cw {
                continue;
            }
            let pi = (my * pw + mx) as usize;
            if pi >= src.len() {
                continue;
            }
            let px = src[pi];
            let a = px.alpha();
            if a == 0 {
                continue;
            }
            let ci = ((dy * cw + dx) * 4) as usize;
            if ci + 3 >= canvas.len() {
                continue;
            }
            let rp = px.red() as u32;
            let gp = px.green() as u32;
            let bp = px.blue() as u32;
            let inv = 255 - a as u32;
            let db = canvas[ci] as u32;
            let dg = canvas[ci + 1] as u32;
            let dr = canvas[ci + 2] as u32;
            let da = canvas[ci + 3] as u32;
            canvas[ci] = (bp + db * inv / 255).min(255) as u8;
            canvas[ci + 1] = (gp + dg * inv / 255).min(255) as u8;
            canvas[ci + 2] = (rp + dr * inv / 255).min(255) as u8;
            canvas[ci + 3] = (a as u32 + da * inv / 255).min(255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_from_drag_normalises_direction() {
        // Drag right-down.
        let r = rect_from_drag((10, 20), (110, 220), 1280, 800).unwrap();
        assert_eq!(
            r,
            RegionRect {
                x: 10,
                y: 20,
                width: 100,
                height: 200
            }
        );
        // Drag left-up — same rect.
        let r = rect_from_drag((110, 220), (10, 20), 1280, 800).unwrap();
        assert_eq!(
            r,
            RegionRect {
                x: 10,
                y: 20,
                width: 100,
                height: 200
            }
        );
    }

    #[test]
    fn rect_from_drag_clamps_to_canvas() {
        let r = rect_from_drag((-50, -50), (200, 200), 1280, 800).unwrap();
        assert_eq!(
            r,
            RegionRect {
                x: 0,
                y: 0,
                width: 200,
                height: 200
            }
        );
        let r = rect_from_drag((1200, 700), (1500, 900), 1280, 800).unwrap();
        assert_eq!(
            r,
            RegionRect {
                x: 1200,
                y: 700,
                width: 80,
                height: 100
            }
        );
    }

    #[test]
    fn rect_from_drag_returns_none_for_zero_area() {
        // Same point.
        assert!(rect_from_drag((50, 50), (50, 50), 1280, 800).is_none());
        // Same x, different y.
        assert!(rect_from_drag((50, 10), (50, 200), 1280, 800).is_none());
        // Outside canvas entirely.
        assert!(rect_from_drag((2000, 900), (3000, 1000), 1280, 800).is_none());
    }

    #[test]
    fn to_screenshot_region_preserves_geometry() {
        let r = RegionRect {
            x: 12,
            y: 34,
            width: 56,
            height: 78,
        };
        let sr = r.to_screenshot_region();
        assert_eq!(sr.x, 12);
        assert_eq!(sr.y, 34);
        assert_eq!(sr.width, 56);
        assert_eq!(sr.height, 78);
    }
}
