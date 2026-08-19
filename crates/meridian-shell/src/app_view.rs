use meridian_tokens::Interaction;
use std::collections::HashSet;

use tiny_skia::{
    FillRule, LineCap, LineJoin, Paint as SkPaint, PathBuilder, Pixmap, PixmapMut, PixmapPaint,
    Stroke, Transform,
};

use crate::launcher::DesktopApp;
use crate::panel::PinnedApp;
use crate::{
    icons::{icon_image_to_pixmap, IconCache},
    ui::tokens::glass_theme_from_config,
};
use meridian_ui::{
    effect::{paint_fill, paint_text, rounded_rect_path, truncate_to_fit},
    paint::Rect,
    style::Color,
};

// ─── Layout constants ─────────────────────────────────────────────────────────
pub(crate) const CP_HEADER_H: i32 = 52;
const CP_DIVIDER_H: i32 = 1;
const CP_SECTION_LABEL_H: i32 = 24;
const CP_SECTION_PAD: i32 = 8;

pub(crate) const CP_BENTO_TILE_W: i32 = 72;
pub(crate) const CP_BENTO_TILE_H: i32 = 72;
const CP_BENTO_TILE_GAP: i32 = 8;
pub(crate) const CP_MAX_BENTO: usize = 8;
// bento zone: label + top-pad + tiles + bottom-pad
const CP_BENTO_ZONE_H: i32 = CP_SECTION_LABEL_H + CP_SECTION_PAD + CP_BENTO_TILE_H + CP_SECTION_PAD;

// y-coordinates for the two main zones
pub(crate) const CP_BENTO_TOP: i32 = CP_HEADER_H + CP_DIVIDER_H; // 53
pub(crate) const CP_APPS_TOP: i32 = CP_BENTO_TOP + CP_BENTO_ZONE_H + CP_DIVIDER_H; // 190

// App grid: 3-col, gutter 21, gap 8 → card = (880−42−16)/3 = 274
pub(crate) const CP_APP_ROW_H: i32 = 44;
pub(crate) const CP_APP_COLS: usize = 3;
pub(crate) const CP_GUTTER: i32 = 21;
pub(crate) const CP_COL_GAP: i32 = 8;
pub(crate) const CP_CARD_W: i32 = 274;

// Power footer
pub(crate) const CP_FOOTER_H: i32 = 40;
// 5 power buttons × 32px + 4 gaps × 8px = 192, right-margin 12 → leftmost btn x = w−12−192 = 676
const CP_PWR_BTN_SIZE: i32 = 32;
const CP_PWR_BTN_STRIDE: i32 = 40; // 32 + 8 gap
const CP_PWR_START_X: i32 = 676; // for launcher_w=880

// Header settings button
const CP_HDR_ICON_W: i32 = 28;
const CP_HDR_ICON_H: i32 = 28;
const CP_HDR_ICON_MARGIN_R: i32 = 12;

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
const LAUNCHER_CELL_ALPHA: u8 = meridian_tokens::Launcher::DEFAULT.cell_alpha;
const LAUNCHER_HOVER_ALPHA: u8 = meridian_tokens::Launcher::DEFAULT.hover_alpha;
const LAUNCHER_SELECTED_ALPHA: u8 = meridian_tokens::Launcher::DEFAULT.selected_alpha;
const LAUNCHER_TILE_RADIUS: i32 = meridian_tokens::Radius::DEFAULT.md;

const LAUNCHER_SEARCH_FIELD_ALPHA: u8 = meridian_tokens::Launcher::DEFAULT.search_field_alpha;
const LAUNCHER_BENTO_ACCENT_ALPHA: u8 = meridian_tokens::Launcher::DEFAULT.bento_accent_alpha;
const LAUNCHER_POWER_ARMED_ALPHA: u8 = meridian_tokens::Launcher::DEFAULT.power_armed_alpha;
const LAUNCHER_SCROLLBAR_TRACK_ALPHA: u8 = meridian_tokens::Scrollbar::DEFAULT.track_alpha;
const LAUNCHER_SCROLLBAR_THUMB_ALPHA: u8 = meridian_tokens::Scrollbar::DEFAULT.thumb_alpha;
const LAUNCHER_DIVIDER_ALPHA: u8 = meridian_tokens::Launcher::DEFAULT.divider_alpha;

// ─── Hit testing ──────────────────────────────────────────────────────────────

fn cp_settings_btn_x(launcher_w: u32) -> i32 {
    launcher_w as i32 - CP_HDR_ICON_MARGIN_R - CP_HDR_ICON_W
}

fn cp_hdr_icon_y() -> i32 {
    (CP_HEADER_H - CP_HDR_ICON_H) / 2
}

fn cp_footer_y(launcher_h: u32) -> i32 {
    launcher_h as i32 - CP_FOOTER_H
}

fn cp_bento_tile_x(n_tiles: usize) -> i32 {
    let n = n_tiles.min(CP_MAX_BENTO) as i32;
    let total_w = n * CP_BENTO_TILE_W + (n - 1).max(0) * CP_BENTO_TILE_GAP;
    (crate::LAUNCHER_WIDTH as i32 - total_w) / 2
}

pub(crate) fn hit_bento_tile(cx: i32, cy: i32, n_tiles: usize) -> Option<usize> {
    if n_tiles == 0 {
        return None;
    }
    let n = n_tiles.min(CP_MAX_BENTO) as i32;
    let tile_y = CP_BENTO_TOP + CP_SECTION_LABEL_H + CP_SECTION_PAD;
    if cy < tile_y || cy >= tile_y + CP_BENTO_TILE_H {
        return None;
    }
    let strip_x = cp_bento_tile_x(n_tiles);
    let rel_x = cx - strip_x;
    if rel_x < 0 {
        return None;
    }
    let stride = CP_BENTO_TILE_W + CP_BENTO_TILE_GAP;
    let col = rel_x / stride;
    let in_tile = rel_x % stride < CP_BENTO_TILE_W;
    if in_tile && col < n {
        Some(col as usize)
    } else {
        None
    }
}

pub(crate) fn hit_app_row(
    cx: i32,
    cy: i32,
    scroll_y: i32,
    launcher_h: u32,
    search_active: bool,
) -> Option<usize> {
    let footer_y = cp_footer_y(launcher_h);
    let content_y = if search_active {
        CP_HEADER_H + CP_DIVIDER_H
    } else {
        CP_APPS_TOP + CP_SECTION_LABEL_H + CP_SECTION_PAD
    };
    if cy < content_y || cy >= footer_y - 1 {
        return None;
    }
    let row_y = cy - content_y + scroll_y;
    if row_y < 0 {
        return None;
    }
    let row = (row_y / CP_APP_ROW_H) as usize;
    if search_active {
        Some(row)
    } else {
        let rel_x = cx - CP_GUTTER;
        if rel_x < 0 {
            return None;
        }
        let col_stride = CP_CARD_W + CP_COL_GAP;
        let col = (rel_x / col_stride) as usize;
        if rel_x % col_stride < CP_CARD_W && col < CP_APP_COLS {
            Some(row * CP_APP_COLS + col)
        } else {
            None
        }
    }
}

pub(crate) fn hit_header_settings(cx: i32, cy: i32, launcher_w: u32) -> bool {
    let bx = cp_settings_btn_x(launcher_w);
    let by = cp_hdr_icon_y();
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
    icon_cache: &IconCache,
    hidden_execs: &HashSet<String>,
) -> Vec<&'a DesktopApp> {
    apps.iter()
        .filter(|app| {
            !app.terminal
                && !hidden_execs.contains(&app.program)
                && app
                    .icon_name
                    .as_deref()
                    .and_then(|name| icon_cache.lookup(name, 24))
                    .is_some()
                && (search_query.is_empty()
                    || app
                        .name
                        .to_lowercase()
                        .contains(&search_query.to_lowercase()))
        })
        .collect()
}

pub(crate) fn max_scroll_for_palette(
    apps: &[DesktopApp],
    search_query: &str,
    icon_cache: &IconCache,
    hidden_execs: &HashSet<String>,
    launcher_h: u32,
) -> i32 {
    let filtered = collect_palette_apps(apps, search_query, icon_cache, hidden_execs);
    let search_active = !search_query.is_empty();
    let content_y = if search_active {
        CP_HEADER_H + CP_DIVIDER_H
    } else {
        CP_APPS_TOP + CP_SECTION_LABEL_H + CP_SECTION_PAD
    };
    let n_rows = if search_active {
        filtered.len()
    } else {
        filtered.len().div_ceil(CP_APP_COLS)
    };
    let content_h = n_rows as i32 * CP_APP_ROW_H;
    let view_h = launcher_h as i32 - content_y - CP_FOOTER_H - 1;
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
    // The compositor renders the launcher's glass backdrop at the theme's
    // fill_alpha (see `themed_layer_glass_info`). When glass is on, painting an
    // opaque body here too would DOUBLE the opacity and hide the blur — so keep
    // the body transparent and let the compositor glass provide the translucency,
    // exactly like the popups and the launcher's own settings page. Only the
    // non-glass fallback paints a solid body so the launcher stays legible.
    let glass = theme_config.decorations.glass && theme_config.decorations.glass_blur;
    let body_alpha = if glass {
        0
    } else {
        theme_config
            .decorations
            .surface_treatment(meridian_config::ThemeSurface::Launcher)
            .fill_alpha
    };
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

        let search_active = !search_query.is_empty();
        if search_active {
            draw_search_results(
                &mut pm,
                width,
                height,
                all_apps,
                search_query,
                scroll_y,
                selected_idx,
                icon_cache,
                hidden_execs,
                hovered_app_idx,
                &pal,
            );
        } else {
            draw_bento_strip(
                &mut pm,
                width,
                pinned_apps,
                icon_cache,
                hovered_bento_idx,
                &pal,
            );
            divider(&mut pm, width, CP_BENTO_TOP + CP_BENTO_ZONE_H, &pal);
            section_label(&mut pm, "ALLE APPS", CP_APPS_TOP, &pal);
            draw_app_grid(
                &mut pm,
                width,
                height,
                all_apps,
                search_query,
                scroll_y,
                selected_idx,
                icon_cache,
                hidden_execs,
                hovered_app_idx,
                &pal,
            );
        }

        draw_power_footer(&mut pm, width, height, hovered_power_btn, armed_power, &pal);
    }

    blit_rgba_to_argb(pixmap.data(), canvas);
}

include!("app_view/header_and_grid.rs");
include!("app_view/rows_and_helpers.rs");

#[cfg(test)]
#[path = "app_view_tests.rs"]
mod tests;
