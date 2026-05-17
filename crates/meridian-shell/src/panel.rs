use std::cell::RefCell;

use meridian_config::ThemeConfig;

use crate::{
    ui::{
        primitives::{
            draw_active_indicator, draw_panel_button, draw_section_separator, ActiveIndicatorEdge,
            InteractiveState,
        },
        tokens,
    },
    ClickAction, ClickZone, Painter, Rect, TextRenderer, PANEL_HEIGHT,
};

pub struct PanelState {
    pub clicks: Vec<ClickZone>,
}

#[derive(Debug, Clone)]
pub struct PinnedApp {
    pub label: String,
    pub program: String,
    pub args: Vec<String>,
    pub terminal: bool,
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
    pub total_workspaces: u8,
    pub pinned_apps: &'a [PinnedApp],
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
        total_workspaces,
        pinned_apps,
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

    let x = panel_card.x + tokens::panel::LEFT_PADDING;
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
    let launcher_x_end = launcher_rect.x + launcher_rect.w;

    // ── Left: Pinned apps ───────────────────────────────────────────────────
    let mut pinned_x = launcher_x_end + 12;
    for (idx, app) in pinned_apps.iter().enumerate() {
        let rect = Rect {
            x: pinned_x,
            y: tokens::panel::WORKSPACE_BUTTON_Y,
            w: tokens::panel::PINNED_TILE_W,
            h: tokens::panel::WORKSPACE_BUTTON_H,
        };
        let hovered = hover_pos
            .map(|(px, py)| rect.contains(px, py))
            .unwrap_or(false);
        let bg = if hovered {
            colors.border
        } else {
            colors.surface
        };
        painter.roundish_rect_with_radius(rect, bg, 0);
        painter.text_centered(font, &app.label, rect, colors.text);
        panel_state.clicks.push(ClickZone {
            rect,
            action: ClickAction::LaunchPinnedApp(idx),
        });
        pinned_x += tokens::panel::PINNED_TILE_W + tokens::panel::PINNED_TILE_GAP;
    }

    let launcher_sep_x = launcher_x_end + 8;
    draw_section_separator(painter, launcher_sep_x, panel_card.y, panel_card.h, theme);

    let center_left = if pinned_apps.is_empty() {
        launcher_x_end + 12
    } else {
        let sep_x = pinned_x - tokens::panel::PINNED_TILE_GAP + 4;
        draw_section_separator(painter, sep_x, panel_card.y, panel_card.h, theme);
        sep_x + tokens::panel::PINNED_SECTION_GAP
    };

    // ── Right: Clock / Workspace indicator / Tray slot ─────────────────────
    let clock_measured = font
        .borrow_mut()
        .as_mut()
        .map(|renderer| renderer.measure_text(clock))
        .unwrap_or_else(|| clock.chars().count() as i32 * 7);
    let clock_w = clock_measured + 2 * tokens::panel::CLOCK_PADDING_H;
    let clock_rect = Rect {
        x: width as i32 - clock_w - tokens::panel::RIGHT_PADDING,
        y: tokens::panel::WORKSPACE_BUTTON_Y,
        w: clock_w,
        h: tokens::panel::WORKSPACE_BUTTON_H,
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
    painter.text_centered(font, clock, clock_rect, colors.text);
    panel_state.clicks.push(ClickZone {
        rect: clock_rect,
        action: ClickAction::Clock,
    });

    let sep_x_after_clock = clock_rect.x - 8;
    draw_section_separator(
        painter,
        sep_x_after_clock,
        panel_card.y,
        panel_card.h,
        theme,
    );

    let ws_ind_rect = Rect {
        x: sep_x_after_clock - 8 - tokens::panel::WORKSPACE_IND_W,
        y: tokens::panel::WORKSPACE_BUTTON_Y,
        w: tokens::panel::WORKSPACE_IND_W,
        h: tokens::panel::WORKSPACE_BUTTON_H,
    };
    let ws_hovered = hover_pos
        .map(|(px, py)| ws_ind_rect.contains(px, py))
        .unwrap_or(false);
    let ws_bg = if ws_hovered {
        colors.border
    } else {
        colors.surface
    };
    painter.roundish_rect_with_radius(ws_ind_rect, ws_bg, 0);
    let ws_text = format!("{}/{}", active_workspace, total_workspaces.max(1));
    painter.text_centered(font, &ws_text, ws_ind_rect, colors.text);
    panel_state.clicks.push(ClickZone {
        rect: ws_ind_rect,
        action: ClickAction::ToggleWorkspacePopup,
    });

    let sep_x_after_ws = ws_ind_rect.x - 8;
    draw_section_separator(painter, sep_x_after_ws, panel_card.y, panel_card.h, theme);

    let tray_slot_rect = Rect {
        x: sep_x_after_ws - 8 - tokens::panel::TRAY_SLOT_W,
        y: tokens::panel::WORKSPACE_BUTTON_Y,
        w: tokens::panel::TRAY_SLOT_W,
        h: tokens::panel::WORKSPACE_BUTTON_H,
    };

    // ── Center: Read-only workspace window list ────────────────────────────
    let center_right = tray_slot_rect.x - 8;
    let center_w = center_right - center_left;

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
