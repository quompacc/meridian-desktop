use std::cell::RefCell;

use meridian_config::ThemeConfig;

use crate::{
    popup_card::{draw_card_body, draw_card_title, BODY_TOP, PAD_BOTTOM, PAD_X},
    ui::primitives::{draw_active_indicator, ActiveIndicatorEdge},
    ClickAction, ClickZone, Painter, Rect, TextRenderer, WORKSPACE_POPUP_HEIGHT,
    WORKSPACE_POPUP_WIDTH,
};

const GAP: i32 = 6;
const TILE_RADIUS: i32 = 6;

pub struct WorkspacePopupState {
    pub clicks: Vec<ClickZone>,
}

impl WorkspacePopupState {
    pub fn new() -> Self {
        Self { clicks: Vec::new() }
    }
}

pub struct WorkspacePopupInput {
    pub active_workspace: u32,
    pub total_workspaces: u32,
    pub occupied: [bool; 9],
    pub hovered_idx: Option<usize>,
}

fn grid_geometry() -> (i32, i32, i32, i32) {
    let width = WORKSPACE_POPUP_WIDTH as i32;
    let height = WORKSPACE_POPUP_HEIGHT as i32;
    let grid_left = PAD_X;
    let grid_top = BODY_TOP;
    let grid_w = width - 2 * PAD_X;
    let grid_bottom = height - PAD_BOTTOM;
    let grid_h = grid_bottom - grid_top;
    let tile_w = (grid_w - 2 * GAP) / 3;
    let tile_h = (grid_h - 2 * GAP) / 3;
    (grid_left, grid_top, tile_w, tile_h)
}

pub fn workspace_popup_hover_idx(x: f64, y: f64) -> Option<usize> {
    let (left, top, tile_w, tile_h) = grid_geometry();
    for i in 0_usize..9 {
        let col = (i % 3) as i32;
        let row = (i / 3) as i32;
        let rect = Rect {
            x: left + col * (tile_w + GAP),
            y: top + row * (tile_h + GAP),
            w: tile_w,
            h: tile_h,
        };
        if rect.contains(x, y) {
            return Some(i);
        }
    }
    None
}

pub fn draw_workspace_popup(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    input: WorkspacePopupInput,
    state: &mut WorkspacePopupState,
) {
    state.clicks.clear();
    let colors = &theme.colors;
    let total_workspaces = input.total_workspaces.max(1);

    draw_card_body(painter, theme);
    draw_card_title(painter, font, theme, "Arbeitsbereiche");

    let (left, top, tile_w, tile_h) = grid_geometry();

    for i in 0_usize..9 {
        let ws_id = (i + 1) as u32;
        let col = (i % 3) as i32;
        let row = (i / 3) as i32;
        let rect = Rect {
            x: left + col * (tile_w + GAP),
            y: top + row * (tile_h + GAP),
            w: tile_w,
            h: tile_h,
        };

        let is_active = ws_id == input.active_workspace;
        let is_occupied = input.occupied[i];
        let is_hovered = input.hovered_idx == Some(i);

        let bg = if is_active {
            colors.surface
        } else if is_hovered {
            colors.border
        } else if is_occupied {
            colors.surface
        } else {
            colors.surface_alt
        };
        painter.roundish_rect_with_radius(rect, bg, TILE_RADIUS);

        if is_active {
            draw_active_indicator(painter, rect, ActiveIndicatorEdge::Top, theme);
        }

        let text_color = if is_active {
            colors.text
        } else if is_occupied {
            colors.accent
        } else {
            colors.text_dim
        };
        painter.text_centered(font, &ws_id.to_string(), rect, text_color);
        state.clicks.push(ClickZone {
            id: Some(format!("workspace-popup-{ws_id}")),
            rect,
            action: ClickAction::SwitchWorkspace(ws_id.clamp(1, total_workspaces) as u8),
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::ClickAction;

    use super::{
        draw_workspace_popup, workspace_popup_hover_idx, WorkspacePopupInput, WorkspacePopupState,
    };

    #[test]
    fn workspace_popup_generates_nine_switch_click_zones() {
        let mut surface =
            vec![0_u8; (crate::WORKSPACE_POPUP_WIDTH * crate::WORKSPACE_POPUP_HEIGHT * 4) as usize];
        let mut painter = crate::Painter::new(
            &mut surface,
            crate::WORKSPACE_POPUP_WIDTH as i32,
            crate::WORKSPACE_POPUP_HEIGHT as i32,
        );
        let mut state = WorkspacePopupState::new();
        let theme = meridian_config::ThemeConfig::default();
        let font = std::cell::RefCell::new(None);

        draw_workspace_popup(
            &mut painter,
            &font,
            &theme,
            WorkspacePopupInput {
                active_workspace: 3,
                total_workspaces: 9,
                occupied: [false; 9],
                hovered_idx: None,
            },
            &mut state,
        );

        assert_eq!(state.clicks.len(), 9);
        assert!(matches!(
            state.clicks[0].action,
            ClickAction::SwitchWorkspace(1)
        ));
        assert!(matches!(
            state.clicks[8].action,
            ClickAction::SwitchWorkspace(9)
        ));
    }

    #[test]
    fn workspace_popup_hover_idx_finds_first_tile_inside_grid() {
        let probe_x = crate::popup_card::PAD_X as f64 + 4.0;
        let probe_y = crate::popup_card::BODY_TOP as f64 + 4.0;
        assert_eq!(workspace_popup_hover_idx(probe_x, probe_y), Some(0));
        assert_eq!(workspace_popup_hover_idx(0.0, 0.0), None);
    }
}
