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
fn menu_radius(theme_config: &ThemeConfig) -> i32 {
    theme_config
        .decorations
        .surface_radius(meridian_config::ThemeSurface::Popup)
        .round() as i32
}

fn is_glass_menu(theme_config: &ThemeConfig) -> bool {
    theme_config.decorations.glass && theme_config.decorations.glass_blur
}

fn menu_palette_from_config(theme_config: &ThemeConfig) -> meridian_ui::style::Palette {
    palette_from_config(theme_config)
}

fn ui_color(color: meridian_config::Color) -> meridian_ui::style::Color {
    meridian_ui::style::Color::rgba(color.r, color.g, color.b, color.a)
}

fn theme_accent_idle(theme_config: &ThemeConfig) -> meridian_ui::style::Color {
    ui_color(meridian_tokens::Interaction::DEFAULT.accent_idle(theme_config.colors.accent))
}

fn theme_accent_hover(theme_config: &ThemeConfig) -> meridian_ui::style::Color {
    ui_color(meridian_tokens::Interaction::DEFAULT.accent_hover(theme_config.colors.accent))
}

fn paint_menu_background(
    canvas: &mut tiny_skia::PixmapMut<'_>,
    path: &tiny_skia::Path,
    theme_config: &ThemeConfig,
    pal: meridian_ui::style::Palette,
) {
    if !is_glass_menu(theme_config) {
        paint_fill(canvas, path, pal.surface_alt);
    }
    paint_border(canvas, path, pal.border, 1.0);
}

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
    LockScreen,
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
        ("Bildschirm sperren", DesktopContextMenuAction::LockScreen),
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

/// True when `px` falls inside the settings flyout column (surface-local
/// coords) — this is the click hit-test (gap excluded; clicks in the gap
/// must not trigger flyout items).
pub(crate) fn is_in_submenu_area(px: f64) -> bool {
    let lx = px as i32;
    (MENU_WIDTH + SUBMENU_GAP..MENU_WIDTH + SUBMENU_GAP + SUBMENU_WIDTH).contains(&lx)
}

/// True while the cursor is anywhere right of the main menu and not past
/// the flyout — including the small visual gap between them. Used by the
/// hover state so a cursor traversing main-menu → gap → flyout doesn't
/// momentarily lose the submenu hover (closing it + shrinking the surface).
pub(crate) fn is_in_submenu_hover_region(px: f64) -> bool {
    let lx = px as i32;
    (MENU_WIDTH..MENU_WIDTH + SUBMENU_GAP + SUBMENU_WIDTH).contains(&lx)
}

/// Returns the 0-based flyout item index under surface-local `(px, py)`, or `None`.
pub(crate) fn submenu_hit_item_local(px: f64, py: f64) -> Option<usize> {
    let lx = px as i32 - MENU_WIDTH - SUBMENU_GAP;
    let ly = py as i32;
    let n = submenu_items().len();
    if !(0..SUBMENU_WIDTH).contains(&lx) {
        return None;
    }
    if ly < VPAD || ly >= VPAD + n as i32 * ITEM_H {
        return None;
    }
    let i = ((ly - VPAD) / ITEM_H) as usize;
    if i < n {
        Some(i)
    } else {
        None
    }
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
    Lock,
}

fn icon_for_desktop(action: DesktopContextMenuAction) -> MenuIcon {
    match action {
        DesktopContextMenuAction::Terminal => MenuIcon::Terminal,
        DesktopContextMenuAction::Launcher => MenuIcon::Launcher,
        DesktopContextMenuAction::FileManager => MenuIcon::FileManager,
        DesktopContextMenuAction::Settings => MenuIcon::Settings,
        DesktopContextMenuAction::LockScreen => MenuIcon::Lock,
    }
}

// Draw a 16-unit-viewbox line-art icon at (`ox`,`oy`) scaled to `sz` px.

include!("context_menu/icons.rs");
include!("context_menu/overlays.rs");

#[cfg(test)]
#[path = "context_menu_tests.rs"]
mod tests;
