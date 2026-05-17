use std::cell::RefCell;

use meridian_config::ThemeConfig;

use crate::{
    ui::{
        primitives::{
            draw_active_indicator, draw_panel_button, draw_section_separator,
            draw_workspace_button, ActiveIndicatorEdge, InteractiveState,
        },
        tokens,
    },
    ClickAction, ClickZone, Painter, Rect, TextRenderer, PANEL_HEIGHT,
};

pub struct PanelState {
    pub clicks: Vec<ClickZone>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PanelWindowEntry {
    pub id: String,
    pub title: String,
    pub focused: bool,
    pub minimized: bool,
}

pub struct PanelDrawInput<'a> {
    pub font: &'a RefCell<Option<TextRenderer>>,
    pub theme: &'a ThemeConfig,
    pub active_workspace: u8,
    pub occupied_workspaces: Option<&'a [bool; 9]>,
    pub window_entries: &'a [PanelWindowEntry],
    pub clock: &'a str,
    pub width: u32,
    pub hover_pos: Option<(f64, f64)>,
}

impl PanelState {
    pub fn new() -> Self {
        Self { clicks: Vec::new() }
    }
}

pub fn draw_panel(
    panel_state: &mut PanelState,
    painter: &mut Painter<'_>,
    input: PanelDrawInput<'_>,
) {
    let PanelDrawInput {
        font,
        theme,
        active_workspace,
        occupied_workspaces,
        window_entries,
        clock,
        width,
        hover_pos,
    } = input;
    let colors = &theme.colors;
    painter.clear(colors.surface_alt);
    painter.rect(
        Rect {
            x: 0,
            y: 0,
            w: width as i32,
            h: 1,
        },
        colors.border,
    );
    panel_state.clicks.clear();

    let height = PANEL_HEIGHT as i32;
    let panel_card = Rect {
        x: 0,
        y: 0,
        w: width as i32,
        h: height,
    };

    let mut x = panel_card.x + tokens::panel::LEFT_PADDING;
    let controls_y = panel_card.y + tokens::panel::WORKSPACE_BUTTON_Y;

    // ── Left: Launcher button ───────────────────────────────────────────────
    let launcher_rect = Rect {
        x,
        y: controls_y,
        w: tokens::panel::LAUNCHER_BUTTON_W,
        h: tokens::panel::WORKSPACE_BUTTON_H,
    };
    let launcher_hovered = hover_pos
        .map(|(px, py)| launcher_rect.contains(px, py))
        .unwrap_or(false);
    let launcher_text = draw_panel_button(
        painter,
        launcher_rect,
        theme,
        InteractiveState::Default,
        launcher_hovered,
    );
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
        let is_hovered = hover_pos
            .map(|(px, py)| rect.contains(px, py))
            .unwrap_or(false);
        let text_color =
            draw_workspace_button(painter, rect, theme, is_active, is_occupied, is_hovered);
        painter.text_centered(font, &ws.to_string(), rect, text_color);
        panel_state.clicks.push(ClickZone {
            rect,
            action: ClickAction::SwitchWorkspace(ws),
        });
        x += tokens::panel::WORKSPACE_BUTTON_W + tokens::panel::WORKSPACE_BUTTON_GAP;
    }

    let workspace_group_end_x = x - tokens::panel::WORKSPACE_BUTTON_GAP;

    // ── Right: Clock ────────────────────────────────────────────────────────
    let clock_h = tokens::panel::WORKSPACE_BUTTON_H;
    let clock_y = tokens::panel::WORKSPACE_BUTTON_Y;
    let clock_rect = Rect {
        x: width as i32 - tokens::panel::CLOCK_W - tokens::panel::RIGHT_PADDING,
        y: clock_y,
        w: tokens::panel::CLOCK_W,
        h: clock_h,
    };
    let clock_hovered = hover_pos
        .map(|(px, py)| clock_rect.contains(px, py))
        .unwrap_or(false);
    let clock_bg = if clock_hovered {
        colors.border
    } else {
        colors.surface
    };
    painter.roundish_rect_with_radius(clock_rect, clock_bg, tokens::panel::CLOCK_RADIUS);
    painter.text_right_aligned(font, clock, clock_rect, colors.text);
    panel_state.clicks.push(ClickZone {
        rect: clock_rect,
        action: ClickAction::Clock,
    });

    // ── Center: Read-only workspace window list ────────────────────────────
    let center_left = x + 12;
    let center_right = clock_rect.x - 12;
    let center_w = center_right - center_left;
    draw_section_separator(
        painter,
        workspace_group_end_x + 8,
        panel_card.y,
        panel_card.h,
        theme,
    );
    draw_section_separator(painter, clock_rect.x - 8, panel_card.y, panel_card.h, theme);

    if center_w > 40 && !window_entries.is_empty() {
        let center_rect = Rect {
            x: center_left,
            y: (height - 20) / 2,
            w: center_w,
            h: 20,
        };
        let mut text_x = center_rect.x;
        let right = center_rect.x + center_rect.w;

        for (idx, entry) in window_entries.iter().enumerate() {
            if text_x >= right {
                break;
            }

            let mut label = String::new();
            label.push_str(&entry.title);
            let prefix_w = if idx > 0 { 10 } else { 0 };
            let text_w = (label.chars().count() as i32 * 8).max(0);
            let tile_w = (prefix_w + text_w + 14).min((right - text_x).max(0));
            if tile_w <= 0 {
                break;
            }
            let tile_rect = Rect {
                x: text_x,
                y: center_rect.y,
                w: tile_w,
                h: center_rect.h,
            };
            let tile_hovered = hover_pos
                .map(|(px, py)| tile_rect.contains(px, py))
                .unwrap_or(false);
            let tile_bg = if tile_hovered {
                colors.border
            } else {
                colors.surface
            };
            painter.roundish_rect_with_radius(tile_rect, tile_bg, tokens::panel::BUTTON_RADIUS);

            let color = if entry.focused {
                colors.accent
            } else if entry.minimized {
                colors.border
            } else {
                colors.text
            };
            if entry.focused {
                draw_active_indicator(painter, tile_rect, ActiveIndicatorEdge::Bottom, theme);
            }
            let text_x_start = tile_rect.x + 7 + prefix_w;
            let baseline = tile_rect.y + (tile_rect.h / 2) + 5;
            painter.text_clipped(
                font,
                &label,
                text_x_start,
                baseline,
                (tile_rect.w - 12 - prefix_w).max(0),
                color,
            );
            panel_state.clicks.push(ClickZone {
                rect: tile_rect,
                action: ClickAction::FocusWindow(entry.id.clone()),
            });
            text_x += tile_w + 6;
        }
    }
}
