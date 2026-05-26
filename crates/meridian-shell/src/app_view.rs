use std::collections::HashSet;

use tiny_skia::{FillRule, LineCap, LineJoin, Paint as SkPaint, PathBuilder, Pixmap, PixmapMut, PixmapPaint, Stroke, Transform};

use crate::launcher::DesktopApp;
use crate::panel::PinnedApp;
use crate::{
    icons::{icon_image_to_pixmap, IconCache},
    ui::tokens::theme_from_config,
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
const CP_BENTO_ZONE_H: i32 =
    CP_SECTION_LABEL_H + CP_SECTION_PAD + CP_BENTO_TILE_H + CP_SECTION_PAD;

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
const CP_PWR_START_X: i32 = 676;   // for launcher_w=880

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

pub(crate) fn power_widget_action_for_idx(idx: usize) -> Option<crate::widget_action::WidgetAction> {
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

    let theme = theme_from_config(theme_config);
    let pal = theme.palette;
    pixmap.fill(to_tiny_skia_color(pal.background));

    {
        let mut pm = pixmap.as_mut();

        draw_header(&mut pm, width, search_query, settings_hovered, icon_cache, &pal);
        divider(&mut pm, width, CP_HEADER_H, &pal);

        let search_active = !search_query.is_empty();
        if search_active {
            draw_search_results(
                &mut pm, width, height, all_apps, search_query, scroll_y,
                selected_idx, icon_cache, hidden_execs, hovered_app_idx, &pal,
            );
        } else {
            draw_bento_strip(&mut pm, width, pinned_apps, icon_cache, hovered_bento_idx, &pal);
            divider(&mut pm, width, CP_BENTO_TOP + CP_BENTO_ZONE_H, &pal);
            section_label(&mut pm, "ALLE APPS", CP_APPS_TOP, &pal);
            draw_app_grid(
                &mut pm, width, height, all_apps, search_query, scroll_y,
                selected_idx, icon_cache, hidden_execs, hovered_app_idx, &pal,
            );
        }

        draw_power_footer(&mut pm, width, height, hovered_power_btn, armed_power, &pal);
    }

    blit_rgba_to_argb(pixmap.data(), canvas);
}

fn draw_header(
    pm: &mut PixmapMut<'_>,
    width: u32,
    search_query: &str,
    settings_hovered: bool,
    _icon_cache: &IconCache,
    pal: &meridian_ui::style::Palette,
) {
    fill_rect(pm, Rect { x: 0, y: 0, width: width as i32, height: CP_HEADER_H }, pal.surface);

    let text_x = 20i32;
    let text_baseline = CP_HEADER_H - (CP_HEADER_H - 14) / 2 - 2;
    if search_query.is_empty() {
        let ph = Color::rgba(pal.text.r, pal.text.g, pal.text.b, 80);
        paint_text(pm, "Apps suchen...", text_x, text_baseline, 13.0, ph);
    } else {
        paint_text(pm, search_query, text_x, text_baseline, 13.0, pal.text);
    }

    let sx = cp_settings_btn_x(width);
    let sy = cp_hdr_icon_y();
    let icon_col = if settings_hovered { pal.text } else { pal.text_dim };
    draw_settings_symbol(pm, sx + CP_HDR_ICON_W / 2, sy + CP_HDR_ICON_H / 2, icon_col);
}

fn draw_bento_strip(
    pm: &mut PixmapMut<'_>,
    _width: u32,
    pinned_apps: &[PinnedApp],
    icon_cache: &IconCache,
    hovered_idx: Option<usize>,
    pal: &meridian_ui::style::Palette,
) {
    section_label(pm, "ANGEHEFTET", CP_BENTO_TOP, pal);

    let n = pinned_apps.len().min(CP_MAX_BENTO);
    if n == 0 {
        return;
    }
    let strip_x = cp_bento_tile_x(n);
    let tile_y = CP_BENTO_TOP + CP_SECTION_LABEL_H + CP_SECTION_PAD;

    for (i, app) in pinned_apps.iter().take(CP_MAX_BENTO).enumerate() {
        let tx = strip_x + i as i32 * (CP_BENTO_TILE_W + CP_BENTO_TILE_GAP);

        let bg = if hovered_idx == Some(i) {
            pal.surface.lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.12)
        } else {
            pal.surface
        };
        fill_rect(pm, Rect { x: tx, y: tile_y, width: CP_BENTO_TILE_W, height: CP_BENTO_TILE_H }, bg);
        // bottom accent line
        fill_rect(pm, Rect { x: tx, y: tile_y + CP_BENTO_TILE_H - 1, width: CP_BENTO_TILE_W, height: 1 },
            Color::rgba(pal.accent.r, pal.accent.g, pal.accent.b, 140));

        // icon – try large then small sizes
        if let Some(name) = app.icon_name.as_deref() {
            for &sz in &[48u32, 32, 24] {
                if let Some(img) = icon_cache.lookup(name, sz) {
                    if let Some(pix) = icon_image_to_pixmap(img) {
                        let pw = pix.width() as i32;
                        let ph = pix.height() as i32;
                        let ix = tx + (CP_BENTO_TILE_W - pw) / 2;
                        let icon_center = tile_y + (CP_BENTO_TILE_H * 55) / 100;
                        let iy = icon_center - ph / 2;
                        pm.draw_pixmap(ix, iy, pix.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
                        break;
                    }
                }
            }
        }

        // label
        let max_w = CP_BENTO_TILE_W - 6;
        let label = truncate_to_fit(&app.label, max_w, 11.0);
        let lx = tx + 3;
        let ly = tile_y + CP_BENTO_TILE_H - 5;
        paint_text(pm, &label, lx, ly, 11.0, pal.text);
    }
}

fn draw_app_grid(
    pm: &mut PixmapMut<'_>,
    width: u32,
    height: u32,
    apps: &[DesktopApp],
    search_query: &str,
    scroll_y: i32,
    selected_idx: Option<usize>,
    icon_cache: &IconCache,
    hidden_execs: &HashSet<String>,
    hovered_idx: Option<usize>,
    pal: &meridian_ui::style::Palette,
) {
    let content_y = CP_APPS_TOP + CP_SECTION_LABEL_H + CP_SECTION_PAD;
    let grid_h = (height as i32 - content_y - CP_FOOTER_H - 1).max(0) as u32;
    let Some(mut grid_pix) = Pixmap::new(width, grid_h) else { return };
    grid_pix.fill(to_tiny_skia_color(pal.background));
    {
        let mut gpm = grid_pix.as_mut();
        let filtered = collect_palette_apps(apps, search_query, icon_cache, hidden_execs);
        let n_rows = filtered.len().div_ceil(CP_APP_COLS);
        let content_h = n_rows as i32 * CP_APP_ROW_H;

        for (global_idx, app) in filtered.iter().enumerate() {
            let row = global_idx / CP_APP_COLS;
            let col = global_idx % CP_APP_COLS;
            let row_y = row as i32 * CP_APP_ROW_H - scroll_y;
            if row_y + CP_APP_ROW_H <= 0 { continue; }
            if row_y >= grid_h as i32 { break; }

            let card_x = CP_GUTTER + col as i32 * (CP_CARD_W + CP_COL_GAP);
            let is_sel = selected_idx == Some(global_idx);
            let is_hov = hovered_idx == Some(global_idx);

            if is_sel || is_hov {
                let bg = if is_sel {
                    pal.surface.lerp(pal.accent, 0.10)
                } else {
                    pal.surface
                };
                fill_rect(&mut gpm, Rect { x: card_x, y: row_y, width: CP_CARD_W, height: CP_APP_ROW_H }, bg);
                if is_sel {
                    fill_rect(&mut gpm, Rect { x: card_x, y: row_y, width: 2, height: CP_APP_ROW_H }, pal.accent);
                }
            }

            draw_app_row_content(&mut gpm, app, card_x + 10, row_y, icon_cache, pal);
        }

        if content_h > grid_h as i32 {
            draw_scrollbar(&mut gpm, width, grid_h, content_h, scroll_y, pal);
        }
    }
    pm.draw_pixmap(0, content_y, grid_pix.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
}

fn draw_search_results(
    pm: &mut PixmapMut<'_>,
    width: u32,
    height: u32,
    apps: &[DesktopApp],
    search_query: &str,
    scroll_y: i32,
    selected_idx: Option<usize>,
    icon_cache: &IconCache,
    hidden_execs: &HashSet<String>,
    hovered_idx: Option<usize>,
    pal: &meridian_ui::style::Palette,
) {
    let content_y = CP_HEADER_H + CP_DIVIDER_H;
    let list_h = (height as i32 - content_y - CP_FOOTER_H - 1).max(0) as u32;
    let Some(mut list_pix) = Pixmap::new(width, list_h) else { return };
    list_pix.fill(to_tiny_skia_color(pal.background));
    {
        let mut lpm = list_pix.as_mut();
        let filtered = collect_palette_apps(apps, search_query, icon_cache, hidden_execs);
        let content_h = filtered.len() as i32 * CP_APP_ROW_H;

        for (idx, app) in filtered.iter().enumerate() {
            let row_y = idx as i32 * CP_APP_ROW_H - scroll_y;
            if row_y + CP_APP_ROW_H <= 0 { continue; }
            if row_y >= list_h as i32 { break; }

            let is_sel = selected_idx == Some(idx);
            let is_hov = hovered_idx == Some(idx);

            if is_sel || is_hov {
                let bg = if is_sel {
                    pal.surface.lerp(pal.accent, 0.10)
                } else {
                    pal.surface
                };
                fill_rect(&mut lpm, Rect { x: 0, y: row_y, width: width as i32, height: CP_APP_ROW_H }, bg);
                if is_sel {
                    fill_rect(&mut lpm, Rect { x: 0, y: row_y, width: 3, height: CP_APP_ROW_H }, pal.accent);
                }
            }

            draw_app_row_content(&mut lpm, app, 16, row_y, icon_cache, pal);
        }

        if content_h > list_h as i32 {
            draw_scrollbar(&mut lpm, width, list_h, content_h, scroll_y, pal);
        }
    }
    pm.draw_pixmap(0, content_y, list_pix.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
}

fn draw_app_row_content(
    pm: &mut PixmapMut<'_>,
    app: &DesktopApp,
    icon_x: i32,
    row_y: i32,
    icon_cache: &IconCache,
    pal: &meridian_ui::style::Palette,
) {
    if let Some(name) = app.icon_name.as_deref() {
        if let Some(img) = icon_cache.lookup(name, 24) {
            if let Some(pix) = icon_image_to_pixmap(img) {
                let iy = row_y + (CP_APP_ROW_H - pix.height() as i32) / 2;
                pm.draw_pixmap(icon_x, iy, pix.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
            }
        }
    }
    let tx = icon_x + 24 + 8;
    let ty = row_y + CP_APP_ROW_H - 10;
    let max_w = 240;
    let label = truncate_to_fit(&app.name, max_w, 13.0);
    paint_text(pm, &label, tx, ty, 13.0, pal.text);
}

fn draw_power_footer(
    pm: &mut PixmapMut<'_>,
    width: u32,
    launcher_h: u32,
    hovered_idx: Option<usize>,
    armed_power: Option<(&str, f32)>,
    pal: &meridian_ui::style::Palette,
) {
    let footer_y = cp_footer_y(launcher_h);
    // divider + background
    fill_rect(pm, Rect { x: 0, y: footer_y - 1, width: width as i32, height: 1 }, divider_col(pal));
    fill_rect(pm, Rect { x: 0, y: footer_y, width: width as i32, height: CP_FOOTER_H }, pal.surface);

    let btn_y = footer_y + (CP_FOOTER_H - CP_PWR_BTN_SIZE) / 2;

    for i in 0..5usize {
        let bx = CP_PWR_START_X + i as i32 * CP_PWR_BTN_STRIDE;
        let is_hov = hovered_idx == Some(i);
        let is_armed = armed_power
            .map(|(id, _)| id == POWER_IDS[i])
            .unwrap_or(false);

        let col = if is_armed {
            pal.error
        } else if is_hov {
            pal.text
        } else {
            pal.text_dim
        };

        draw_power_symbol(pm, i, bx + CP_PWR_BTN_SIZE / 2, btn_y + CP_PWR_BTN_SIZE / 2, col);

        // arm progress bar
        if is_armed {
            if let Some((_, p)) = armed_power.filter(|(id, _)| *id == POWER_IDS[i]) {
                let bar_w = (CP_PWR_BTN_SIZE as f32 * p) as i32;
                fill_rect(pm, Rect { x: bx, y: btn_y + CP_PWR_BTN_SIZE - 1, width: bar_w, height: 1 }, pal.error);
            }
        }
    }
}


fn draw_settings_symbol(pm: &mut PixmapMut<'_>, cx: i32, cy: i32, col: meridian_ui::style::Color) {
    let ts_col = tiny_skia::Color::from_rgba8(col.r, col.g, col.b, col.a);
    let mut paint = SkPaint::default();
    paint.set_color(ts_col);
    paint.anti_alias = true;
    let stroke = Stroke { width: 1.5, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
    let fx = cx as f32;
    let fy = cy as f32;
    // Three slider lines
    for dy in [-6.0f32, 0.0, 6.0] {
        let mut pb = PathBuilder::new();
        pb.move_to(fx - 9.0, fy + dy);
        pb.line_to(fx + 9.0, fy + dy);
        if let Some(p) = pb.finish() {
            pm.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
        }
    }
    // Three knobs (filled circles) at different x positions
    for (dy, kx) in [(-6.0f32, -3.5f32), (0.0, 2.5), (6.0, -1.0)] {
        let mut pb = PathBuilder::new();
        pb.push_circle(fx + kx, fy + dy, 3.0);
        if let Some(p) = pb.finish() {
            pm.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }
}


fn arc_seg(pb: &mut PathBuilder, cx: f32, cy: f32, r: f32, start_deg: f32, end_deg: f32, first_move: bool) {
    let n = ((end_deg - start_deg).abs() / 5.0).ceil() as usize + 2;
    for i in 0..=n {
        let t = (start_deg + (end_deg - start_deg) * i as f32 / n as f32).to_radians();
        let x = cx + r * t.cos();
        let y = cy + r * t.sin();
        if i == 0 && first_move { pb.move_to(x, y); } else { pb.line_to(x, y); }
    }
}

fn draw_power_symbol(pm: &mut PixmapMut<'_>, idx: usize, cx: i32, cy: i32, col: meridian_ui::style::Color) {
    let fx = cx as f32;
    let fy = cy as f32;
    let ts_col = tiny_skia::Color::from_rgba8(col.r, col.g, col.b, col.a);
    let mut paint = SkPaint::default();
    paint.set_color(ts_col);
    paint.anti_alias = true;
    let stroke = Stroke { width: 1.5, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
    let do_stroke = |pm: &mut PixmapMut<'_>, pb: PathBuilder| {
        if let Some(p) = pb.finish() {
            pm.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
        }
    };
    match idx {
        0 => {
            // Lock — shackle arc + body rect + keyhole dot
            let mut pb = PathBuilder::new();
            pb.move_to(fx - 4.0, fy - 2.0);
            pb.line_to(fx - 4.0, fy - 6.0);
            arc_seg(&mut pb, fx, fy - 6.0, 4.0, 180.0, 360.0, false);
            pb.line_to(fx + 4.0, fy - 2.0);
            do_stroke(pm, pb);
            let mut pb = PathBuilder::new();
            pb.move_to(fx - 6.0, fy - 2.0);
            pb.line_to(fx + 6.0, fy - 2.0);
            pb.line_to(fx + 6.0, fy + 6.0);
            pb.line_to(fx - 6.0, fy + 6.0);
            pb.close();
            do_stroke(pm, pb);
            let mut pb = PathBuilder::new();
            pb.push_circle(fx, fy + 2.0, 2.0);
            do_stroke(pm, pb);
        }
        1 => {
            // Logout — door bar + right arrow
            let mut pb = PathBuilder::new();
            pb.move_to(fx - 7.0, fy - 7.0);
            pb.line_to(fx - 7.0, fy + 7.0);
            pb.move_to(fx - 2.0, fy);
            pb.line_to(fx + 7.0, fy);
            pb.move_to(fx + 3.0, fy - 4.0);
            pb.line_to(fx + 7.0, fy);
            pb.line_to(fx + 3.0, fy + 4.0);
            do_stroke(pm, pb);
        }
        2 => {
            // Sleep — crescent moon via EvenOdd (big circle minus offset smaller circle)
            let mut pb = PathBuilder::new();
            pb.push_circle(fx - 1.0, fy, 7.5);
            pb.push_circle(fx + 2.5, fy - 1.0, 6.0);
            if let Some(p) = pb.finish() {
                let mut fp = SkPaint::default();
                fp.set_color(ts_col);
                fp.anti_alias = true;
                pm.fill_path(&p, &fp, FillRule::EvenOdd, Transform::identity(), None);
            }
        }
        3 => {
            // Restart — 270° arc with arrowhead
            let r = 7.0f32;
            let mut pb = PathBuilder::new();
            arc_seg(&mut pb, fx, fy, r, -30.0, -30.0 + 270.0, true);
            do_stroke(pm, pb);
            // Arrowhead at end of arc (240°): tip at (fx-3.5, fy-6.06)
            let end_rad = 240.0f32.to_radians();
            let ex = fx + r * end_rad.cos();
            let ey = fy + r * end_rad.sin();
            let al = 4.5f32;
            // Arms rotated ±150° from CW tangent at 240° = (-0.866, 0.5)
            let mut pb = PathBuilder::new();
            pb.move_to(ex + 0.5 * al, ey - 0.866 * al);
            pb.line_to(ex, ey);
            pb.line_to(ex + 1.0 * al, ey);
            do_stroke(pm, pb);
        }
        _ => {
            // Power off — 300° circle arc + vertical line through top gap
            let r = 7.0f32;
            let mut pb = PathBuilder::new();
            arc_seg(&mut pb, fx, fy, r, -60.0, -60.0 + 300.0, true);
            do_stroke(pm, pb);
            let mut pb = PathBuilder::new();
            pb.move_to(fx, fy - 3.0);
            pb.line_to(fx, fy - 9.0);
            do_stroke(pm, pb);
        }
    }
}

fn draw_scrollbar(
    pm: &mut PixmapMut<'_>,
    width: u32,
    view_h: u32,
    content_h: i32,
    scroll_y: i32,
    pal: &meridian_ui::style::Palette,
) {
    let track_x = width as i32 - 6;
    let track_h = view_h as i32 - 8;
    if track_h <= 0 { return; }
    let thumb_h = ((track_h * view_h as i32) / content_h).max(20).min(track_h);
    let max_scroll = (content_h - view_h as i32).max(1);
    let thumb_y = 4 + scroll_y * (track_h - thumb_h) / max_scroll;
    let track_col = Color::rgba(pal.text.r, pal.text.g, pal.text.b, 25);
    let thumb_col = Color::rgba(pal.accent.r, pal.accent.g, pal.accent.b, 180);
    fill_rect(pm, Rect { x: track_x, y: 4, width: 4, height: track_h }, track_col);
    fill_rect(pm, Rect { x: track_x, y: thumb_y, width: 4, height: thumb_h }, thumb_col);
}

fn section_label(pm: &mut PixmapMut<'_>, label: &str, y: i32, pal: &meridian_ui::style::Palette) {
    paint_text(pm, label, CP_GUTTER, y + CP_SECTION_LABEL_H - 6, 10.0, pal.text_dim);
}

fn divider(pm: &mut PixmapMut<'_>, width: u32, y: i32, pal: &meridian_ui::style::Palette) {
    fill_rect(pm, Rect { x: 0, y, width: width as i32, height: CP_DIVIDER_H }, divider_col(pal));
}

fn divider_col(pal: &meridian_ui::style::Palette) -> Color {
    Color::rgba(pal.accent.r, pal.accent.g, pal.accent.b, 60)
}

fn fill_rect(pm: &mut PixmapMut<'_>, rect: Rect, color: Color) {
    if let Some(path) = rounded_rect_path(rect, 0) {
        paint_fill(pm, &path, color);
    }
}

fn blit_rgba_to_argb(src: &[u8], dst: &mut [u8]) {
    if src.len() != dst.len() || !src.len().is_multiple_of(4) {
        return;
    }
    for (rgba, argb) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        argb[0] = rgba[2];
        argb[1] = rgba[1];
        argb[2] = rgba[0];
        argb[3] = rgba[3];
    }
}

fn to_tiny_skia_color(color: Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(color.r, color.g, color.b, color.a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blit_rgba_to_argb_swaps_red_and_blue() {
        let src = [0x12u8, 0x34, 0x56, 0x78];
        let mut dst = [0u8; 4];
        blit_rgba_to_argb(&src, &mut dst);
        assert_eq!(dst, [0x56, 0x34, 0x12, 0x78]);
    }

    #[test]
    fn blit_twice_roundtrips() {
        let src = [0x12u8, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0];
        let mut mid = [0u8; 8];
        let mut dst = [0u8; 8];
        blit_rgba_to_argb(&src, &mut mid);
        blit_rgba_to_argb(&mid, &mut dst);
        assert_eq!(dst, src);
    }

    #[test]
    fn hit_bento_tile_correct_columns() {
        // 8 tiles: strip_x = (880−824)/2 = 28; stride=104
        let n = 8;
        let tile_y = CP_BENTO_TOP + CP_SECTION_LABEL_H + CP_SECTION_PAD;
        assert_eq!(hit_bento_tile(50, tile_y + 10, n), Some(0));
        assert_eq!(hit_bento_tile(132, tile_y + 10, n), Some(1)); // 28+104=132
        assert_eq!(hit_bento_tile(28, tile_y - 1, n), None);
        assert_eq!(hit_bento_tile(10, tile_y + 10, n), None); // before strip
    }

    #[test]
    fn hit_app_row_grid_mode() {
        // content_y = 190+24+8=222; col0: x in [21,295)
        let idx = hit_app_row(CP_GUTTER + 5, 222 + 5, 0, 620, false);
        assert_eq!(idx, Some(0));
        let idx = hit_app_row(CP_GUTTER + CP_CARD_W + CP_COL_GAP + 5, 222 + 5, 0, 620, false);
        assert_eq!(idx, Some(1)); // col 1
    }

    #[test]
    fn hit_app_row_search_mode() {
        // content_y = 53
        assert_eq!(hit_app_row(100, 53 + 5, 0, 620, true), Some(0));
        assert_eq!(hit_app_row(100, 53 + 44 + 5, 0, 620, true), Some(1));
    }

    #[test]
    fn hit_footer_power_btn_range() {
        // btn 0: x=[676,708), y=[footer+4, footer+36); footer=580
        let h = 620u32;
        assert_eq!(hit_footer_power_btn(680, 588, h), Some(0));
        assert_eq!(hit_footer_power_btn(720, 588, h), Some(1)); // 676+40=716..748
        assert_eq!(hit_footer_power_btn(680, 570, h), None); // above footer
    }

    #[test]
    fn hit_header_settings_btn() {
        // w=880: x=[840,868), y=[12,40)
        assert!(hit_header_settings(850, 20, 880));
        assert!(!hit_header_settings(850, 5, 880));
        assert!(!hit_header_settings(800, 20, 880));
    }
}
