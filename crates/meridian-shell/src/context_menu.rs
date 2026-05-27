use meridian_config::ThemeConfig;
use meridian_ui::{
    effect::{paint_border, paint_fill, paint_text, rounded_rect_path},
    paint::Rect,
};
use tiny_skia::Pixmap;

use crate::ui::tokens::palette_from_config;

pub(crate) const MENU_WIDTH: i32 = 236;
const ICON_SZ: f32 = 16.0;
const ICON_GAP: i32 = 12;
const ITEM_H: i32 = 36;
const VPAD: i32 = 6;
const PADDING_X: i32 = 14;
const FONT_SIZE: f32 = 13.0;
const CORNER_R: i32 = 10;

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
    DisplaySettings,
    WallpaperSettings,
    Settings,
}

pub(crate) struct DesktopContextMenuState {
    /// Menu top-left in desktop-surface pixels.
    pub x: i32,
    pub y: i32,
    pub hover_idx: Option<usize>,
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
        (
            "Display-Einstellungen",
            DesktopContextMenuAction::DisplaySettings,
        ),
        (
            "Wallpaper-Einstellungen",
            DesktopContextMenuAction::WallpaperSettings,
        ),
        ("Einstellungen", DesktopContextMenuAction::Settings),
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
    };
    desktop_hit_item(&state, px, py)
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
    Display,
    Wallpaper,
    Settings,
}

fn icon_for_desktop(action: DesktopContextMenuAction) -> MenuIcon {
    match action {
        DesktopContextMenuAction::Terminal => MenuIcon::Terminal,
        DesktopContextMenuAction::Launcher => MenuIcon::Launcher,
        DesktopContextMenuAction::DisplaySettings => MenuIcon::Display,
        DesktopContextMenuAction::WallpaperSettings => MenuIcon::Wallpaper,
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
        MenuIcon::Display => {
            let mut pb = PathBuilder::new();
            let (l, t) = m(2.5, 3.5);
            let (r, b) = m(13.5, 10.5);
            pb.move_to(l, t);
            pb.line_to(r, t);
            pb.line_to(r, b);
            pb.line_to(l, b);
            pb.close();
            stroke_pb(canvas, pb);
            let mut pb2 = PathBuilder::new();
            let (sx, sy0) = m(8.0, 10.5);
            let (_, sy1) = m(8.0, 12.8);
            pb2.move_to(sx, sy0);
            pb2.line_to(sx, sy1);
            let (b0, by) = m(5.5, 12.8);
            let (b1, _) = m(10.5, 12.8);
            pb2.move_to(b0, by);
            pb2.line_to(b1, by);
            stroke_pb(canvas, pb2);
        }
        MenuIcon::Wallpaper => {
            let mut pb = PathBuilder::new();
            let (l, t) = m(2.5, 3.5);
            let (r, b) = m(13.5, 12.5);
            pb.move_to(l, t);
            pb.line_to(r, t);
            pb.line_to(r, b);
            pb.line_to(l, b);
            pb.close();
            stroke_pb(canvas, pb);
            let mut pb2 = PathBuilder::new();
            let (cx, cy) = m(6.0, 6.3);
            pb2.push_circle(cx, cy, sz / 16.0 * 1.3);
            stroke_pb(canvas, pb2);
            let mut pb3 = PathBuilder::new();
            let (a, c) = m(3.5, 11.8);
            let (d, e) = m(7.2, 7.5);
            let (f, g) = m(13.0, 11.8);
            pb3.move_to(a, c);
            pb3.line_to(d, e);
            pb3.line_to(f, g);
            stroke_pb(canvas, pb3);
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

/// Render the context menu as an overlay onto the existing BGRA `canvas`.
/// `icons` is parallel to `items` (empty = no icon column).
pub(crate) fn draw_overlay(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    state: &ContextMenuState,
    items: &[(&str, ContextMenuAction)],
    icons: &[MenuIcon],
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
            // Subtle steel-blue accent wash + a left accent marker, matching the
            // window-control cluster and the rest of the redesigned chrome.
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

pub(crate) fn draw_desktop_overlay(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    state: &DesktopContextMenuState,
    items: &[(&str, DesktopContextMenuAction)],
    theme_config: &ThemeConfig,
) {
    let app_items: Vec<(&str, ContextMenuAction)> = items
        .iter()
        .map(|(label, _)| (*label, ContextMenuAction::Launch))
        .collect();
    let icons: Vec<MenuIcon> = items
        .iter()
        .map(|(_, action)| icon_for_desktop(*action))
        .collect();
    let local_state = ContextMenuState {
        x: state.x,
        y: state.y,
        app_name: "Desktop".into(),
        exec: "".into(),
        is_terminal: false,
        is_pinned: false,
        running_window_id: None,
        hover_idx: state.hover_idx,
    };
    draw_overlay(
        canvas,
        canvas_w,
        canvas_h,
        &local_state,
        &app_items,
        &icons,
        theme_config,
    );
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
        draw_overlay(&mut canvas, 880, 620, &s, &items, &[], &ThemeConfig::default());
    }

    #[test]
    fn draw_overlay_modifies_canvas_at_menu_location() {
        let s = state(false, false);
        let items = item_list(false, false, false);
        let mut canvas = vec![0u8; 880 * 620 * 4];
        draw_overlay(&mut canvas, 880, 620, &s, &items, &[], &ThemeConfig::default());
        // At least some pixel in the menu area should be non-zero.
        let row_stride = 880 * 4;
        let menu_start = (s.y * row_stride + s.x * 4) as usize;
        assert!(canvas[menu_start..menu_start + MENU_WIDTH as usize * 4]
            .iter()
            .any(|b| *b != 0));
    }
    #[test]
    fn desktop_item_list_contains_core_actions() {
        let items = desktop_item_list();
        assert_eq!(items[0].1, DesktopContextMenuAction::Terminal);
        assert!(items
            .iter()
            .any(|(_, action)| *action == DesktopContextMenuAction::DisplaySettings));
        assert!(items
            .iter()
            .any(|(_, action)| *action == DesktopContextMenuAction::WallpaperSettings));
    }

    #[test]
    fn desktop_hit_item_uses_desktop_coordinates() {
        let state = DesktopContextMenuState {
            x: 40,
            y: 50,
            hover_idx: None,
        };
        assert_eq!(desktop_hit_item(&state, 55.0, 65.0), Some(0));
        assert_eq!(desktop_hit_item(&state, 5.0, 65.0), None);
    }
}
