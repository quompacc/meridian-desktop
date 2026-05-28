use meridian_config::ThemeConfig;
use meridian_ui::{
    effect::{paint_border, paint_fill, paint_text, rounded_rect_path},
    paint::Rect,
};
use tiny_skia::Pixmap;

use crate::ui::tokens::palette_from_config;

pub(crate) const MENU_WIDTH: i32 = 236;
pub(crate) const SUBMENU_GAP: i32 = 6;
pub(crate) const SUBMENU_WIDTH: i32 = 188;
/// Index of the "Einstellungen ▸" item in desktop_item_list().
pub(crate) const SETTINGS_ITEM_IDX: usize = 3;

const ICON_SZ: f32 = 16.0;
const ICON_GAP: i32 = 12;
const ITEM_H: i32 = 36;
const VPAD: i32 = 6;
const PADDING_X: i32 = 14;
const FONT_SIZE: f32 = 13.0;
const CORNER_R: i32 = 10;

/// Total surface width depending on whether the settings flyout is open.
pub(crate) fn total_menu_width(submenu_open: bool) -> i32 {
    if submenu_open {
        MENU_WIDTH + SUBMENU_GAP + SUBMENU_WIDTH
    } else {
        MENU_WIDTH
    }
}

/// Height of the settings flyout panel (no separator).
pub(crate) fn submenu_height() -> i32 {
    VPAD * 2 + submenu_items().len() as i32 * ITEM_H
}

/// Combined surface height for the given main-menu item count and submenu state.
pub(crate) fn surface_height(n: usize, submenu_open: bool) -> i32 {
    let main_h = menu_height(n);
    if submenu_open {
        main_h.max(submenu_height())
    } else {
        main_h
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContextMenuAction {
    Launch,
    NewWindow,
    LaunchInTerminal,
    PinToPanel,
    UnpinFromPanel,
    RemoveFromLauncher,
}

pub(crate) struct ContextMenuState {
    /// Menu top-left in launcher-surface pixels.
    pub x: i32,
    pub y: i32,
    pub app_name: Box<str>,
    pub exec: Box<str>,
    pub is_terminal: bool,
    pub is_pinned: bool,
    /// Window ID to focus when action=Launch and app is already running.
    pub running_window_id: Option<String>,
    pub hover_idx: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DesktopContextMenuAction {
    Terminal,
    Launcher,
    FileManager,
    Settings,
}

/// Sub-actions for the "Einstellungen ▸" flyout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsSubAction {
    Display,
    Wallpaper,
    Theme,
    Sound,
    Network,
    Power,
}

pub(crate) struct DesktopContextMenuState {
    /// Menu top-left in desktop-surface pixels.
    pub x: i32,
    pub y: i32,
    pub hover_idx: Option<usize>,
    /// Whether the settings flyout is currently shown.
    pub submenu_open: bool,
    pub submenu_hover_idx: Option<usize>,
}

/// Build the item list from the current state flags.
pub(crate) fn item_list(
    is_terminal: bool,
    is_pinned: bool,
    is_running: bool,
) -> Vec<(&'static str, ContextMenuAction)> {
    let mut items: Vec<(&str, ContextMenuAction)> = Vec::new();
    items.push(if is_running {
        ("Fokussieren", ContextMenuAction::Launch)
    } else {
        ("Starten", ContextMenuAction::Launch)
    });
    items.push(("Neues Fenster", ContextMenuAction::NewWindow));
    if !is_terminal {
        items.push(("Im Terminal starten", ContextMenuAction::LaunchInTerminal));
    }
    if is_pinned {
        items.push(("Vom Panel lösen", ContextMenuAction::UnpinFromPanel));
    } else {
        items.push(("An Panel anheften", ContextMenuAction::PinToPanel));
    }
    items.push(("Entfernen", ContextMenuAction::RemoveFromLauncher));
    items
}

pub(crate) fn desktop_item_list() -> Vec<(&'static str, DesktopContextMenuAction)> {
    vec![
        ("Terminal öffnen", DesktopContextMenuAction::Terminal),
        ("Launcher öffnen", DesktopContextMenuAction::Launcher),
        ("Dateimanager öffnen", DesktopContextMenuAction::FileManager),
        ("Einstellungen", DesktopContextMenuAction::Settings),
    ]
}

pub(crate) fn submenu_items() -> Vec<(&'static str, SettingsSubAction)> {
    vec![
        ("Anzeige", SettingsSubAction::Display),
        ("Hintergrund", SettingsSubAction::Wallpaper),
        ("Design", SettingsSubAction::Theme),
        ("Sound", SettingsSubAction::Sound),
        ("Netzwerk", SettingsSubAction::Network),
        ("Energie", SettingsSubAction::Power),
    ]
}

/// Total pixel height of the menu for `n` items (includes separator + padding).
pub(crate) fn menu_height(n: usize) -> i32 {
    VPAD * 2 + n as i32 * ITEM_H + 1
}

/// Clamp menu position so it fits entirely inside the launcher surface.
pub(crate) fn clamp_position(
    cx: i32,
    cy: i32,
    n: usize,
    launcher_w: i32,
    launcher_h: i32,
) -> (i32, i32) {
    let mh = menu_height(n);
    let x = cx.min(launcher_w - MENU_WIDTH).max(0);
    let y = if cy + mh > launcher_h {
        (cy - mh).max(0)
    } else {
        cy
    };
    (x, y)
}

/// True if `(px, py)` is anywhere inside the menu bounding box.
pub(crate) fn contains_point(state: &ContextMenuState, n: usize, px: f64, py: f64) -> bool {
    let ix = px as i32;
    let iy = py as i32;
    let mh = menu_height(n);
    ix >= state.x && ix < state.x + MENU_WIDTH && iy >= state.y && iy < state.y + mh
}

/// Returns the 0-based item index under `(px, py)`, or `None`.
///
/// The visual separator sits between items[n-2] and items[n-1] (the pin action
/// is always last). It shifts the last item down by 1px.
pub(crate) fn hit_item(state: &ContextMenuState, n: usize, px: f64, py: f64) -> Option<usize> {
    if !contains_point(state, n, px, py) {
        return None;
    }
    hit_item_at(state.x, state.y, n, px, py)
}

#[allow(dead_code)]
pub(crate) fn desktop_clamp_position(
    cx: i32,
    cy: i32,
    desktop_w: i32,
    desktop_h: i32,
) -> (i32, i32) {
    let n = desktop_item_list().len();
    clamp_position(cx, cy, n, desktop_w, desktop_h)
}

pub(crate) fn desktop_contains_point(state: &DesktopContextMenuState, px: f64, py: f64) -> bool {
    let n = desktop_item_list().len();
    let ix = px as i32;
    let iy = py as i32;
    let mh = menu_height(n);
    ix >= state.x && ix < state.x + MENU_WIDTH && iy >= state.y && iy < state.y + mh
}

pub(crate) fn desktop_hit_item(state: &DesktopContextMenuState, px: f64, py: f64) -> Option<usize> {
    if !desktop_contains_point(state, px, py) {
        return None;
    }
    hit_item_at(state.x, state.y, desktop_item_list().len(), px, py)
}

pub(crate) fn desktop_hit_item_local(px: f64, py: f64) -> Option<usize> {
    let state = DesktopContextMenuState {
        x: 0,
        y: 0,
        hover_idx: None,
        submenu_open: false,
        submenu_hover_idx: None,
    };
    desktop_hit_item(&state, px, py)
}

/// True when `px` falls inside the settings flyout column (surface-local coords).
pub(crate) fn is_in_submenu_area(px: f64) -> bool {
    let lx = px as i32;
    lx >= MENU_WIDTH + SUBMENU_GAP && lx < MENU_WIDTH + SUBMENU_GAP + SUBMENU_WIDTH
}

/// Returns the 0-based flyout item index under surface-local `(px, py)`, or `None`.
pub(crate) fn submenu_hit_item_local(px: f64, py: f64) -> Option<usize> {
    let lx = px as i32 - MENU_WIDTH - SUBMENU_GAP;
    let ly = py as i32;
    let n = submenu_items().len();
    if lx < 0 || lx >= SUBMENU_WIDTH {
        return None;
    }
    if ly < VPAD || ly >= VPAD + n as i32 * ITEM_H {
        return None;
    }
    let i = ((ly - VPAD) / ITEM_H) as usize;
    if i < n { Some(i) } else { None }
}

fn hit_item_at(x: i32, y: i32, n: usize, px: f64, py: f64) -> Option<usize> {
    let sep_before = n.saturating_sub(1);
    for i in 0..n {
        let extra = if i >= sep_before { 1 } else { 0 };
        let top = y + VPAD + i as i32 * ITEM_H + extra;
        let bot = top + ITEM_H;
        let iy = py as i32;
        let ix = px as i32;
        if ix >= x && ix < x + MENU_WIDTH && iy >= top && iy < bot {
            return Some(i);
        }
    }
    None
}

/// Monochrome line-art glyphs for menu entries, drawn in the theme text colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuIcon {
    Terminal,
    Launcher,
    FileManager,
    Settings,
}

fn icon_for_desktop(action: DesktopContextMenuAction) -> MenuIcon {
    match action {
        DesktopContextMenuAction::Terminal => MenuIcon::Terminal,
        DesktopContextMenuAction::Launcher => MenuIcon::Launcher,
        DesktopContextMenuAction::FileManager => MenuIcon::FileManager,
        DesktopContextMenuAction::Settings => MenuIcon::Settings,
    }
}

/// Draw a 16-unit-viewbox line-art icon at (`ox`,`oy`) scaled to `sz` px.
fn draw_menu_icon(
    canvas: &mut tiny_skia::PixmapMut<'_>,
    ox: f32,
    oy: f32,
    sz: f32,
    icon: MenuIcon,
    color: meridian_ui::style::Color,
) {
    use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Stroke, Transform};
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    let sw = (sz / 16.0 * 1.5).max(1.0);
    let stroke = Stroke {
        width: sw,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    };
    let m = |x: f32, y: f32| (ox + x / 16.0 * sz, oy + y / 16.0 * sz);
    let stroke_pb = |canvas: &mut tiny_skia::PixmapMut<'_>, pb: PathBuilder| {
        if let Some(path) = pb.finish() {
            canvas.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    };
    match icon {
        MenuIcon::Terminal => {
            let mut pb = PathBuilder::new();
            let (l, t) = m(2.0, 3.5);
            let (r, b) = m(14.0, 12.5);
            pb.move_to(l, t);
            pb.line_to(r, t);
            pb.line_to(r, b);
            pb.line_to(l, b);
            pb.close();
            stroke_pb(canvas, pb);
            let mut pb2 = PathBuilder::new();
            let (a, c) = m(4.5, 6.0);
            let (d, e) = m(6.8, 8.0);
            let (f, g) = m(4.5, 10.0);
            pb2.move_to(a, c);
            pb2.line_to(d, e);
            pb2.line_to(f, g);
            stroke_pb(canvas, pb2);
            let mut pb3 = PathBuilder::new();
            let (u0, uy) = m(8.2, 10.0);
            let (u1, _) = m(11.5, 10.0);
            pb3.move_to(u0, uy);
            pb3.line_to(u1, uy);
            stroke_pb(canvas, pb3);
        }
        MenuIcon::Launcher => {
            for &(cx, cy) in &[(4.6, 4.6), (11.4, 4.6), (4.6, 11.4), (11.4, 11.4)] {
                let (px, py) = m(cx, cy);
                let half = sz / 16.0 * 2.3;
                if let Some(rect) =
                    tiny_skia::Rect::from_xywh(px - half, py - half, half * 2.0, half * 2.0)
                {
                    let mut pb = PathBuilder::new();
                    pb.push_rect(rect);
                    if let Some(path) = pb.finish() {
                        canvas.fill_path(
                            &path,
                            &paint,
                            FillRule::Winding,
                            Transform::identity(),
                            None,
                        );
                    }
                }
            }
        }
        MenuIcon::FileManager => {
            // Folder icon: body + tab
            let mut pb = PathBuilder::new();
            let (l, t) = m(2.0, 6.0);
            let (r, b) = m(14.0, 13.0);
            // folder body
            pb.move_to(l, t);
            pb.line_to(r, t);
            pb.line_to(r, b);
            pb.line_to(l, b);
            pb.close();
            stroke_pb(canvas, pb);
            // folder tab (top-left)
            let mut pb2 = PathBuilder::new();
            let (tl, tt) = m(2.0, 4.0);
            let (tr, _) = m(7.5, 4.0);
            let (_, tb) = m(0.0, 6.0);
            pb2.move_to(tl, tb);
            pb2.line_to(tl, tt);
            pb2.line_to(tr, tt);
            pb2.line_to(tr, tb);
            stroke_pb(canvas, pb2);
        }
        MenuIcon::Settings => {
            let knobs = [(4.0f32, 11.0f32), (8.0, 5.0), (12.0, 9.5)];
            for &(yy, kx) in &knobs {
                let mut pb = PathBuilder::new();
                let (l, y) = m(2.5, yy);
                let (r, _) = m(13.5, yy);
                pb.move_to(l, y);
                pb.line_to(r, y);
                stroke_pb(canvas, pb);
                let (cx, cy) = m(kx, yy);
                let mut pbk = PathBuilder::new();
                pbk.push_circle(cx, cy, sz / 16.0 * 1.7);
                if let Some(path) = pbk.finish() {
                    canvas.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
                }
            }
        }
    }
}

/// Draw a small right-pointing solid triangle at the right edge of a menu item,
/// indicating that hovering/clicking opens a submenu.
fn draw_submenu_arrow_indicator(
    canvas: &mut tiny_skia::PixmapMut<'_>,
    menu_w: i32,
    item_top: i32,
    color: meridian_ui::style::Color,
) {
    use tiny_skia::{FillRule, Paint, PathBuilder, Transform};
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.set_color_rgba8(color.r, color.g, color.b, (color.a as u32 * 160 / 255) as u8);
    let cx = (menu_w - PADDING_X + 2) as f32;
    let cy = (item_top + ITEM_H / 2) as f32;
    let h = 6.0f32;
    let w = 4.0f32;
    let mut pb = PathBuilder::new();
    pb.move_to(cx - w / 2.0, cy - h / 2.0);
    pb.line_to(cx + w / 2.0, cy);
    pb.line_to(cx - w / 2.0, cy + h / 2.0);
    pb.close();
    if let Some(path) = pb.finish() {
        canvas.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

/// Render the context menu as an overlay onto the existing BGRA `canvas`.
/// `icons` is parallel to `items` (empty = no icon column).
/// `submenu_arrows`: parallel to `items`; if `true` for index `i`, a right-pointing
/// arrow is drawn at the right edge of that item to indicate a flyout.
pub(crate) fn draw_overlay(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    state: &ContextMenuState,
    items: &[(&str, ContextMenuAction)],
    icons: &[MenuIcon],
    submenu_arrows: &[bool],
    theme_config: &ThemeConfig,
) {
    let n = items.len();
    let mw = MENU_WIDTH as u32;
    let mh = menu_height(n) as u32;
    let Some(mut pm) = Pixmap::new(mw, mh) else {
        return;
    };
    let pal = palette_from_config(theme_config);

    // Background
    let bg_rect = Rect {
        x: 0,
        y: 0,
        width: mw as i32,
        height: mh as i32,
    };
    let Some(bg_path) = rounded_rect_path(bg_rect, CORNER_R) else {
        return;
    };
    paint_fill(&mut pm.as_mut(), &bg_path, pal.surface_alt);
    paint_border(&mut pm.as_mut(), &bg_path, pal.border, 1.0);

    // Separator before last item
    let sep_before = n.saturating_sub(1);
    let sep_y = VPAD + sep_before as i32 * ITEM_H;
    let sep_rect = Rect {
        x: 8,
        y: sep_y,
        width: mw as i32 - 16,
        height: 1,
    };
    if let Some(sep_path) = rounded_rect_path(sep_rect, 0) {
        paint_fill(&mut pm.as_mut(), &sep_path, pal.border);
    }

    // Items
    for (i, (label, _)) in items.iter().enumerate() {
        let extra = if i >= sep_before { 1 } else { 0 };
        let item_top = VPAD + i as i32 * ITEM_H + extra;
        let item_rect = Rect {
            x: 2,
            y: item_top,
            width: mw as i32 - 4,
            height: ITEM_H,
        };

        if state.hover_idx == Some(i) {
            if let Some(p) = rounded_rect_path(item_rect, 6) {
                paint_fill(
                    &mut pm.as_mut(),
                    &p,
                    pal.surface_alt.lerp(pal.accent, 0.20),
                );
            }
            let marker = Rect {
                x: item_rect.x + 1,
                y: item_top + 7,
                width: 3,
                height: ITEM_H - 14,
            };
            if let Some(mp) = rounded_rect_path(marker, 1) {
                paint_fill(&mut pm.as_mut(), &mp, pal.accent);
            }
        }

        let text_x = if let Some(icon) = icons.get(i).copied() {
            let iy = item_top as f32 + (ITEM_H as f32 - ICON_SZ) / 2.0;
            draw_menu_icon(
                &mut pm.as_mut(),
                PADDING_X as f32,
                iy,
                ICON_SZ,
                icon,
                pal.text,
            );
            PADDING_X + ICON_SZ as i32 + ICON_GAP
        } else {
            PADDING_X
        };
        let text_y = item_top + ITEM_H - 10;
        paint_text(&mut pm.as_mut(), label, text_x, text_y, FONT_SIZE, pal.text);

        // Right-pointing triangle for items that open a submenu
        if submenu_arrows.get(i).copied().unwrap_or(false) {
            draw_submenu_arrow_indicator(&mut pm.as_mut(), mw as i32, item_top, pal.text);
        }
    }

    blit_over(
        canvas,
        canvas_w as i32,
        canvas_h as i32,
        &pm,
        state.x,
        state.y,
    );
}

/// Render the desktop context menu (main + optional settings flyout) onto BGRA `canvas`.
pub(crate) fn draw_desktop_overlay(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    state: &DesktopContextMenuState,
    items: &[(&str, DesktopContextMenuAction)],
    theme_config: &ThemeConfig,
) {
    let icons: Vec<MenuIcon> = items.iter().map(|(_, a)| icon_for_desktop(*a)).collect();
    // When the flyout is open, keep the Settings item highlighted in the main menu.
    let effective_hover = if state.submenu_open {
        Some(SETTINGS_ITEM_IDX)
    } else {
        state.hover_idx
    };
    let local_state = ContextMenuState {
        x: state.x,
        y: state.y,
        app_name: "Desktop".into(),
        exec: "".into(),
        is_terminal: false,
        is_pinned: false,
        running_window_id: None,
        hover_idx: effective_hover,
    };
    let app_items: Vec<(&str, ContextMenuAction)> = items
        .iter()
        .map(|(label, _)| (*label, ContextMenuAction::Launch))
        .collect();
    // submenu_arrows: mark the Settings item with a right-pointing triangle
    let mut submenu_arrows = vec![false; app_items.len()];
    if SETTINGS_ITEM_IDX < submenu_arrows.len() {
        submenu_arrows[SETTINGS_ITEM_IDX] = true;
    }
    draw_overlay(
        canvas,
        canvas_w,
        canvas_h,
        &local_state,
        &app_items,
        &icons,
        &submenu_arrows,
        theme_config,
    );

    if state.submenu_open {
        draw_submenu_overlay(canvas, canvas_w, canvas_h, state, theme_config);
    }
}

/// Render the settings flyout panel to the right of the main menu.
fn draw_submenu_overlay(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    state: &DesktopContextMenuState,
    theme_config: &ThemeConfig,
) {
    let items = submenu_items();
    let n = items.len();
    let mw = SUBMENU_WIDTH as u32;
    let mh = submenu_height() as u32;
    let Some(mut pm) = Pixmap::new(mw, mh) else {
        return;
    };
    let pal = palette_from_config(theme_config);

    let bg_rect = Rect { x: 0, y: 0, width: mw as i32, height: mh as i32 };
    let Some(bg_path) = rounded_rect_path(bg_rect, CORNER_R) else { return };
    paint_fill(&mut pm.as_mut(), &bg_path, pal.surface_alt);
    paint_border(&mut pm.as_mut(), &bg_path, pal.border, 1.0);

    for (i, (label, _)) in items.iter().enumerate() {
        let item_top = VPAD + i as i32 * ITEM_H;
        let item_rect = Rect {
            x: 2,
            y: item_top,
            width: mw as i32 - 4,
            height: ITEM_H,
        };

        if state.submenu_hover_idx == Some(i) {
            if let Some(p) = rounded_rect_path(item_rect, 6) {
                paint_fill(&mut pm.as_mut(), &p, pal.surface_alt.lerp(pal.accent, 0.20));
            }
            let marker = Rect {
                x: item_rect.x + 1,
                y: item_top + 7,
                width: 3,
                height: ITEM_H - 14,
            };
            if let Some(mp) = rounded_rect_path(marker, 1) {
                paint_fill(&mut pm.as_mut(), &mp, pal.accent);
            }
        }

        let text_y = item_top + ITEM_H - 10;
        paint_text(&mut pm.as_mut(), label, PADDING_X, text_y, FONT_SIZE, pal.text);
    }

    blit_over(
        canvas,
        canvas_w as i32,
        canvas_h as i32,
        &pm,
        state.x + MENU_WIDTH + SUBMENU_GAP,
        state.y,
    );
    let _ = n; // used via items.iter().enumerate()
}

/// Alpha-composite a tiny_skia Pixmap (premultiplied RGBA) over a Wayland BGRA canvas.
fn blit_over(canvas: &mut [u8], cw: i32, ch: i32, pm: &Pixmap, dx: i32, dy: i32) {
    let pw = pm.width() as i32;
    let ph = pm.height() as i32;
    let src = pm.pixels();
    for my in 0..ph {
        let cy = dy + my;
        if cy < 0 || cy >= ch {
            continue;
        }
        for mx in 0..pw {
            let cx = dx + mx;
            if cx < 0 || cx >= cw {
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
            let ci = ((cy * cw + cx) * 4) as usize;
            if ci + 3 >= canvas.len() {
                continue;
            }
            // premultiplied src, straight dst (BGRA)
            let rp = px.red() as u32;
            let gp = px.green() as u32;
            let bp = px.blue() as u32;
            let inv = 255 - a as u32;
            let db = canvas[ci] as u32;
            let dg = canvas[ci + 1] as u32;
            let dr = canvas[ci + 2] as u32;
            canvas[ci] = (bp + db * inv / 255).min(255) as u8;
            canvas[ci + 1] = (gp + dg * inv / 255).min(255) as u8;
            canvas[ci + 2] = (rp + dr * inv / 255).min(255) as u8;
            canvas[ci + 3] = 255;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(is_terminal: bool, is_pinned: bool) -> ContextMenuState {
        ContextMenuState {
            x: 0,
            y: 0,
            app_name: "Test".into(),
            exec: "test".into(),
            is_terminal,
            is_pinned,
            running_window_id: None,
            hover_idx: None,
        }
    }

    #[test]
    fn item_list_non_terminal_non_pinned_has_five_items() {
        let items = item_list(false, false, false);
        assert_eq!(items.len(), 5);
        assert!(matches!(items[0].1, ContextMenuAction::Launch));
        assert!(matches!(items[1].1, ContextMenuAction::NewWindow));
        assert!(matches!(items[2].1, ContextMenuAction::LaunchInTerminal));
        assert!(matches!(items[3].1, ContextMenuAction::PinToPanel));
        assert!(matches!(items[4].1, ContextMenuAction::RemoveFromLauncher));
    }

    #[test]
    fn item_list_terminal_pinned_has_four_items() {
        let items = item_list(true, true, false);
        assert_eq!(items.len(), 4);
        assert!(matches!(items[0].1, ContextMenuAction::Launch));
        assert!(matches!(items[1].1, ContextMenuAction::NewWindow));
        assert!(matches!(items[2].1, ContextMenuAction::UnpinFromPanel));
        assert!(matches!(items[3].1, ContextMenuAction::RemoveFromLauncher));
    }

    #[test]
    fn item_list_non_terminal_pinned_shows_unpin() {
        let items = item_list(false, true, false);
        assert_eq!(items.len(), 5);
        assert!(matches!(items[3].1, ContextMenuAction::UnpinFromPanel));
        assert!(matches!(items[4].1, ContextMenuAction::RemoveFromLauncher));
    }

    #[test]
    fn hit_item_above_menu_is_none() {
        let s = state(false, false);
        let items = item_list(false, false, false);
        assert!(hit_item(&s, items.len(), 50.0, -10.0).is_none());
    }

    #[test]
    fn hit_item_first_row() {
        let s = state(false, false);
        let items = item_list(false, false, false);
        let n = items.len();
        let mid_y = (VPAD + ITEM_H / 2) as f64;
        assert_eq!(hit_item(&s, n, 50.0, mid_y), Some(0));
    }

    #[test]
    fn hit_item_last_row() {
        let s = state(false, false);
        let items = item_list(false, false, false);
        let n = items.len();
        // last item is n-1, with 1px separator shift
        let last_top = VPAD + (n as i32 - 1) * ITEM_H + 1;
        let mid_y = (last_top + ITEM_H / 2) as f64;
        assert_eq!(hit_item(&s, n, 50.0, mid_y), Some(n - 1));
    }

    #[test]
    fn contains_point_outside_returns_false() {
        let s = ContextMenuState {
            x: 100,
            y: 100,
            ..state(false, false)
        };
        let items = item_list(false, false, false);
        assert!(!contains_point(&s, items.len(), 50.0, 50.0));
    }

    #[test]
    fn clamp_position_fits_inside_launcher() {
        let (x, y) = clamp_position(10, 10, 3, 880, 620);
        assert!(x >= 0 && x + MENU_WIDTH <= 880);
        assert!(y >= 0 && y + menu_height(3) <= 620);
    }

    #[test]
    fn clamp_position_right_edge_clamped() {
        let (x, _) = clamp_position(870, 10, 3, 880, 620);
        assert!(x + MENU_WIDTH <= 880);
    }

    #[test]
    fn clamp_position_bottom_edge_flips_up() {
        let n = 3;
        let (_, y) = clamp_position(10, 610, n, 880, 620);
        assert!(y + menu_height(n) <= 620);
    }

    #[test]
    fn draw_overlay_does_not_panic() {
        let s = state(false, false);
        let items = item_list(false, false, false);
        let mut canvas = vec![0u8; 880 * 620 * 4];
        draw_overlay(&mut canvas, 880, 620, &s, &items, &[], &[], &ThemeConfig::default());
    }

    #[test]
    fn draw_overlay_modifies_canvas_at_menu_location() {
        let s = state(false, false);
        let items = item_list(false, false, false);
        let mut canvas = vec![0u8; 880 * 620 * 4];
        draw_overlay(&mut canvas, 880, 620, &s, &items, &[], &[], &ThemeConfig::default());
        // At least some pixel in the menu area should be non-zero.
        let row_stride = 880 * 4;
        let menu_start = (s.y * row_stride + s.x * 4) as usize;
        assert!(canvas[menu_start..menu_start + MENU_WIDTH as usize * 4]
            .iter()
            .any(|b| *b != 0));
    }

    #[test]
    fn desktop_item_list_has_four_items_with_settings_last() {
        let items = desktop_item_list();
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].1, DesktopContextMenuAction::Terminal);
        assert_eq!(items[1].1, DesktopContextMenuAction::Launcher);
        assert_eq!(items[2].1, DesktopContextMenuAction::FileManager);
        assert_eq!(items[SETTINGS_ITEM_IDX].1, DesktopContextMenuAction::Settings);
    }

    #[test]
    fn submenu_items_has_expected_categories() {
        let items = submenu_items();
        assert!(items.iter().any(|(_, a)| *a == SettingsSubAction::Display));
        assert!(items.iter().any(|(_, a)| *a == SettingsSubAction::Wallpaper));
        assert!(items.iter().any(|(_, a)| *a == SettingsSubAction::Theme));
        assert!(items.iter().any(|(_, a)| *a == SettingsSubAction::Sound));
        assert!(items.iter().any(|(_, a)| *a == SettingsSubAction::Network));
        assert!(items.iter().any(|(_, a)| *a == SettingsSubAction::Power));
    }

    #[test]
    fn submenu_hit_item_local_returns_correct_index() {
        let sub_x = (MENU_WIDTH + SUBMENU_GAP + 10) as f64;
        let first_mid_y = (VPAD + ITEM_H / 2) as f64;
        assert_eq!(submenu_hit_item_local(sub_x, first_mid_y), Some(0));
        assert_eq!(submenu_hit_item_local(5.0, first_mid_y), None);
    }

    #[test]
    fn total_menu_width_grows_when_submenu_open() {
        assert_eq!(total_menu_width(false), MENU_WIDTH);
        assert!(total_menu_width(true) > MENU_WIDTH);
    }

    #[test]
    fn desktop_hit_item_uses_desktop_coordinates() {
        let state = DesktopContextMenuState {
            x: 40,
            y: 50,
            hover_idx: None,
            submenu_open: false,
            submenu_hover_idx: None,
        };
        assert_eq!(desktop_hit_item(&state, 55.0, 65.0), Some(0));
        assert_eq!(desktop_hit_item(&state, 5.0, 65.0), None);
    }
}
