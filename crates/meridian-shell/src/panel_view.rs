use meridian_ui::{
    compute_layout,
    effect::{measure_text, paint_fill, paint_text, rounded_rect_path},
    paint::{LayoutNode, Rect},
    render,
    style::Color,
    ui_length,
    widget::{Container, Widget},
    AlignItems, FlexDirection, JustifyContent, Overflow, PixelSize, TaffyPoint, TaffyRect, Theme,
    UiSize, WidgetState, WidgetStyle,
};
use tiny_skia::{Pixmap, PixmapMut, PixmapPaint, Transform};

use crate::{
    icons::{IconCache, IconImage},
    network::NetworkState,
    panel::{PanelWindowEntry, PinnedApp},
    ClickAction, ClickZone, Rect as ShellRect, PANEL_HEIGHT,
};

const CHIP_H: i32 = 28;
const LAUNCHER_W: i32 = 58;
const PINNED_W: i32 = 44;
const TRAY_W: i32 = 60;
const SCREENSHOT_W: i32 = 36;
const WS_W: i32 = 56;
const CLOCK_PAD: i32 = 8;
const ICON_SIZE: u32 = 22;
const PANEL_H: i32 = PANEL_HEIGHT as i32;

const LEFT_PADDING: i32 = 8;
const RIGHT_PADDING: i32 = 10;
const CHIP_RADIUS: i32 = 3;
const GAP: i32 = 4;

const FONT_SIZE: f32 = 14.0;
const ACCENT_LINE_H: i32 = 2;

pub(crate) fn icon_image_to_pixmap(img: &IconImage) -> Option<Pixmap> {
    let w = img.width;
    let h = img.height;
    let mut pixmap = Pixmap::new(w, h)?;
    let data = pixmap.data_mut();
    for (i, chunk) in img.bgra.chunks_exact(4).enumerate() {
        let b = chunk[0];
        let g = chunk[1];
        let r = chunk[2];
        let a = chunk[3];
        let out_idx = i * 4;
        data[out_idx] = ((r as u16 * a as u16) / 255) as u8;
        data[out_idx + 1] = ((g as u16 * a as u16) / 255) as u8;
        data[out_idx + 2] = ((b as u16 * a as u16) / 255) as u8;
        data[out_idx + 3] = a;
    }
    Some(pixmap)
}

fn action_for_id_as_click(id: &str) -> Option<ClickAction> {
    match id {
        "panel-launcher" => Some(ClickAction::ToggleLauncher),
        "panel-network" => Some(ClickAction::ToggleNetworkPopup),
        "panel-workspace" => Some(ClickAction::ToggleWorkspacePopup),
        "panel-screenshot" => Some(ClickAction::TakeScreenshot),
        "panel-clock" => Some(ClickAction::Clock),
        _ => None,
    }
}

// ── PanelChip ───────────────────────────────────────────────────────────────

struct PanelChip {
    id: &'static str,
    label: Box<str>,
    icon: Option<Pixmap>,
    width: i32,
    active: bool,
}

impl PanelChip {
    fn new(
        id: &'static str,
        label: Box<str>,
        icon: Option<Pixmap>,
        width: i32,
        active: bool,
    ) -> Self {
        Self {
            id,
            label,
            icon,
            width,
            active,
        }
    }
}

impl Widget for PanelChip {
    fn id(&self) -> Option<&'static str> {
        Some(self.id)
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length(CHIP_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = if self.active {
            theme.palette.border
        } else {
            match state {
                WidgetState::Idle => theme.palette.surface,
                WidgetState::Hovered => theme
                    .palette
                    .surface
                    .lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.12),
                WidgetState::Pressed => theme.palette.surface.lerp(Color::rgb(0, 0, 0), 0.15),
            }
        };

        if let Some(ref path) = rounded_rect_path(area, CHIP_RADIUS) {
            paint_fill(canvas, path, bg);
        }

        if let Some(ref icon) = self.icon {
            let iw = icon.width() as i32;
            let ih = icon.height() as i32;
            let x = area.x + (area.width - iw) / 2;
            let y = area.y + (area.height - ACCENT_LINE_H - ih) / 2;
            canvas.draw_pixmap(
                x,
                y,
                icon.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        } else {
            let (text_w, _) = measure_text(&self.label, FONT_SIZE);
            let tx = area.x + (area.width - text_w) / 2;
            let ty = area.y + (area.height - ACCENT_LINE_H) / 2 + 5;
            paint_text(canvas, &self.label, tx, ty, FONT_SIZE, theme.palette.text);
        }

        // accent line bottom
        let line = Rect {
            x: area.x,
            y: area.y + area.height - ACCENT_LINE_H,
            width: area.width,
            height: ACCENT_LINE_H,
        };
        if let Some(ref path) = rounded_rect_path(line, 0) {
            paint_fill(canvas, path, theme.palette.accent);
        }
    }
}

// ── PanelPinnedChip ─────────────────────────────────────────────────────────

struct PanelPinnedChip {
    idx: usize,
    label: Box<str>,
    icon: Option<Pixmap>,
    program: Box<str>,
    args: Vec<String>,
}

impl Widget for PanelPinnedChip {
    fn id(&self) -> Option<&'static str> {
        None
    }

    fn pinned_app_idx(&self) -> Option<usize> {
        Some(self.idx)
    }

    fn launch_info(&self) -> Option<(&str, &[String])> {
        Some((&self.program, &self.args))
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(PINNED_W as f32),
                height: ui_length(CHIP_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = match state {
            WidgetState::Idle => theme.palette.surface,
            WidgetState::Hovered => theme
                .palette
                .surface
                .lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.12),
            WidgetState::Pressed => theme.palette.surface.lerp(Color::rgb(0, 0, 0), 0.15),
        };

        if let Some(ref path) = rounded_rect_path(area, CHIP_RADIUS) {
            paint_fill(canvas, path, bg);
        }

        if let Some(ref icon) = self.icon {
            let iw = icon.width() as i32;
            let ih = icon.height() as i32;
            let x = area.x + (area.width - iw) / 2;
            let y = area.y + (area.height - ACCENT_LINE_H - ih) / 2;
            canvas.draw_pixmap(
                x,
                y,
                icon.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        } else {
            let (text_w, _) = measure_text(&self.label, FONT_SIZE);
            let tx = area.x + (area.width - text_w) / 2;
            let ty = area.y + (area.height - ACCENT_LINE_H) / 2 + 5;
            paint_text(canvas, &self.label, tx, ty, FONT_SIZE, theme.palette.text);
        }

        // accent line bottom
        let line = Rect {
            x: area.x,
            y: area.y + area.height - ACCENT_LINE_H,
            width: area.width,
            height: ACCENT_LINE_H,
        };
        if let Some(ref path) = rounded_rect_path(line, 0) {
            paint_fill(canvas, path, theme.palette.accent);
        }
    }
}

// ── PanelWindowChip ─────────────────────────────────────────────────────────

struct PanelWindowChip {
    window_id: Box<str>,
    title: Box<str>,
    focused: bool,
    minimized: bool,
    width: i32,
}

impl Widget for PanelWindowChip {
    fn id(&self) -> Option<&'static str> {
        None
    }

    fn focus_window_id(&self) -> Option<&str> {
        Some(&self.window_id)
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length(20.0),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let base_bg = if self.focused {
            theme
                .palette
                .border
                .lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.15)
        } else if self.minimized {
            theme.palette.surface.lerp(Color::rgb(0, 0, 0), 0.25)
        } else {
            theme.palette.surface
        };

        let bg = match state {
            WidgetState::Idle => base_bg,
            WidgetState::Hovered => base_bg.lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.10),
            WidgetState::Pressed => base_bg.lerp(Color::rgb(0, 0, 0), 0.10),
        };

        if let Some(ref path) = rounded_rect_path(area, CHIP_RADIUS) {
            paint_fill(canvas, path, bg);
        }

        let text_color = if self.focused {
            theme.palette.accent
        } else if self.minimized {
            theme.palette.text_dim
        } else {
            theme.palette.text
        };

        paint_text(
            canvas,
            &self.title,
            area.x + 4,
            area.y + area.height / 2 + 3,
            FONT_SIZE,
            text_color,
        );

        if self.focused {
            let indicator_rect = Rect {
                x: area.x,
                y: area.y + area.height - 2,
                width: area.width,
                height: 2,
            };
            if let Some(ref path) = rounded_rect_path(indicator_rect, 0) {
                paint_fill(canvas, path, theme.palette.accent);
            }
        }
    }
}

// ── build_panel_widget_tree ─────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_panel_widget_tree(
    width: u32,
    pinned_apps: &[PinnedApp],
    window_entries: &[PanelWindowEntry],
    network_state: &NetworkState,
    network_popup_open: bool,
    active_workspace: u8,
    total_workspaces: u8,
    clock: &str,
    icon_cache: &IconCache,
    screenshot_icon: Option<Pixmap>,
) -> Box<dyn Widget> {
    let network_icon = icon_cache
        .lookup(network_state.icon_name(), ICON_SIZE)
        .and_then(icon_image_to_pixmap);

    // Left cluster
    let mut left_children: Vec<Box<dyn Widget>> = Vec::new();
    left_children.push(Box::new(PanelChip::new(
        "panel-launcher",
        "Apps".into(),
        None,
        LAUNCHER_W,
        false,
    )));
    for (idx, app) in pinned_apps.iter().enumerate() {
        let icon = app
            .icon_name
            .as_deref()
            .and_then(|name| icon_cache.lookup(name, ICON_SIZE))
            .and_then(icon_image_to_pixmap);
        left_children.push(Box::new(PanelPinnedChip {
            idx,
            label: app.label.clone().into_boxed_str(),
            icon,
            program: app.program.clone().into_boxed_str(),
            args: app.args.clone(),
        }));
    }
    let left_cluster = Container::new(
        WidgetStyle {
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Center),
            gap: UiSize {
                width: ui_length(GAP as f32),
                height: ui_length(0.0),
            },
            ..Default::default()
        },
        left_children,
    );

    // Center cluster
    let mut center_children: Vec<Box<dyn Widget>> = Vec::new();
    for entry in window_entries {
        let title_width = entry.title.len() as i32 * 8 + 16;
        let entry_w = title_width.clamp(60, 200);
        center_children.push(Box::new(PanelWindowChip {
            window_id: entry.id.clone().into_boxed_str(),
            title: entry.title.clone().into_boxed_str(),
            focused: entry.focused,
            minimized: entry.minimized,
            width: entry_w,
        }));
    }
    let center_cluster = Container::new(
        WidgetStyle {
            flex_direction: FlexDirection::Row,
            flex_grow: 1.0,
            align_items: Some(AlignItems::Center),
            gap: UiSize {
                width: ui_length(GAP as f32),
                height: ui_length(0.0),
            },
            overflow: TaffyPoint {
                x: Overflow::Hidden,
                y: Overflow::Hidden,
            },
            ..Default::default()
        },
        center_children,
    );

    // Right cluster
    let (clock_text_w, _) = measure_text(clock, FONT_SIZE);
    let clock_w = (clock_text_w + 2 * CLOCK_PAD).max(40);
    let ws_text: Box<str> = format!("{}/{}", active_workspace, total_workspaces.max(1)).into();
    let right_children: Vec<Box<dyn Widget>> = vec![
        Box::new(PanelChip::new(
            "panel-screenshot",
            "📷".into(),
            screenshot_icon,
            SCREENSHOT_W,
            false,
        )),
        Box::new(PanelChip::new(
            "panel-network",
            "NET".into(),
            network_icon,
            TRAY_W,
            network_popup_open,
        )),
        Box::new(PanelChip::new(
            "panel-workspace",
            ws_text,
            None,
            WS_W,
            false,
        )),
        Box::new(PanelChip::new(
            "panel-clock",
            clock.to_string().into_boxed_str(),
            None,
            clock_w,
            false,
        )),
    ];
    let right_cluster = Container::new(
        WidgetStyle {
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Center),
            gap: UiSize {
                width: ui_length(GAP as f32),
                height: ui_length(0.0),
            },
            ..Default::default()
        },
        right_children,
    );

    Box::new(Container::new(
        WidgetStyle {
            flex_direction: FlexDirection::Row,
            justify_content: Some(JustifyContent::SpaceBetween),
            align_items: Some(AlignItems::Center),
            size: UiSize {
                width: ui_length(width as f32),
                height: ui_length(PANEL_H as f32),
            },
            padding: TaffyRect {
                left: ui_length(LEFT_PADDING as f32),
                right: ui_length(RIGHT_PADDING as f32),
                top: ui_length(0.0),
                bottom: ui_length(0.0),
            },
            ..Default::default()
        },
        vec![
            Box::new(left_cluster) as Box<dyn Widget>,
            Box::new(center_cluster) as Box<dyn Widget>,
            Box::new(right_cluster) as Box<dyn Widget>,
        ],
    ))
}

// ── collect_click_zones ─────────────────────────────────────────────────────

fn collect_click_zones(
    widget: &dyn Widget,
    node: &LayoutNode,
    parent_x: i32,
    parent_y: i32,
    out: &mut Vec<ClickZone>,
) {
    let abs_x = parent_x + node.rect.x;
    let abs_y = parent_y + node.rect.y;

    let action = widget
        .id()
        .and_then(action_for_id_as_click)
        .or_else(|| widget.pinned_app_idx().map(ClickAction::LaunchPinnedApp))
        .or_else(|| {
            widget
                .focus_window_id()
                .map(|id| ClickAction::FocusWindow(id.to_string()))
        });

    if let Some(action) = action {
        out.push(ClickZone {
            rect: ShellRect {
                x: abs_x,
                y: abs_y,
                w: node.rect.width,
                h: node.rect.height,
            },
            action,
        });
    }

    for (child, child_node) in widget.children().iter().zip(node.children.iter()) {
        collect_click_zones(child.as_ref(), child_node, abs_x, abs_y, out);
    }
}

// ── draw_panel_ui ───────────────────────────────────────────────────────────

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

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_panel_ui(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    pinned_apps: &[PinnedApp],
    window_entries: &[PanelWindowEntry],
    network_state: &NetworkState,
    network_popup_open: bool,
    active_workspace: u8,
    total_workspaces: u8,
    clock: &str,
    icon_cache: &IconCache,
    screenshot_icon: Option<Pixmap>,
    state_fn: &dyn Fn(&[usize]) -> WidgetState,
    clicks_out: &mut Vec<ClickZone>,
) {
    let expected_len = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if canvas.len() != expected_len {
        tracing::warn!(
            "draw_panel_ui: canvas size mismatch, expected {} got {}",
            expected_len,
            canvas.len()
        );
        return;
    }

    let root = build_panel_widget_tree(
        width,
        pinned_apps,
        window_entries,
        network_state,
        network_popup_open,
        active_workspace,
        total_workspaces,
        clock,
        icon_cache,
        screenshot_icon,
    );

    let Ok(layout) = compute_layout(&*root, PixelSize { width, height }) else {
        return;
    };

    let Some(mut pixmap) = Pixmap::new(width, height) else {
        return;
    };
    let bg = meridian_ui::style::Palette::TOKYO_NIGHT_METRO.surface_alt;
    pixmap.fill(tiny_skia::Color::from_rgba8(bg.r, bg.g, bg.b, 0xff));

    let mut pixmap_canvas = pixmap.as_mut();
    let _ = render(
        &*root,
        &layout,
        &mut pixmap_canvas,
        &Theme::TOKYO_NIGHT_METRO,
        state_fn,
    );

    blit_rgba_to_argb(pixmap.data(), canvas);

    clicks_out.clear();
    collect_click_zones(&*root, &layout.root, 0, 0, clicks_out);
}

#[cfg(test)]
mod tests {
    use meridian_ui::Widget;

    use super::*;
    use crate::{icons::IconCache, network::NetworkState};

    #[test]
    fn panel_chip_style_returns_correct_size() {
        let chip = PanelChip::new("test", "Test".into(), None, 58, false);
        let style = chip.style();
        assert_eq!(style.size.width, ui_length(58.0));
        assert_eq!(style.size.height, ui_length(CHIP_H as f32));
    }

    #[test]
    fn panel_pinned_chip_pinned_app_idx_returns_idx() {
        let chip = PanelPinnedChip {
            idx: 2,
            label: "App".into(),
            icon: None,
            program: "prog".into(),
            args: vec![],
        };
        assert_eq!(chip.pinned_app_idx(), Some(2));
    }

    #[test]
    fn panel_pinned_chip_launch_info_returns_program_and_args() {
        let chip = PanelPinnedChip {
            idx: 0,
            label: "Firefox".into(),
            icon: None,
            program: "firefox".into(),
            args: vec![],
        };
        assert_eq!(chip.launch_info(), Some(("firefox", &vec![] as &[String])));
    }

    #[test]
    fn panel_window_chip_focus_window_id_returns_id() {
        let chip = PanelWindowChip {
            window_id: "win-1".into(),
            title: "Window".into(),
            focused: false,
            minimized: false,
            width: 100,
        };
        assert_eq!(chip.focus_window_id(), Some("win-1"));
    }

    #[test]
    fn build_panel_widget_tree_root_has_three_children() {
        let icon_cache = IconCache::new();
        let network = NetworkState::Disconnected;
        let tree = build_panel_widget_tree(
            1920,
            &[],
            &[],
            &network,
            false,
            1,
            9,
            "12:34",
            &icon_cache,
            None,
        );
        assert_eq!(tree.children().len(), 3);
    }

    #[test]
    #[test]
    fn draw_panel_ui_modifies_canvas_and_fills_clicks() {
        let width = 1024u32;
        let height = PANEL_HEIGHT;
        let mut canvas = vec![0u8; (width * height * 4) as usize];
        let icon_cache = IconCache::new();
        let network = NetworkState::Disconnected;
        let mut clicks = Vec::new();
        let state_fn = |_: &[usize]| WidgetState::Idle;

        draw_panel_ui(
            &mut canvas,
            width,
            height,
            &[],
            &[],
            &network,
            false,
            1,
            9,
            "12:34",
            &icon_cache,
            None,
            &state_fn,
            &mut clicks,
        );

        assert!(canvas.iter().any(|byte| *byte != 0));
        assert!(!clicks.is_empty());
    }

    #[test]
    fn action_for_id_as_click_screenshot() {
        assert!(matches!(
            action_for_id_as_click("panel-screenshot"),
            Some(ClickAction::TakeScreenshot)
        ));
    }
}
