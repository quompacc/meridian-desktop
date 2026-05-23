// Phase A3.1+A3.2: paint the settings overlay — sidebar on the left
// (category list), content area on the right (renders the selected
// category's view).
//
// A3.3: Theme picker — shows available_themes as clickable rows;
// selected theme row gets accent colour + left-edge strip.

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
const THEME_ROW_H: i32 = 36;

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

/// Hit-test the theme content area. Returns the index into
/// `available_themes` if the click lands on a valid theme row,
/// or `None` otherwise.
pub fn theme_content_hit_test(
    x: f64,
    y: f64,
    available_themes: &[String],
) -> Option<usize> {
    if x <= (SIDEBAR_WIDTH + 1) as f64 {
        return None;
    }
    let body_y = CONTENT_TOP_PAD + TITLE_HEIGHT + ACCENT_STRIP_H + 30;
    let y_rel = y as i32 - body_y;
    if y_rel < 0 {
        return None;
    }
    let row = (y_rel / THEME_ROW_H) as usize;
    if row < available_themes.len() {
        Some(row)
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
    available_themes: &[String],
    current_theme: &str,
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

    // Content body area.
    let body_y = CONTENT_TOP_PAD + TITLE_HEIGHT + ACCENT_STRIP_H + 30;

    match selected {
        SettingsCategory::Theme => {
            // Theme picker: one clickable row per available theme.
            for (i, name) in available_themes.iter().enumerate() {
                let row_y = body_y + (i as i32) * THEME_ROW_H;
                let is_selected = name.as_str() == current_theme;

                if is_selected {
                    // 3px accent left-edge strip for the active theme.
                    painter.rect(
                        Rect {
                            x: content_x,
                            y: row_y,
                            w: 3,
                            h: THEME_ROW_H,
                        },
                        colors.accent,
                    );
                }
                let text_color = if is_selected {
                    colors.accent
                } else {
                    colors.text
                };
                painter.text_clipped(
                    font,
                    name,
                    content_x + 10,
                    row_y + THEME_ROW_H / 2 + 6,
                    content_w - 10,
                    text_color,
                );
            }
        }
        SettingsCategory::Cursor => {
            painter.text_clipped(
                font,
                "Cursor theme + size — A3.4",
                content_x,
                body_y,
                content_w,
                colors.text_dim,
            );
        }
        SettingsCategory::Wallpaper => {
            painter.text_clipped(
                font,
                "Wallpaper path + mode — A3.5",
                content_x,
                body_y,
                content_w,
                colors.text_dim,
            );
        }
        SettingsCategory::PinnedApps => {
            painter.text_clipped(
                font,
                "Reorder / add / remove pinned apps — A3.6",
                content_x,
                body_y,
                content_w,
                colors.text_dim,
            );
        }
    }
}


const LAUNCHER_FOOTER_H: i32 = 56;
const FOOTER_PAD_X: i32 = 28;
const FOOTER_BTN_W: i32 = 144;
const FOOTER_BTN_H: i32 = 48;

/// Returns true if the click lands inside the ← Back button in the
/// launcher settings footer.
pub fn back_button_hit_test(x: f64, y: f64, launcher_height: u32) -> bool {
    let footer_y = launcher_height as i32 - LAUNCHER_FOOTER_H;
    y >= footer_y as f64
        && y < (footer_y + LAUNCHER_FOOTER_H) as f64
        && x >= FOOTER_PAD_X as f64
        && x < (FOOTER_PAD_X + FOOTER_BTN_W) as f64
}

/// Draws the settings view sized to fit inside the launcher — content
/// in the upper portion, a footer strip with Back button at the bottom.
pub fn draw_settings_launcher(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    width: u32,
    height: u32,
    selected: SettingsCategory,
    available_themes: &[String],
    current_theme: &str,
) {
    let content_h = (height as i32 - LAUNCHER_FOOTER_H).max(0) as u32;
    draw_settings(painter, font, theme, width, content_h, selected, available_themes, current_theme);

    let colors = &theme.colors;
    let footer_y = content_h as i32;
    let w = width as i32;

    painter.rect(Rect { x: 0, y: footer_y, w, h: 1 }, colors.border);
    painter.rect(Rect { x: 0, y: footer_y + 1, w, h: LAUNCHER_FOOTER_H - 1 }, colors.surface);

    let btn_y = footer_y + (LAUNCHER_FOOTER_H - FOOTER_BTN_H) / 2;
    painter.roundish_rect(
        Rect { x: FOOTER_PAD_X, y: btn_y, w: FOOTER_BTN_W, h: FOOTER_BTN_H },
        colors.accent_alt,
    );
    painter.text_clipped(
        font,
        "← Back",
        FOOTER_PAD_X + 12,
        btn_y + FOOTER_BTN_H / 2 + 6,
        FOOTER_BTN_W - 24,
        colors.surface,
    );
}
