// Phase A3.1+A3.2: paint the settings overlay — sidebar on the left
// (category list), content area on the right (renders the selected
// category's view).
//
// v1 categories: Theme, Cursor, Wallpaper, Pinned Apps. Only the layout
// + sidebar selection is implemented here; the per-category content
// rendering (theme picker etc.) is A3.3 and follow-up sessions.

use std::cell::RefCell;

use meridian_config::ThemeConfig;

use crate::{Painter, Rect, TextRenderer};

const SIDEBAR_WIDTH: i32 = 180;
const SIDEBAR_ITEM_HEIGHT: i32 = 44;
const SIDEBAR_PAD_X: i32 = 16;
const SIDEBAR_TOP_PAD: i32 = 20;
const CONTENT_PAD_X: i32 = 32;
const CONTENT_TOP_PAD: i32 = 28;
const TITLE_HEIGHT: i32 = 28;
const ACCENT_STRIP_H: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    Theme,
    Cursor,
    Wallpaper,
    PinnedApps,
}

impl SettingsCategory {
    pub const ALL: &'static [SettingsCategory] = &[
        SettingsCategory::Theme,
        SettingsCategory::Cursor,
        SettingsCategory::Wallpaper,
        SettingsCategory::PinnedApps,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            SettingsCategory::Theme => "Theme",
            SettingsCategory::Cursor => "Cursor",
            SettingsCategory::Wallpaper => "Wallpaper",
            SettingsCategory::PinnedApps => "Pinned Apps",
        }
    }
}

impl Default for SettingsCategory {
    fn default() -> Self {
        SettingsCategory::Theme
    }
}

/// Hit-test the sidebar at the given local coordinates inside the
/// settings surface. Returns the category whose row was clicked, or
/// `None` for clicks outside the sidebar.
pub fn sidebar_hit_test(x: f64, y: f64) -> Option<SettingsCategory> {
    if x < 0.0 || x > SIDEBAR_WIDTH as f64 {
        return None;
    }
    let y_rel = y as i32 - SIDEBAR_TOP_PAD;
    if y_rel < 0 {
        return None;
    }
    let row = y_rel / SIDEBAR_ITEM_HEIGHT;
    if (row as usize) < SettingsCategory::ALL.len() {
        Some(SettingsCategory::ALL[row as usize])
    } else {
        None
    }
}

pub fn draw_settings(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    width: u32,
    height: u32,
    selected: SettingsCategory,
) {
    let colors = &theme.colors;
    let width = width as i32;
    let height = height as i32;

    // Backdrop + border (same metro-card pattern as the other popups).
    painter.clear(colors.surface_alt);
    painter.stroke_rect(
        Rect {
            x: 0,
            y: 0,
            w: width,
            h: height,
        },
        colors.border,
    );

    // Sidebar background — slightly darker so the active app feels
    // anchored on the left.
    painter.rect(
        Rect {
            x: 0,
            y: 0,
            w: SIDEBAR_WIDTH,
            h: height,
        },
        colors.surface,
    );
    // 1px vertical divider between sidebar and content.
    painter.rect(
        Rect {
            x: SIDEBAR_WIDTH,
            y: 0,
            w: 1,
            h: height,
        },
        colors.border,
    );

    // Sidebar items.
    for (i, cat) in SettingsCategory::ALL.iter().enumerate() {
        let y = SIDEBAR_TOP_PAD + (i as i32) * SIDEBAR_ITEM_HEIGHT;
        if *cat == selected {
            // Accent strip on the left edge of the selected row.
            painter.rect(
                Rect {
                    x: 0,
                    y,
                    w: 3,
                    h: SIDEBAR_ITEM_HEIGHT,
                },
                colors.accent,
            );
        }
        let label_color = if *cat == selected {
            colors.accent
        } else {
            colors.text
        };
        painter.text_clipped(
            font,
            cat.label(),
            SIDEBAR_PAD_X,
            y + SIDEBAR_ITEM_HEIGHT / 2 + 6,
            SIDEBAR_WIDTH - 2 * SIDEBAR_PAD_X,
            label_color,
        );
    }

    // Content area title (selected category name).
    let content_x = SIDEBAR_WIDTH + CONTENT_PAD_X;
    let content_w = width - SIDEBAR_WIDTH - 2 * CONTENT_PAD_X;
    painter.text_clipped(
        font,
        selected.label(),
        content_x,
        CONTENT_TOP_PAD + 18,
        content_w,
        colors.accent,
    );
    // 2px accent strip under the title (metro signature).
    painter.rect(
        Rect {
            x: content_x,
            y: CONTENT_TOP_PAD + TITLE_HEIGHT,
            w: content_w,
            h: ACCENT_STRIP_H,
        },
        colors.accent,
    );

    // Placeholder content per category. Real content (theme picker,
    // cursor size slider, wallpaper file path, pinned-app list) lands
    // in A3.3 and follow-up sessions.
    let body_y = CONTENT_TOP_PAD + TITLE_HEIGHT + ACCENT_STRIP_H + 30;
    let placeholder = match selected {
        SettingsCategory::Theme => "Theme picker — coming next (A3.3)",
        SettingsCategory::Cursor => "Cursor theme + size — A3.4",
        SettingsCategory::Wallpaper => "Wallpaper path + mode — A3.5",
        SettingsCategory::PinnedApps => "Reorder / add / remove pinned apps — A3.6",
    };
    painter.text_clipped(font, placeholder, content_x, body_y, content_w, colors.text_dim);
}
