use std::cell::RefCell;

use meridian_config::ThemeConfig;

use crate::{status_notifier::DbusMenuEntry, Painter, Rect, TextRenderer};

pub const SNI_MENU_WIDTH: u32 = 280;
pub const SNI_MENU_MAX_HEIGHT: u32 = 360;
const PAD_X: i32 = 12;
const TITLE_TOP: i32 = 10;
const TITLE_H: i32 = 24;
const SEP_H: i32 = 2;
pub const ROW_H: i32 = 28;
const BODY_TOP_PAD: i32 = 6;

pub fn menu_height(row_count: usize) -> u32 {
    let h = TITLE_TOP + TITLE_H + SEP_H + BODY_TOP_PAD + row_count as i32 * ROW_H + 8;
    h.clamp(56, SNI_MENU_MAX_HEIGHT as i32) as u32
}

pub fn visible_row_capacity(height: u32) -> usize {
    let body_top = TITLE_TOP + TITLE_H + SEP_H + BODY_TOP_PAD;
    let available = height as i32 - body_top - 8;
    (available.max(0) / ROW_H) as usize
}

pub fn hit_item(entries: &[DbusMenuEntry], height: u32, x: f64, y: f64) -> Option<i32> {
    if x < 0.0 || x >= SNI_MENU_WIDTH as f64 || y < 0.0 || y >= height as f64 {
        return None;
    }
    let body_top = TITLE_TOP + TITLE_H + SEP_H + BODY_TOP_PAD;
    let row = ((y as i32 - body_top) / ROW_H) as usize;
    if (y as i32) < body_top || row >= visible_row_capacity(height) {
        return None;
    }
    entries
        .iter()
        .take(visible_row_capacity(height))
        .nth(row)
        .filter(|entry| !entry.separator && entry.enabled)
        .map(|entry| entry.id)
}

pub fn draw_status_notifier_menu(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    title: &str,
    entries: &[DbusMenuEntry],
    height: u32,
) {
    let colors = &theme.colors;
    let width = SNI_MENU_WIDTH as i32;
    let height_i = height as i32;

    painter.clear(colors.surface_alt);
    painter.stroke_rect(
        Rect {
            x: 0,
            y: 0,
            w: width,
            h: height_i,
        },
        colors.border,
    );

    let title_text = if title.trim().is_empty() {
        "Tray"
    } else {
        title
    };
    painter.text_clipped(
        font,
        title_text,
        PAD_X,
        TITLE_TOP + 16,
        width - 2 * PAD_X,
        colors.accent,
    );
    let sep_y = TITLE_TOP + TITLE_H;
    painter.rect(
        Rect {
            x: 0,
            y: sep_y,
            w: width,
            h: SEP_H,
        },
        colors.accent,
    );

    let mut y = sep_y + SEP_H + BODY_TOP_PAD;
    let max_rows = visible_row_capacity(height);
    for entry in entries.iter().take(max_rows) {
        if entry.separator {
            painter.rect(
                Rect {
                    x: PAD_X,
                    y: y + ROW_H / 2,
                    w: width - 2 * PAD_X,
                    h: 1,
                },
                colors.border,
            );
            y += ROW_H;
            continue;
        }
        let indent = (entry.depth as i32).min(3) * 14;
        let label = if entry.label.trim().is_empty() {
            "Untitled"
        } else {
            entry.label.as_str()
        };
        let color = if entry.enabled {
            colors.text
        } else {
            colors.text_dim
        };
        painter.text_clipped(
            font,
            label,
            PAD_X + indent,
            y + 18,
            width - 2 * PAD_X - indent,
            color,
        );
        y += ROW_H;
    }

    if entries.len() > max_rows {
        painter.text_clipped(
            font,
            "...",
            PAD_X,
            height_i - 8,
            width - 2 * PAD_X,
            colors.text_dim,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: i32, label: &str, enabled: bool, separator: bool) -> DbusMenuEntry {
        DbusMenuEntry {
            id,
            label: label.to_string(),
            enabled,
            separator,
            depth: 0,
        }
    }

    #[test]
    fn menu_height_grows_with_rows() {
        assert!(menu_height(5) > menu_height(1));
        assert!(menu_height(50) <= SNI_MENU_MAX_HEIGHT);
    }

    #[test]
    fn hit_item_returns_enabled_row_id() {
        let entries = vec![entry(1, "Open", true, false), entry(2, "Off", false, false)];
        let h = menu_height(entries.len());
        assert_eq!(hit_item(&entries, h, 10.0, 48.0), Some(1));
        assert_eq!(hit_item(&entries, h, 10.0, 76.0), None);
    }

    #[test]
    fn hit_item_ignores_separators() {
        let entries = vec![entry(1, "Open", true, false), entry(2, "", true, true)];
        let h = menu_height(entries.len());
        assert_eq!(hit_item(&entries, h, 10.0, 76.0), None);
    }
}
