//! Shared rendering primitives for tray popups.
//!
//! Unified visual language across Audio, Network, Workspaces and Calendar:
//! rounded dark-navy card with a German title, a short cyan accent rule
//! beneath the title, key/value rows, status dots, optional volume bar,
//! and an optional cyan footer link with a chevron.

use std::cell::RefCell;

use meridian_config::{Color, ThemeConfig};

use crate::{
    ui::tokens::{GLASS_BORDER, GLASS_FOREGROUND, GLASS_FOREGROUND_DIM},
    Painter, Rect, TextRenderer,
};

pub const POPUP_WIDTH: u32 = 280;
pub const PAD_X: i32 = 16;
pub const PAD_TOP: i32 = 14;
pub const PAD_BOTTOM: i32 = 14;
pub const CARD_RADIUS: i32 = meridian_tokens::Radius::DEFAULT.xl;

pub const TITLE_BASELINE: i32 = PAD_TOP + 14;
pub const TITLE_RULE_Y: i32 = PAD_TOP + 22;
pub const TITLE_RULE_HEIGHT: i32 = 1;
/// Y position where popup body content (rows) begins.
pub const BODY_TOP: i32 = PAD_TOP + 38;

pub const ROW_HEIGHT: i32 = 26;
pub const ROW_TEXT_BASELINE_OFFSET: i32 = 17;

pub const STATUS_DOT_SIZE: i32 = 8;
pub const FOOTER_LINK_HEIGHT: i32 = 28;
pub const VOLUME_BAR_HEIGHT: i32 = 4;

/// Keep the popup canvas transparent and draw only the card outline. The
/// compositor supplies the blurred glass body behind this layer.
pub fn draw_card_body(painter: &mut Painter<'_>, _theme: &ThemeConfig) {
    painter.clear(Color::rgba(0, 0, 0, 0));
    draw_card_border(painter, GLASS_BORDER, CARD_RADIUS);
}

fn draw_card_border(painter: &mut Painter<'_>, color: Color, radius: i32) {
    let w = painter.width.max(0);
    let h = painter.height.max(0);
    if w == 0 || h == 0 {
        return;
    }

    for y in 0..h {
        for x in 0..w {
            let outer = rounded_rect_coverage(x, y, w, h, radius);
            if outer == 0 {
                continue;
            }
            let inner = rounded_rect_coverage(x - 1, y - 1, w - 2, h - 2, radius - 1);
            let coverage = outer.saturating_sub(inner);
            if coverage == 0 {
                continue;
            }

            let alpha = ((u16::from(color.a) * u16::from(coverage)) / 255) as u8;
            let offset = ((y * painter.width + x) * 4) as usize;
            painter.data[offset] = ((u16::from(color.b) * u16::from(alpha)) / 255) as u8;
            painter.data[offset + 1] = ((u16::from(color.g) * u16::from(alpha)) / 255) as u8;
            painter.data[offset + 2] = ((u16::from(color.r) * u16::from(alpha)) / 255) as u8;
            painter.data[offset + 3] = alpha;
        }
    }
}

fn rounded_rect_coverage(x: i32, y: i32, w: i32, h: i32, radius: i32) -> u8 {
    if w <= 0 || h <= 0 || x < 0 || y < 0 || x >= w || y >= h {
        return 0;
    }
    let radius = radius.max(0).min(w / 2).min(h / 2);
    if radius == 0 {
        return 255;
    }

    let mut inside = 0u8;
    for sy in [0.25_f32, 0.75] {
        for sx in [0.25_f32, 0.75] {
            if rounded_rect_sample_inside(x as f32 + sx, y as f32 + sy, w, h, radius) {
                inside += 1;
            }
        }
    }

    match inside {
        0 => 0,
        4 => 255,
        _ => ((u16::from(inside) * 255) / 4) as u8,
    }
}

fn rounded_rect_sample_inside(x: f32, y: f32, w: i32, h: i32, radius: i32) -> bool {
    let w = w as f32;
    let h = h as f32;
    let r = radius as f32;
    if x < 0.0 || y < 0.0 || x >= w || y >= h {
        return false;
    }
    if x >= r && x < w - r {
        return true;
    }
    if y >= r && y < h - r {
        return true;
    }

    let cx = if x < r { r } else { w - r };
    let cy = if y < r { r } else { h - r };
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= r * r
}

/// Soft drop shadow params shared by every tray popup. Same look as the
/// panel island (`panel_view.rs`) so the visual language stays uniform.
pub const POPUP_SHADOW_BLUR: f32 = meridian_tokens::Elevation::POPUP.blur;
pub const POPUP_SHADOW_ALPHA: f32 = meridian_tokens::Elevation::POPUP.alpha;
pub const POPUP_SHADOW_OFFSET_Y: i32 = meridian_tokens::Elevation::POPUP.offset_y;

/// Draw the shared glass popup border into a BGRA buffer at `rect`.
///
/// This is the same coverage code used by [`draw_card_body`], exposed for
/// composite popup surfaces that contain more than one glass panel.
pub fn draw_glass_card_border_in_rect(buf: &mut [u8], buf_w: i32, buf_h: i32, rect: Rect) {
    draw_card_border_in_rect(buf, buf_w, buf_h, rect, GLASS_BORDER, CARD_RADIUS);
}

fn draw_card_border_in_rect(
    buf: &mut [u8],
    buf_w: i32,
    buf_h: i32,
    rect: Rect,
    color: Color,
    radius: i32,
) {
    if buf_w <= 0 || buf_h <= 0 || rect.w <= 0 || rect.h <= 0 {
        return;
    }

    for y in 0..rect.h {
        let py = rect.y + y;
        if py < 0 || py >= buf_h {
            continue;
        }
        for x in 0..rect.w {
            let px = rect.x + x;
            if px < 0 || px >= buf_w {
                continue;
            }

            let outer = rounded_rect_coverage(x, y, rect.w, rect.h, radius);
            if outer == 0 {
                continue;
            }
            let inner = rounded_rect_coverage(x - 1, y - 1, rect.w - 2, rect.h - 2, radius - 1);
            let coverage = outer.saturating_sub(inner);
            if coverage == 0 {
                continue;
            }

            let alpha = ((u16::from(color.a) * u16::from(coverage)) / 255) as u8;
            let offset = ((py * buf_w + px) * 4) as usize;
            if offset + 4 > buf.len() {
                continue;
            }
            buf[offset] = ((u16::from(color.b) * u16::from(alpha)) / 255) as u8;
            buf[offset + 1] = ((u16::from(color.g) * u16::from(alpha)) / 255) as u8;
            buf[offset + 2] = ((u16::from(color.r) * u16::from(alpha)) / 255) as u8;
            buf[offset + 3] = alpha;
        }
    }
}

/// Composite an already-rendered `card_buf` (card_w * card_h * 4 BGRA bytes)
/// onto a layer-shell `surface` buffer plus a soft drop shadow. The surface
/// buffer is sized `card + 2*POPUP_SHADOW_PAD`; the card lands at
/// `(PAD, PAD)` with the shadow filling the surrounding margin.
///
/// Caller renders the card normally (Painter, round_buffer_corners) into a
/// temp Vec and passes it here. Performance: one O(N) composite per draw,
/// no per-pixel work on idle ticks.
pub fn paint_card_with_shadow(
    surface: &mut [u8],
    surface_w: u32,
    surface_h: u32,
    card_w: u32,
    card_h: u32,
    card_buf: &[u8],
) {
    let panel = Rect {
        x: 0,
        y: 0,
        w: card_w as i32,
        h: card_h as i32,
    };
    paint_card_panels_with_shadow(
        surface,
        surface_w,
        surface_h,
        card_w,
        card_h,
        card_buf,
        &[panel],
    );
}

/// Composite a card buffer and draw the shared popup shadow for each glass
/// panel inside it. Panel coordinates are local to `card_buf`.
pub fn paint_card_panels_with_shadow(
    surface: &mut [u8],
    surface_w: u32,
    surface_h: u32,
    card_w: u32,
    card_h: u32,
    card_buf: &[u8],
    panels: &[Rect],
) {
    surface.fill(0);
    let pad = crate::POPUP_SHADOW_PAD;
    for panel in panels {
        crate::soft_shadow::draw_soft_shadow(
            surface,
            surface_w as i32,
            surface_h as i32,
            pad + panel.x,
            pad + panel.y,
            panel.w,
            panel.h,
            CARD_RADIUS as f32,
            POPUP_SHADOW_BLUR,
            POPUP_SHADOW_ALPHA,
            POPUP_SHADOW_OFFSET_Y,
            true,
        );
    }
    crate::soft_shadow::composite_card_onto_surface(
        surface,
        surface_w as usize,
        surface_h as usize,
        card_buf,
        card_w as usize,
        card_h as usize,
        pad as usize,
        pad as usize,
    );
}

/// Draw the title text and the short cyan accent rule beneath it. The rule
/// sits under the title word (not full-width) for a calmer look.
pub fn draw_card_title(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    _theme: &ThemeConfig,
    title: &str,
) {
    let width = POPUP_WIDTH as i32;
    painter.text_clipped(
        font,
        title,
        PAD_X,
        TITLE_BASELINE,
        width - 2 * PAD_X,
        GLASS_FOREGROUND,
    );
    let title_w = font
        .borrow_mut()
        .as_mut()
        .map(|r| r.measure_text(title))
        .unwrap_or(80);
    let rule_w = (title_w + 10).min(width - 2 * PAD_X);
    painter.rect(
        Rect {
            x: PAD_X,
            y: TITLE_RULE_Y,
            w: rule_w,
            h: TITLE_RULE_HEIGHT,
        },
        GLASS_FOREGROUND,
    );
}

/// Label on the left (dim), value text on the right (bright, right-aligned).
pub fn draw_kv_row(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    _theme: &ThemeConfig,
    label: &str,
    value: &str,
    row_y: i32,
) {
    let width = POPUP_WIDTH as i32;
    let baseline = row_y + ROW_TEXT_BASELINE_OFFSET;
    painter.text_clipped(
        font,
        label,
        PAD_X,
        baseline,
        width / 2 - PAD_X,
        GLASS_FOREGROUND_DIM,
    );
    let value_w = font
        .borrow_mut()
        .as_mut()
        .map(|r| r.measure_text(value))
        .unwrap_or(value.chars().count() as i32 * 8);
    let value_x = width - PAD_X - value_w;
    painter.text_clipped(font, value, value_x, baseline, value_w, GLASS_FOREGROUND);
}

/// Status row: label on the left, colored dot + status text on the right.
pub fn draw_status_row(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    _theme: &ThemeConfig,
    label: &str,
    status: &str,
    dot_color: Color,
    row_y: i32,
) {
    let width = POPUP_WIDTH as i32;
    let baseline = row_y + ROW_TEXT_BASELINE_OFFSET;
    painter.text_clipped(
        font,
        label,
        PAD_X,
        baseline,
        width / 2 - PAD_X,
        GLASS_FOREGROUND_DIM,
    );
    let status_w = font
        .borrow_mut()
        .as_mut()
        .map(|r| r.measure_text(status))
        .unwrap_or(status.chars().count() as i32 * 8);
    let status_x = width - PAD_X - status_w;
    painter.text_clipped(font, status, status_x, baseline, status_w, GLASS_FOREGROUND);
    let dot_x = status_x - STATUS_DOT_SIZE - 8;
    let dot_y = row_y + (ROW_HEIGHT - STATUS_DOT_SIZE) / 2;
    painter.roundish_rect_with_radius(
        Rect {
            x: dot_x,
            y: dot_y,
            w: STATUS_DOT_SIZE,
            h: STATUS_DOT_SIZE,
        },
        dot_color,
        STATUS_DOT_SIZE,
    );
}

/// Volume row: label, slim cyan bar, percent text. `muted=true` greys out the
/// fill and appends "stumm".
pub fn draw_volume_row(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    label: &str,
    percent: Option<u32>,
    muted: bool,
    row_y: i32,
) {
    let width = POPUP_WIDTH as i32;
    let value_text = match (percent, muted) {
        (Some(v), true) => format!("{v}% stumm"),
        (Some(v), false) => format!("{v}%"),
        (None, _) => "\u{2014}".to_string(),
    };
    let baseline = row_y + ROW_TEXT_BASELINE_OFFSET;

    let label_w = font
        .borrow_mut()
        .as_mut()
        .map(|r| r.measure_text(label))
        .unwrap_or(70);
    painter.text_clipped(font, label, PAD_X, baseline, label_w, GLASS_FOREGROUND_DIM);

    let value_w = font
        .borrow_mut()
        .as_mut()
        .map(|r| r.measure_text(&value_text))
        .unwrap_or(40);
    let value_x = width - PAD_X - value_w;

    let bar_left = PAD_X + label_w + 12;
    let bar_right = value_x - 12;
    let bar_w = (bar_right - bar_left).max(0);
    if bar_w >= 20 {
        let bar_y = row_y + (ROW_HEIGHT - VOLUME_BAR_HEIGHT) / 2;
        painter.roundish_rect_with_radius(
            Rect {
                x: bar_left,
                y: bar_y,
                w: bar_w,
                h: VOLUME_BAR_HEIGHT,
            },
            GLASS_BORDER,
            VOLUME_BAR_HEIGHT,
        );
        if let Some(pct) = percent {
            let fill_w = (bar_w * pct.min(100) as i32) / 100;
            if fill_w > 0 {
                let fill_color = if muted {
                    GLASS_FOREGROUND_DIM
                } else {
                    theme.colors.accent
                };
                painter.roundish_rect_with_radius(
                    Rect {
                        x: bar_left,
                        y: bar_y,
                        w: fill_w,
                        h: VOLUME_BAR_HEIGHT,
                    },
                    fill_color,
                    VOLUME_BAR_HEIGHT,
                );
            }
        }
    }

    painter.text_clipped(
        font,
        &value_text,
        value_x,
        baseline,
        value_w,
        GLASS_FOREGROUND,
    );
}

/// Right-aligned cyan link with a chevron at the bottom of the popup.
/// Returns the click rectangle so the pointer handler can hit-test it.
pub fn draw_footer_link(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    _theme: &ThemeConfig,
    height: i32,
    label: &str,
) -> Rect {
    let width = POPUP_WIDTH as i32;
    let text = format!("{label}  \u{203A}");
    let measured = font
        .borrow_mut()
        .as_mut()
        .map(|r| r.measure_text(&text))
        .unwrap_or(text.chars().count() as i32 * 8);
    let y_top = height - PAD_BOTTOM - FOOTER_LINK_HEIGHT;
    let x = width - PAD_X - measured;
    let baseline = y_top + ROW_TEXT_BASELINE_OFFSET;
    painter.text_clipped(font, &text, x, baseline, measured, GLASS_FOREGROUND);
    Rect {
        x: x - 10,
        y: y_top,
        w: measured + 20,
        h: FOOTER_LINK_HEIGHT,
    }
}
