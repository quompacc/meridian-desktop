use meridian_ui::{
    effect::{paint_border, paint_fill, paint_text, rounded_rect_path},
    paint::Rect,
    style::{Color, Palette},
};
use tiny_skia::Pixmap;

pub(crate) const MENU_WIDTH: i32 = 200;
const ITEM_H: i32 = 36;
const VPAD: i32 = 6;
const PADDING_X: i32 = 14;
const FONT_SIZE: f32 = 13.0;
const CORNER_R: i32 = 6;

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

/// Build the item list from the current state flags.
pub(crate) fn item_list(is_terminal: bool, is_pinned: bool, is_running: bool) -> Vec<(&'static str, ContextMenuAction)> {
    let mut items: Vec<(&str, ContextMenuAction)> = Vec::new();
    items.push(if is_running { ("Fokussieren", ContextMenuAction::Launch) } else { ("Starten", ContextMenuAction::Launch) });
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
    ix >= state.x
        && ix < state.x + MENU_WIDTH
        && iy >= state.y
        && iy < state.y + mh
}

/// Returns the 0-based item index under `(px, py)`, or `None`.
///
/// The visual separator sits between items[n-2] and items[n-1] (the pin action
/// is always last). It shifts the last item down by 1px.
pub(crate) fn hit_item(state: &ContextMenuState, n: usize, px: f64, py: f64) -> Option<usize> {
    if !contains_point(state, n, px, py) {
        return None;
    }
    let sep_before = n.saturating_sub(1);
    for i in 0..n {
        let extra = if i >= sep_before { 1 } else { 0 };
        let top = state.y + VPAD + i as i32 * ITEM_H + extra;
        let bot = top + ITEM_H;
        let iy = py as i32;
        if iy >= top && iy < bot {
            return Some(i);
        }
    }
    None
}

/// Render the context menu as an overlay onto the existing BGRA `canvas`.
pub(crate) fn draw_overlay(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    state: &ContextMenuState,
    items: &[(&str, ContextMenuAction)],
) {
    let n = items.len();
    let mw = MENU_WIDTH as u32;
    let mh = menu_height(n) as u32;
    let Some(mut pm) = Pixmap::new(mw, mh) else {
        return;
    };
    let pal = Palette::TOKYO_NIGHT_METRO;

    // Background
    let bg_rect = Rect { x: 0, y: 0, width: mw as i32, height: mh as i32 };
    let Some(bg_path) = rounded_rect_path(bg_rect, CORNER_R) else {
        return;
    };
    paint_fill(&mut pm.as_mut(), &bg_path, pal.surface_alt);
    paint_border(&mut pm.as_mut(), &bg_path, pal.border, 1.0);

    // Separator before last item
    let sep_before = n.saturating_sub(1);
    let sep_y = VPAD + sep_before as i32 * ITEM_H;
    let sep_rect = Rect { x: 8, y: sep_y, width: mw as i32 - 16, height: 1 };
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
            if let Some(p) = rounded_rect_path(item_rect, 4) {
                paint_fill(
                    &mut pm.as_mut(),
                    &p,
                    pal.surface.lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.12),
                );
            }
        }

        let text_y = item_top + ITEM_H - 10;
        paint_text(&mut pm.as_mut(), label, PADDING_X, text_y, FONT_SIZE, pal.text);
    }

    blit_over(canvas, canvas_w as i32, canvas_h as i32, &pm, state.x, state.y);
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
        let s = ContextMenuState { x: 100, y: 100, ..state(false, false) };
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
        draw_overlay(&mut canvas, 880, 620, &s, &items);
    }

    #[test]
    fn draw_overlay_modifies_canvas_at_menu_location() {
        let s = state(false, false);
        let items = item_list(false, false, false);
        let mut canvas = vec![0u8; 880 * 620 * 4];
        draw_overlay(&mut canvas, 880, 620, &s, &items);
        // At least some pixel in the menu area should be non-zero.
        let row_stride = 880 * 4;
        let menu_start = (s.y * row_stride + s.x * 4) as usize;
        assert!(
            canvas[menu_start..menu_start + MENU_WIDTH as usize * 4]
                .iter()
                .any(|b| *b != 0)
        );
    }
}
