use std::cell::RefCell;

use meridian_config::ThemeConfig;

use crate::{
    ui::{
        primitives::{
            draw_panel_button, draw_workspace_button, fill_surface, subtle_border,
            InteractiveState, SurfaceKind,
        },
        tokens,
    },
    ClickAction, ClickZone, Painter, Rect, TextRenderer, PANEL_HEIGHT,
};

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

    let height = PANEL_HEIGHT as i32;
    let panel_card = Rect {
        x: tokens::spacing::XS,
        y: 2,
        w: width as i32 - tokens::spacing::XS * 2,
        h: height - 4,
    };
    fill_surface(painter, panel_card, theme, SurfaceKind::Background);
    subtle_border(painter, panel_card, theme);

    let mut x = panel_card.x + tokens::panel::LEFT_PADDING;
    let controls_y = panel_card.y + tokens::panel::WORKSPACE_BUTTON_Y - 2;
    let controls_x = x;
    let controls_w = tokens::panel::LAUNCHER_BUTTON_W
        + tokens::panel::WORKSPACE_BUTTON_GAP
        + (tokens::panel::WORKSPACE_BUTTON_W + tokens::panel::WORKSPACE_BUTTON_GAP) * 9;
    let controls_surface = Rect {
        x: controls_x - 2,
        y: controls_y - 2,
        w: controls_w + 4,
        h: tokens::panel::WORKSPACE_BUTTON_H + 4,
    };
    fill_surface(painter, controls_surface, theme, SurfaceKind::Surface);
    subtle_border(painter, controls_surface, theme);

    // ── Left: Launcher button ───────────────────────────────────────────────
    let launcher_rect = Rect {
        x,
        y: controls_y,
        w: tokens::panel::LAUNCHER_BUTTON_W,
        h: tokens::panel::WORKSPACE_BUTTON_H,
    };
    let launcher_text = draw_panel_button(painter, launcher_rect, theme, InteractiveState::Default);
    painter.text_centered(font, "Launcher", launcher_rect, launcher_text);
    panel_state.clicks.push(ClickZone {
        rect: launcher_rect,
        action: ClickAction::ToggleLauncher,
    });
    x += tokens::panel::LAUNCHER_BUTTON_W + tokens::panel::WORKSPACE_BUTTON_GAP;

    // ── Left: Workspace buttons ─────────────────────────────────────────────
    for ws in 1u8..=9 {
        let ws_idx = (ws - 1) as usize;
        let is_active = ws == active_workspace;
        let is_occupied = occupied_workspaces
            .map(|occupied| occupied[ws_idx])
            .unwrap_or(false);

        let rect = Rect {
            x,
            y: controls_y,
            w: tokens::panel::WORKSPACE_BUTTON_W,
            h: tokens::panel::WORKSPACE_BUTTON_H,
        };
        let text_color = draw_workspace_button(painter, rect, theme, is_active, is_occupied);
        painter.text_centered(font, &ws.to_string(), rect, text_color);
        panel_state.clicks.push(ClickZone {
            rect,
            action: ClickAction::SwitchWorkspace(ws),
        });
        x += tokens::panel::WORKSPACE_BUTTON_W + tokens::panel::WORKSPACE_BUTTON_GAP;
    }

    // ── Right: Clock ────────────────────────────────────────────────────────
    let clock_surface = Rect {
        x: width as i32 - tokens::panel::CLOCK_W - tokens::panel::RIGHT_PADDING - 4,
        y: panel_card.y + (panel_card.h - 24) / 2,
        w: tokens::panel::CLOCK_W + 8,
        h: 24,
    };
    fill_surface(painter, clock_surface, theme, SurfaceKind::Surface);
    subtle_border(painter, clock_surface, theme);
    let clock_rect = Rect {
        x: width as i32 - tokens::panel::CLOCK_W - tokens::panel::RIGHT_PADDING,
        y: (height - 20) / 2,
        w: tokens::panel::CLOCK_W,
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
