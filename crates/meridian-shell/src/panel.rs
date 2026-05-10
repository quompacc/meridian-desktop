use std::cell::RefCell;

use meridian_config::{Color, ThemeConfig};

use crate::{ClickAction, ClickZone, Painter, Rect, TextRenderer, PANEL_HEIGHT};

const WS_BTN_W: i32 = 28;
const WS_BTN_H: i32 = 28;
const WS_BTN_Y: i32 = 4;
const WS_GAP: i32 = 4;
const LEFT_PAD: i32 = 8;
const LAUNCHER_BTN_W: i32 = 58;
const CLOCK_W: i32 = 170;
const RIGHT_PAD: i32 = 10;

pub struct PanelState {
    pub clicks: Vec<ClickZone>,
}

impl PanelState {
    pub fn new() -> Self {
        Self { clicks: Vec::new() }
    }
}

pub fn draw_panel(
    panel_state: &mut PanelState,
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    active_workspace: u8,
    occupied_workspaces: Option<&[bool; 9]>,
    focused_title: Option<&str>,
    clock: &str,
    width: u32,
) {
    let colors = &theme.colors;
    painter.clear(colors.surface);
    panel_state.clicks.clear();

    let mut x = LEFT_PAD;
    let height = PANEL_HEIGHT as i32;

    // ── Left: Launcher button ───────────────────────────────────────────────
    let launcher_rect = Rect {
        x,
        y: WS_BTN_Y,
        w: LAUNCHER_BTN_W,
        h: WS_BTN_H,
    };
    painter.roundish_rect(launcher_rect, colors.background);
    painter.text_centered(font, "Launcher", launcher_rect, colors.text);
    panel_state.clicks.push(ClickZone {
        rect: launcher_rect,
        action: ClickAction::ToggleLauncher,
    });
    x += LAUNCHER_BTN_W + WS_GAP;

    // ── Left: Workspace buttons ─────────────────────────────────────────────
    for ws in 1u8..=9 {
        let ws_idx = (ws - 1) as usize;
        let is_active = ws == active_workspace;
        let is_occupied = occupied_workspaces
            .map(|occupied| occupied[ws_idx])
            .unwrap_or(false);

        let rect = Rect {
            x,
            y: WS_BTN_Y,
            w: WS_BTN_W,
            h: WS_BTN_H,
        };
        let bg = if is_active {
            colors.accent
        } else if is_occupied {
            colors.border
        } else {
            colors.background
        };
        painter.roundish_rect(rect, bg);

        let text_color = if is_active {
            Color::rgb(0x1e, 0x1e, 0x2e)
        } else {
            colors.text
        };
        painter.text_centered(font, &ws.to_string(), rect, text_color);
        panel_state.clicks.push(ClickZone {
            rect,
            action: ClickAction::SwitchWorkspace(ws),
        });
        x += WS_BTN_W + WS_GAP;
    }

    // ── Right: Clock ────────────────────────────────────────────────────────
    let clock_rect = Rect {
        x: width as i32 - CLOCK_W - RIGHT_PAD,
        y: (height - 20) / 2,
        w: CLOCK_W,
        h: 20,
    };
    painter.text_centered(font, clock, clock_rect, colors.text);

    // ── Center: Focused window title ────────────────────────────────────────
    let center_left = x + 12;
    let center_right = clock_rect.x - 12;
    let center_w = center_right - center_left;
    if center_w > 40 {
        if let Some(title) = focused_title {
            let center_rect = Rect {
                x: center_left,
                y: (height - 20) / 2,
                w: center_w,
                h: 20,
            };
            painter.text_centered(font, title, center_rect, colors.text);
        }
    }
}
