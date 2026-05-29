//! Shared rendering primitives for tray popups.
//!
//! Unified visual language across Audio, Network, Workspaces and Calendar:
//! rounded dark-navy card with a German title, a short cyan accent rule
//! beneath the title, key/value rows, status dots, optional volume bar,
//! and an optional cyan footer link with a chevron.

use std::cell::RefCell;

use meridian_config::{Color, ThemeConfig};

use crate::{Painter, Rect, TextRenderer};

pub const POPUP_WIDTH: u32 = 280;
pub const PAD_X: i32 = 16;
pub const PAD_TOP: i32 = 14;
pub const PAD_BOTTOM: i32 = 14;
pub const CARD_RADIUS: i32 = 14;

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

/// Fill the popup canvas with the card body color. Rounded corners are
/// applied later by the surface-level `round_buffer_corners` helper so the
/// corner pixels become transparent and the wallpaper shows through.
pub fn draw_card_body(painter: &mut Painter<'_>, theme: &ThemeConfig) {
    painter.clear(theme.colors.surface_alt);
}

/// Draw the title text and the short cyan accent rule beneath it. The rule
/// sits under the title word (not full-width) for a calmer look.
pub fn draw_card_title(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    title: &str,
) {
    let width = POPUP_WIDTH as i32;
    painter.text_clipped(
        font,
        title,
        PAD_X,
        TITLE_BASELINE,
        width - 2 * PAD_X,
        theme.colors.text,
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
        theme.colors.accent,
    );
}

/// Label on the left (dim), value text on the right (bright, right-aligned).
pub fn draw_kv_row(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
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
        theme.colors.text_dim,
    );
    let value_w = font
        .borrow_mut()
        .as_mut()
        .map(|r| r.measure_text(value))
        .unwrap_or(value.chars().count() as i32 * 8);
    let value_x = width - PAD_X - value_w;
    painter.text_clipped(font, value, value_x, baseline, value_w, theme.colors.text);
}

/// Status row: label on the left, colored dot + status text on the right.
pub fn draw_status_row(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
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
        theme.colors.text_dim,
    );
    let status_w = font
        .borrow_mut()
        .as_mut()
        .map(|r| r.measure_text(status))
        .unwrap_or(status.chars().count() as i32 * 8);
    let status_x = width - PAD_X - status_w;
    painter.text_clipped(
        font,
        status,
        status_x,
        baseline,
        status_w,
        theme.colors.text,
    );
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
    painter.text_clipped(font, label, PAD_X, baseline, label_w, theme.colors.text_dim);

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
            theme.colors.border,
            VOLUME_BAR_HEIGHT,
        );
        if let Some(pct) = percent {
            let fill_w = (bar_w * pct.min(100) as i32) / 100;
            if fill_w > 0 {
                let fill_color = if muted {
                    theme.colors.text_dim
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
        theme.colors.text,
    );
}

/// Right-aligned cyan link with a chevron at the bottom of the popup.
/// Returns the click rectangle so the pointer handler can hit-test it.
pub fn draw_footer_link(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
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
    painter.text_clipped(font, &text, x, baseline, measured, theme.colors.accent);
    Rect {
        x: x - 10,
        y: y_top,
        w: measured + 20,
        h: FOOTER_LINK_HEIGHT,
    }
}
