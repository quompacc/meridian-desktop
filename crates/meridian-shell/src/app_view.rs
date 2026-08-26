use meridian_tokens::Interaction;
use std::collections::HashSet;

use tiny_skia::{
    FillRule, LineCap, LineJoin, Paint as SkPaint, PathBuilder, Pixmap, PixmapMut, PixmapPaint,
    Stroke, Transform,
};

use crate::launcher::{DesktopApp, LauncherCategory};
use crate::panel::PinnedApp;
use crate::{
    icons::{icon_image_to_pixmap, IconCache},
    ui::tokens::glass_theme_from_config,
};
use meridian_ui::{
    effect::{paint_border, paint_fill, paint_text, rounded_rect_path, truncate_to_fit},
    paint::Rect,
    style::Color,
};

// ─── Layout constants ─────────────────────────────────────────────────────────
const LAUNCHER_LAYOUT: meridian_tokens::Launcher = meridian_tokens::Launcher::DEFAULT;
pub(crate) const CP_HEADER_H: i32 = LAUNCHER_LAYOUT.header_height;
const CP_DIVIDER_H: i32 = 1;
pub(crate) const CP_SECTION_LABEL_H: i32 = LAUNCHER_LAYOUT.sidebar_heading_height;
pub(crate) const CP_SECTION_PAD: i32 = LAUNCHER_LAYOUT.outer_pad;

pub(crate) const CP_BENTO_TILE_W: i32 =
    LAUNCHER_LAYOUT.sidebar_width - LAUNCHER_LAYOUT.outer_pad * 2;
pub(crate) const CP_BENTO_TILE_H: i32 = LAUNCHER_LAYOUT.favorite_row_height;
const CP_BENTO_TILE_GAP: i32 = LAUNCHER_LAYOUT.footer_button_gap;
pub(crate) const CP_MAX_BENTO: usize = 8;
pub(crate) const CP_BENTO_TOP: i32 = CP_HEADER_H + CP_DIVIDER_H;
pub(crate) const CP_APPS_TOP: i32 = CP_HEADER_H + CP_DIVIDER_H;

pub(crate) const CP_APP_ROW_H: i32 = LAUNCHER_LAYOUT.app_card_height + LAUNCHER_LAYOUT.grid_gap;
pub(crate) const CP_APP_COLS: usize = LAUNCHER_LAYOUT.grid_columns as usize;
pub(crate) const CP_GUTTER: i32 = LAUNCHER_LAYOUT.sidebar_width + LAUNCHER_LAYOUT.content_pad;
pub(crate) const CP_COL_GAP: i32 = LAUNCHER_LAYOUT.grid_gap;
pub(crate) const CP_CARD_W: i32 = (LAUNCHER_LAYOUT.width
    - LAUNCHER_LAYOUT.sidebar_width
    - LAUNCHER_LAYOUT.content_pad * 2
    - LAUNCHER_LAYOUT.grid_gap)
    / LAUNCHER_LAYOUT.grid_columns;

// Power footer
pub(crate) const CP_FOOTER_H: i32 = LAUNCHER_LAYOUT.footer_height;
const CP_PWR_BTN_SIZE: i32 = LAUNCHER_LAYOUT.footer_button_size;
const CP_PWR_BTN_STRIDE: i32 = CP_PWR_BTN_SIZE + LAUNCHER_LAYOUT.footer_button_gap;
const CP_FOOTER_ACTIONS: i32 = 6;
const CP_PWR_START_X: i32 = LAUNCHER_LAYOUT.width
    - LAUNCHER_LAYOUT.outer_pad
    - CP_FOOTER_ACTIONS * CP_PWR_BTN_SIZE
    - (CP_FOOTER_ACTIONS - 1) * LAUNCHER_LAYOUT.footer_button_gap
    + CP_PWR_BTN_STRIDE;

// Header settings button
const CP_HDR_ICON_W: i32 = CP_PWR_BTN_SIZE;
const CP_HDR_ICON_H: i32 = CP_PWR_BTN_SIZE;

const POWER_IDS: [&str; 5] = [
    "power-lock",
    "power-logout",
    "power-sleep",
    "power-restart",
    "power-off",
];

// Launcher overlay opacities now live centrally in `meridian_tokens::Launcher`
// / `Scrollbar` so a global look change touches one place (GUI-centralization
// plan §6, DoD §9). These aliases keep the call sites readable.
const LAUNCHER_BAND_ALPHA: u8 = meridian_tokens::Launcher::DEFAULT.band_alpha;
const LAUNCHER_HOVER_ALPHA: u8 = meridian_tokens::Launcher::DEFAULT.hover_alpha;
const LAUNCHER_SELECTED_ALPHA: u8 = meridian_tokens::Launcher::DEFAULT.selected_alpha;
const LAUNCHER_TILE_RADIUS: i32 = meridian_tokens::Radius::DEFAULT.md;

const LAUNCHER_SEARCH_FIELD_ALPHA: u8 = meridian_tokens::Launcher::DEFAULT.search_field_alpha;
const LAUNCHER_SEARCH_FOCUS_ALPHA: u8 = meridian_tokens::Launcher::DEFAULT.search_focus_alpha;
const LAUNCHER_POWER_ARMED_ALPHA: u8 = meridian_tokens::Launcher::DEFAULT.power_armed_alpha;
const LAUNCHER_SCROLLBAR_TRACK_ALPHA: u8 = meridian_tokens::Scrollbar::DEFAULT.track_alpha;
const LAUNCHER_SCROLLBAR_THUMB_ALPHA: u8 = meridian_tokens::Scrollbar::DEFAULT.thumb_alpha;
const LAUNCHER_DIVIDER_ALPHA: u8 = meridian_tokens::Launcher::DEFAULT.divider_alpha;

// ─── Hit testing ──────────────────────────────────────────────────────────────

fn cp_settings_btn_x(launcher_w: u32) -> i32 {
    launcher_w as i32
        - LAUNCHER_LAYOUT.outer_pad
        - CP_FOOTER_ACTIONS * CP_PWR_BTN_SIZE
        - (CP_FOOTER_ACTIONS - 1) * LAUNCHER_LAYOUT.footer_button_gap
}

fn cp_hdr_icon_y(launcher_h: u32) -> i32 {
    cp_footer_y(launcher_h) + (CP_FOOTER_H - CP_HDR_ICON_H) / 2
}

fn cp_footer_y(launcher_h: u32) -> i32 {
    launcher_h as i32 - CP_FOOTER_H
}

pub(crate) fn hit_bento_tile(cx: i32, cy: i32, n_tiles: usize) -> Option<usize> {
    if n_tiles == 0 {
        return None;
    }
    let tile_y = CP_BENTO_TOP + CP_SECTION_LABEL_H;
    if !(CP_SECTION_PAD..CP_SECTION_PAD + CP_BENTO_TILE_W).contains(&cx) || cy < tile_y {
        return None;
    }
    let stride = CP_BENTO_TILE_H + CP_BENTO_TILE_GAP;
    let row = (cy - tile_y) / stride;
    let in_tile = (cy - tile_y) % stride < CP_BENTO_TILE_H;
    if in_tile && row < n_tiles.min(CP_MAX_BENTO) as i32 {
        Some(row as usize)
    } else {
        None
    }
}

pub(crate) fn hit_app_row(
    cx: i32,
    cy: i32,
    scroll_y: i32,
    launcher_h: u32,
    _search_active: bool,
) -> Option<usize> {
    let footer_y = cp_footer_y(launcher_h);
    let content_y = CP_APPS_TOP + LAUNCHER_LAYOUT.content_pad + LAUNCHER_LAYOUT.app_heading_height;
    if cy < content_y || cy >= footer_y - 1 {
        return None;
    }
    let row_y = cy - content_y + scroll_y;
    if row_y < 0 {
        return None;
    }
    let row = (row_y / CP_APP_ROW_H) as usize;
    let rel_x = cx - CP_GUTTER;
    if rel_x < 0 {
        return None;
    }
    let col_stride = CP_CARD_W + CP_COL_GAP;
    let col = (rel_x / col_stride) as usize;
    let in_card_y = row_y % CP_APP_ROW_H < LAUNCHER_LAYOUT.app_card_height;
    if in_card_y && rel_x % col_stride < CP_CARD_W && col < CP_APP_COLS {
        Some(row * CP_APP_COLS + col)
    } else {
        None
    }
}

pub(crate) fn hit_header_settings(cx: i32, cy: i32, launcher_w: u32) -> bool {
    let bx = cp_settings_btn_x(launcher_w);
    let by = cp_hdr_icon_y(LAUNCHER_LAYOUT.height as u32);
    cx >= bx && cx < bx + CP_HDR_ICON_W && cy >= by && cy < by + CP_HDR_ICON_H
}

/// Returns power button index 0=lock 1=logout 2=sleep 3=restart 4=off, or None.
pub(crate) fn hit_footer_power_btn(cx: i32, cy: i32, launcher_h: u32) -> Option<usize> {
    let footer_y = cp_footer_y(launcher_h);
    let btn_y = footer_y + (CP_FOOTER_H - CP_PWR_BTN_SIZE) / 2;
    if cy < btn_y || cy >= btn_y + CP_PWR_BTN_SIZE {
        return None;
    }
    let rel_x = cx - CP_PWR_START_X;
    if rel_x < 0 {
        return None;
    }
    let btn = (rel_x / CP_PWR_BTN_STRIDE) as usize;
    let in_btn = rel_x % CP_PWR_BTN_STRIDE < CP_PWR_BTN_SIZE;
    if in_btn && btn < 5 {
        Some(btn)
    } else {
        None
    }
}

pub(crate) fn power_widget_action_for_idx(
    idx: usize,
) -> Option<crate::widget_action::WidgetAction> {
    Some(match idx {
        0 => crate::widget_action::WidgetAction::PowerLock,
        1 => crate::widget_action::WidgetAction::PowerLogout,
        2 => crate::widget_action::WidgetAction::PowerSleep,
        3 => crate::widget_action::WidgetAction::PowerRestart,
        4 => crate::widget_action::WidgetAction::PowerOff,
        _ => return None,
    })
}

// ─── App filtering ────────────────────────────────────────────────────────────

pub(crate) fn collect_palette_apps<'a>(
    apps: &'a [DesktopApp],
    search_query: &str,
    hidden_execs: &HashSet<String>,
    category: LauncherCategory,
    pinned_apps: &[PinnedApp],
) -> Vec<&'a DesktopApp> {
    let query = search_query.to_lowercase();
    apps.iter()
        .filter(|app| {
            !app.terminal
                && !hidden_execs.contains(&app.program)
                && (query.is_empty() || app.name.to_lowercase().contains(&query))
                && app_matches_category(app, category, pinned_apps)
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GridDirection {
    Left,
    Right,
    Up,
    Down,
}

pub(crate) fn next_grid_selection(
    current: Option<usize>,
    app_count: usize,
    direction: GridDirection,
) -> Option<usize> {
    if app_count == 0 {
        return None;
    }

    let last = app_count - 1;
    Some(match (current, direction) {
        (None, GridDirection::Up) => last,
        (None, _) => 0,
        (Some(index), GridDirection::Left) => index.saturating_sub(1),
        (Some(index), GridDirection::Right) => (index + 1).min(last),
        (Some(index), GridDirection::Up) => index.saturating_sub(CP_APP_COLS),
        (Some(index), GridDirection::Down) => (index + CP_APP_COLS).min(last),
    })
}

pub(crate) fn scroll_grid_selection_into_view(
    current_scroll: i32,
    selected_idx: usize,
    app_count: usize,
) -> i32 {
    let content_y = CP_APPS_TOP + LAUNCHER_LAYOUT.content_pad + LAUNCHER_LAYOUT.app_heading_height;
    let view_h =
        (LAUNCHER_LAYOUT.height as i32 - content_y - CP_FOOTER_H - LAUNCHER_LAYOUT.content_pad)
            .max(0);
    let card_top = (selected_idx / CP_APP_COLS) as i32 * CP_APP_ROW_H;
    let card_bottom = card_top + LAUNCHER_LAYOUT.app_card_height;
    let wanted = if card_top < current_scroll {
        card_top
    } else if card_bottom > current_scroll + view_h {
        card_bottom - view_h
    } else {
        current_scroll
    };
    let rows = app_count.div_ceil(CP_APP_COLS);
    let content_h = rows as i32 * CP_APP_ROW_H - CP_COL_GAP;
    wanted.clamp(0, (content_h - view_h).max(0))
}

fn app_matches_category(
    app: &DesktopApp,
    category: LauncherCategory,
    pinned_apps: &[PinnedApp],
) -> bool {
    use LauncherCategory::*;
    if category == Favorites {
        return pinned_apps
            .iter()
            .any(|pinned| pinned.program == app.program);
    }
    if category == All {
        return true;
    }
    app.categories.iter().any(|value| {
        let value = value.as_str();
        match category {
            Internet => matches!(
                value,
                "network" | "webbrowser" | "email" | "chat" | "instantmessaging"
            ),
            Office => matches!(
                value,
                "office" | "wordprocessor" | "spreadsheet" | "presentation" | "calendar"
            ),
            Development => matches!(value, "development" | "ide" | "debugger"),
            Graphics => matches!(
                value,
                "graphics" | "photography" | "2dgraphics" | "rastergraphics" | "vectorgraphics"
            ),
            System => matches!(
                value,
                "system" | "settings" | "security" | "monitor" | "filesystem"
            ),
            Utilities => matches!(
                value,
                "utility" | "accessories" | "filemanager" | "archiving" | "terminalemulator"
            ),
            Favorites | All => false,
        }
    })
}

pub(crate) fn max_scroll_for_palette(
    apps: &[DesktopApp],
    search_query: &str,
    hidden_execs: &HashSet<String>,
    category: LauncherCategory,
    pinned_apps: &[PinnedApp],
    launcher_h: u32,
) -> i32 {
    let filtered = collect_palette_apps(apps, search_query, hidden_execs, category, pinned_apps);
    let content_y = CP_APPS_TOP + LAUNCHER_LAYOUT.content_pad + LAUNCHER_LAYOUT.app_heading_height;
    let n_rows = filtered.len().div_ceil(CP_APP_COLS);
    let content_h = n_rows as i32 * CP_APP_ROW_H - CP_COL_GAP;
    let view_h = launcher_h as i32 - content_y - CP_FOOTER_H - LAUNCHER_LAYOUT.content_pad;
    (content_h - view_h).max(0)
}

// ─── Rendering ────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_command_palette(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    pinned_apps: &[PinnedApp],
    all_apps: &[DesktopApp],
    category: LauncherCategory,
    search_query: &str,
    scroll_y: i32,
    selected_idx: Option<usize>,
    armed_power: Option<(&str, f32)>,
    icon_cache: &IconCache,
    hidden_execs: &HashSet<String>,
    hovered_app_idx: Option<usize>,
    hovered_bento_idx: Option<usize>,
    settings_hovered: bool,
    hovered_power_btn: Option<usize>,
    theme_config: &meridian_config::ThemeConfig,
) {
    let expected = (width as usize) * (height as usize) * 4;
    if canvas.len() != expected {
        return;
    }
    let Some(mut pixmap) = Pixmap::new(width, height) else {
        return;
    };

    let theme = glass_theme_from_config(theme_config);
    let pal = theme.palette;
    // The shell supplies the token-driven tint while the compositor supplies
    // the cached live blur behind it. Keeping the tint in this buffer also
    // makes the colour swap reliable on DRM paths where a custom glass shader
    // cannot be applied to an overlay plane.
    let body_alpha = theme_config
        .decorations
        .surface_treatment(meridian_config::ThemeSurface::Launcher)
        .fill_alpha;
    pixmap.fill(to_tiny_skia_color(with_alpha(pal.surface_alt, body_alpha)));

    {
        let mut pm = pixmap.as_mut();

        draw_header(
            &mut pm,
            width,
            search_query,
            settings_hovered,
            icon_cache,
            &pal,
        );
        divider(&mut pm, width, CP_HEADER_H, &pal);

        draw_bento_strip(
            &mut pm,
            width,
            all_apps,
            hidden_execs,
            pinned_apps,
            category,
            hovered_bento_idx,
            &pal,
        );
        draw_app_grid(
            &mut pm,
            width,
            height,
            all_apps,
            pinned_apps,
            category,
            search_query,
            scroll_y,
            selected_idx,
            icon_cache,
            hidden_execs,
            hovered_app_idx,
            &pal,
        );

        draw_power_footer(
            &mut pm,
            width,
            height,
            settings_hovered,
            hovered_power_btn,
            armed_power,
            &pal,
        );
    }

    blit_rgba_to_argb(pixmap.data(), canvas);
}

include!("app_view/header_and_grid.rs");
include!("app_view/rows_and_helpers.rs");

#[cfg(test)]
#[path = "app_view_tests.rs"]
mod tests;
