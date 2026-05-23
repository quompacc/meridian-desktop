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
// Chip widths sized to comfortably hold a single 22px icon (ICON_SIZE)
// with breathing room — earlier values left a tray that fit the network
// icon three times.
const LAUNCHER_W: i32 = 40;
const PINNED_W: i32 = 30;
const TRAY_W: i32 = 30;
const SCREENSHOT_W: i32 = 30;
const SETTINGS_W: i32 = 30;
// Launcher gets its own larger compass-rose icon that sits visually
// raised above the chip outline (no bg fill, no accent strip) so it
// reads as the entry point rather than just another tile.
const LAUNCHER_ICON_SIZE: u32 = 36;
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

/// Compass-rose launcher badge. A filled accent circle (the "medallion")
/// with a light-coloured 4-point rose on top — visually unmistakably
/// round, and contrasts strongly enough against the panel surface to
/// read as the entry point. Genuine "lifts above the panel line" would
/// need the panel layer-surface to be taller than its exclusive zone
/// (panel chrome painted only in the bottom band, top transparent so
/// the icon overflows visually); see TODO at the call site.
fn build_launcher_icon(theme: &Theme) -> Option<Pixmap> {
    use tiny_skia::{FillRule, Paint, PathBuilder, Stroke, Transform};
    let size = LAUNCHER_ICON_SIZE;
    let cx = (size as f32) / 2.0;
    let cy = (size as f32) / 2.0;
    let mut pm = Pixmap::new(size, size)?;
    let palette = &theme.palette;
    let outer_r = (size as f32) / 2.0 - 1.0;
    let tip_inset = 5.5_f32;
    let tip = tip_inset;
    let edge = (size as f32) - tip_inset;
    let waist: f32 = 3.2;

    let paint_for = |color: Color| {
        let mut p = Paint::default();
        p.anti_alias = true;
        p.set_color_rgba8(color.r, color.g, color.b, color.a);
        p
    };

    // 1) Filled medallion — accent-blue disc, full radius. This is what
    //    makes the icon read as round at a glance.
    let medallion = {
        let mut pb = PathBuilder::new();
        pb.push_circle(cx, cy, outer_r);
        pb.finish()
    };
    if let Some(ref path) = medallion {
        pm.as_mut().fill_path(
            path,
            &paint_for(palette.accent),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    // 2) Inner highlight ring — 1px stroke in a lighter accent variant
    //    for a touch of depth (no full bevel, just a hint of dimension).
    let inner_ring = {
        let mut pb = PathBuilder::new();
        pb.push_circle(cx, cy, outer_r - 1.5);
        pb.finish()
    };
    if let Some(ref path) = inner_ring {
        let mut stroke = Stroke::default();
        stroke.width = 1.0;
        pm.as_mut().stroke_path(
            path,
            &paint_for(palette.accent_alt),
            &stroke,
            Transform::identity(),
            None,
        );
    }

    // 3) 4-point rose. N arm uses palette.surface so it pops clean
    //    against the accent medallion; S/E/W in text_dim give a subtle
    //    directional hint without competing.
    let arm = |x0: f32, y0: f32, ax: f32, ay: f32, bx: f32, by: f32| {
        let mut pb = PathBuilder::new();
        pb.move_to(x0, y0);
        pb.line_to(ax, ay);
        pb.line_to(bx, by);
        pb.close();
        pb.finish()
    };

    if let Some(ref path) = arm(cx, tip, cx - waist, cy, cx + waist, cy) {
        pm.as_mut().fill_path(
            path,
            &paint_for(palette.surface),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    let muted = palette.text_dim;
    for path in [
        arm(cx, edge, cx - waist, cy, cx + waist, cy),
        arm(edge, cy, cx, cy - waist, cx, cy + waist),
        arm(tip, cy, cx, cy - waist, cx, cy + waist),
    ]
    .into_iter()
    .flatten()
    {
        pm.as_mut().fill_path(
            &path,
            &paint_for(muted),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    // 4) Central pivot — bright dot in surface, ties the arms together
    //    and reads as the hinge of a real compass needle.
    let pivot = {
        let mut pb = PathBuilder::new();
        pb.push_circle(cx, cy, 2.0);
        pb.finish()
    };
    if let Some(ref path) = pivot {
        pm.as_mut().fill_path(
            path,
            &paint_for(palette.surface),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    Some(pm)
}

fn action_for_id_as_click(id: &str) -> Option<ClickAction> {
    match id {
        "panel-launcher" => Some(ClickAction::ToggleLauncher),
        "panel-network" => Some(ClickAction::ToggleNetworkPopup),
        "panel-workspace" => Some(ClickAction::ToggleWorkspacePopup),
        "panel-screenshot" => Some(ClickAction::TakeScreenshot),
        "panel-settings" => Some(ClickAction::ToggleSettings),
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
        // The launcher chip is special: no rectangular chip chrome,
        // just the compass rose centred in the panel so the icon
        // visually sits proud of the panel line (Win8-style start-button
        // pivot). Skip the bg fill + accent strip and let the icon
        // speak for itself.
        let is_launcher = self.id == "panel-launcher";

        if !is_launcher {
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
        }

        if let Some(ref icon) = self.icon {
            let iw = icon.width() as i32;
            let ih = icon.height() as i32;
            let x = area.x + (area.width - iw) / 2;
            // Launcher: vertical-centre against the whole panel so an
            // oversized rose extends slightly above/below the chip's
            // own rectangle, not just within it.
            let y = if is_launcher {
                (PANEL_H - ih) / 2
            } else {
                area.y + (area.height - ACCENT_LINE_H - ih) / 2
            };
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

        if !is_launcher {
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
    settings_open: bool,
) -> Box<dyn Widget> {
    let network_icon = icon_cache
        .lookup(network_state.icon_name(), ICON_SIZE)
        .and_then(icon_image_to_pixmap);

    // Left cluster
    let mut left_children: Vec<Box<dyn Widget>> = Vec::new();
    // Compass-needle launcher icon, rendered in-house to match the
    // bootsplash visual language. Uses the same hardcoded theme
    // constant as `draw_panel_ui` below (TOKYO_NIGHT_METRO).
    let launcher_icon = build_launcher_icon(&Theme::TOKYO_NIGHT_METRO);
    left_children.push(Box::new(PanelChip::new(
        "panel-launcher",
        "Apps".into(),
        launcher_icon,
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
            "panel-settings",
            "\u{2699}".into(),
            None,
            SETTINGS_W,
            settings_open,
        )),
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
    settings_open: bool,
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
        settings_open,
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
            false,
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
            false,
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
