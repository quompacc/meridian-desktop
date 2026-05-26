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

use crate::ui::tokens::theme_from_config;
use crate::{
    audio::AudioSnapshot,
    icons::{icon_image_to_pixmap, IconCache},
    network::NetworkState,
    panel::{PanelWindowEntry, PinnedApp},
    status_notifier::StatusNotifierItem,
    ClickAction, ClickZone, Rect as ShellRect, PANEL_BOTTOM_GAP, PANEL_HEIGHT,
    PANEL_SIDE_MARGIN, PANEL_SURFACE_HEIGHT,
};

const CHIP_H: i32 = 28;
// Chip widths sized to comfortably hold a single 22px icon (ICON_SIZE)
// with breathing room — earlier values left a tray that fit the network
// icon three times.
const LAUNCHER_W: i32 = 40;
const PINNED_W: i32 = 30;
const TRAY_W: i32 = 30;
const AUDIO_W: i32 = TRAY_W;
const SNI_W: i32 = 30;
const SCREENSHOT_W: i32 = 30;
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
// Soft rounded highlight behind active/hovered chips (matches the island/launcher).
const CHIP_HL_RADIUS: i32 = 8;
const GAP: i32 = 4;

// Floating island
const ISLAND_RADIUS: i32 = 12;
const SIDE_MARGIN: i32 = PANEL_SIDE_MARGIN as i32;
const BOTTOM_GAP: i32 = PANEL_BOTTOM_GAP as i32;
const SURFACE_H: i32 = PANEL_SURFACE_HEIGHT as i32;
// Segment divider chrome
const DIVIDER_W: i32 = 11;
// Frosted-glass island: more transparent body, a milky veil, and a fine
// deterministic grain that fakes the frosted texture (no live backdrop blur).
const PANEL_ISLAND_ALPHA: u8 = 140;
const PANEL_FROST_ALPHA: u8 = 22;
const PANEL_NOISE_STRENGTH: i32 = 9;

const FONT_SIZE: f32 = 14.0;
const ACCENT_LINE_H: i32 = 2;

/// Faceted compass launcher badge. The icon is still rendered in-house so it
/// matches the boot/login compass language, but it uses layered shadow,
/// bevel and needle facets instead of the old flat 2D disc.
fn build_launcher_icon(theme: &Theme) -> Option<Pixmap> {
    use tiny_skia::{FillRule, Paint, PathBuilder, Stroke, Transform};
    let size = LAUNCHER_ICON_SIZE;
    let cx = (size as f32) / 2.0;
    let cy = (size as f32) / 2.0;
    let mut pm = Pixmap::new(size, size)?;
    let palette = &theme.palette;
    let outer_r = (size as f32) / 2.0 - 2.0;
    let inner_r = outer_r - 3.0;
    let tip_inset = 5.2_f32;
    let tip_n = tip_inset;
    let tip_s = size as f32 - tip_inset - 1.0;
    let tip_e = size as f32 - tip_inset - 1.0;
    let tip_w = tip_inset;
    let waist = 3.4_f32;

    let paint_for = |color: Color| {
        let mut p = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        p.set_color_rgba8(color.r, color.g, color.b, color.a);
        p
    };
    let paint_rgba = |r: u8, g: u8, b: u8, a: u8| {
        let mut p = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        p.set_color_rgba8(r, g, b, a);
        p
    };

    let circle = |x: f32, y: f32, r: f32| {
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, r);
        pb.finish()
    };

    // Ground shadow.
    if let Some(ref path) = circle(cx, cy + 2.4, outer_r - 1.0) {
        pm.as_mut().fill_path(
            path,
            &paint_rgba(0, 0, 0, 92),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    // Outer metal rim, then inner accent glass. Several circles are cheaper
    // than a gradient and still create enough dimensionality at 36px.
    if let Some(ref path) = circle(cx, cy, outer_r) {
        pm.as_mut().fill_path(
            path,
            &paint_rgba(18, 22, 34, 255),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    if let Some(ref path) = circle(cx - 0.4, cy - 0.8, outer_r - 1.2) {
        pm.as_mut().fill_path(
            path,
            &paint_for(palette.border),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    if let Some(ref path) = circle(cx, cy, inner_r) {
        pm.as_mut().fill_path(
            path,
            &paint_for(palette.accent),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    if let Some(ref path) = circle(cx - 3.2, cy - 4.4, inner_r * 0.62) {
        pm.as_mut().fill_path(
            path,
            &paint_rgba(255, 255, 255, 36),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    if let Some(ref path) = circle(cx + 3.0, cy + 4.0, inner_r * 0.76) {
        pm.as_mut().fill_path(
            path,
            &paint_rgba(0, 0, 0, 42),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    if let Some(ref path) = circle(cx, cy, inner_r - 1.0) {
        let stroke = Stroke {
            width: 0.9,
            ..Stroke::default()
        };
        pm.as_mut().stroke_path(
            path,
            &paint_for(palette.accent_alt),
            &stroke,
            Transform::identity(),
            None,
        );
    }

    let triangle = |x0: f32, y0: f32, ax: f32, ay: f32, bx: f32, by: f32| {
        let mut pb = PathBuilder::new();
        pb.move_to(x0, y0);
        pb.line_to(ax, ay);
        pb.line_to(bx, by);
        pb.close();
        pb.finish()
    };

    // Needle shadow.
    for path in [
        triangle(
            cx + 0.8,
            tip_n + 1.2,
            cx - waist + 0.8,
            cy + 1.2,
            cx + waist + 0.8,
            cy + 1.2,
        ),
        triangle(
            cx + 0.8,
            tip_s + 1.2,
            cx - waist + 0.8,
            cy + 1.2,
            cx + waist + 0.8,
            cy + 1.2,
        ),
        triangle(
            tip_e + 0.8,
            cy + 1.2,
            cx + 0.8,
            cy - waist + 1.2,
            cx + 0.8,
            cy + waist + 1.2,
        ),
        triangle(
            tip_w + 0.8,
            cy + 1.2,
            cx + 0.8,
            cy - waist + 1.2,
            cx + 0.8,
            cy + waist + 1.2,
        ),
    ]
    .into_iter()
    .flatten()
    {
        pm.as_mut().fill_path(
            &path,
            &paint_rgba(0, 0, 0, 66),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    // Faceted rose. North is bright, the other arms are shaded so the
    // direction reads immediately without becoming a flat star.
    if let Some(ref path) = triangle(cx, tip_n, cx - waist, cy, cx + waist, cy) {
        pm.as_mut().fill_path(
            path,
            &paint_rgba(246, 249, 255, 255),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    for (shape, color) in [
        (
            triangle(cx, tip_s, cx - waist, cy, cx + waist, cy),
            palette.text_dim,
        ),
        (
            triangle(tip_e, cy, cx, cy - waist, cx, cy + waist),
            palette.surface,
        ),
        (
            triangle(tip_w, cy, cx, cy - waist, cx, cy + waist),
            palette.text_dim,
        ),
    ]
    .into_iter()
    {
        if let Some(ref path) = shape {
            pm.as_mut().fill_path(
                path,
                &paint_for(color),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }

    // Hub and specular dot.
    if let Some(ref path) = circle(cx, cy, 3.4) {
        pm.as_mut().fill_path(
            path,
            &paint_rgba(18, 22, 34, 230),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    if let Some(ref path) = circle(cx - 0.9, cy - 1.1, 1.45) {
        pm.as_mut().fill_path(
            path,
            &paint_rgba(255, 255, 255, 210),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    Some(pm)
}

fn build_audio_icon(snapshot: &AudioSnapshot, theme: &Theme) -> Option<Pixmap> {
    use tiny_skia::{FillRule, Paint, PathBuilder, Stroke};

    let mut pm = Pixmap::new(ICON_SIZE, ICON_SIZE)?;
    let palette = &theme.palette;
    let output = snapshot.default_output.as_ref();
    let muted = output
        .map(|device| device.muted || device.volume_percent == Some(0))
        .unwrap_or(true);
    let volume = output.and_then(|device| device.volume_percent).unwrap_or(0);
    let wave_count = if muted {
        0
    } else if volume < 35 {
        1
    } else if volume < 70 {
        2
    } else {
        3
    };

    let paint_for = |color: Color| {
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color_rgba8(color.r, color.g, color.b, color.a);
        paint
    };

    let icon_paint = paint_for(palette.text);
    let accent_paint = paint_for(palette.accent);
    let stroke = Stroke {
        width: 1.6,
        ..Stroke::default()
    };

    let mut box_path = PathBuilder::new();
    box_path.move_to(3.0, 8.0);
    box_path.line_to(7.0, 8.0);
    box_path.line_to(12.0, 4.5);
    box_path.line_to(12.0, 17.5);
    box_path.line_to(7.0, 14.0);
    box_path.line_to(3.0, 14.0);
    box_path.close();
    if let Some(path) = box_path.finish() {
        pm.as_mut().fill_path(
            &path,
            &icon_paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    for level in 0..wave_count {
        let offset = level as f32 * 2.4;
        let mut wave = PathBuilder::new();
        wave.move_to(14.0 + offset, 8.0 - offset * 0.4);
        wave.quad_to(17.0 + offset, 11.0, 14.0 + offset, 14.0 + offset * 0.4);
        if let Some(path) = wave.finish() {
            pm.as_mut()
                .stroke_path(&path, &icon_paint, &stroke, Transform::identity(), None);
        }
    }

    if muted {
        for (x0, y0, x1, y1) in [(15.0, 8.0, 20.0, 14.0), (20.0, 8.0, 15.0, 14.0)] {
            let mut slash = PathBuilder::new();
            slash.move_to(x0, y0);
            slash.line_to(x1, y1);
            if let Some(path) = slash.finish() {
                pm.as_mut().stroke_path(
                    &path,
                    &accent_paint,
                    &Stroke {
                        width: 1.8,
                        ..Stroke::default()
                    },
                    Transform::identity(),
                    None,
                );
            }
        }
    }

    Some(pm)
}

fn action_for_id_as_click(id: &str) -> Option<ClickAction> {
    if let Some(idx) = id
        .strip_prefix("panel-sni-")
        .and_then(|value| value.parse::<usize>().ok())
    {
        return Some(ClickAction::ActivateStatusNotifierItem(idx));
    }
    match id {
        "panel-launcher" => Some(ClickAction::ToggleLauncher),
        "panel-network" => Some(ClickAction::ToggleNetworkPopup),
        "panel-sound" => Some(ClickAction::ToggleAudioPopup),
        "panel-workspace" => Some(ClickAction::ToggleWorkspacePopup),
        "panel-screenshot" => Some(ClickAction::TakeScreenshot),
        "panel-clock" => Some(ClickAction::Clock),
        _ => None,
    }
}

fn status_notifier_label(item: &StatusNotifierItem) -> String {
    let source = item
        .title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            item.icon_name
                .as_deref()
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| item.service.rsplit('.').next())
        .unwrap_or("TR");
    let label: String = source
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(2)
        .map(|ch| ch.to_ascii_uppercase())
        .collect();
    if label.is_empty() {
        "TR".to_string()
    } else {
        label
    }
}

const SNI_PANEL_IDS: [&str; 8] = [
    "panel-sni-0",
    "panel-sni-1",
    "panel-sni-2",
    "panel-sni-3",
    "panel-sni-4",
    "panel-sni-5",
    "panel-sni-6",
    "panel-sni-7",
];

// ── PanelChip ───────────────────────────────────────────────────────────────

/// Thin vertical hairline that separates logical groups (segments) within a
/// cluster. Purely decorative — no id, so it is never a click target.
struct PanelDivider;

impl Widget for PanelDivider {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(DIVIDER_W as f32),
                height: ui_length(CHIP_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        let pal = theme.palette;
        let col = Color::rgba(pal.text.r, pal.text.g, pal.text.b, 38);
        let line = Rect {
            x: area.x + DIVIDER_W / 2,
            y: area.y + 6,
            width: 1,
            height: (area.height - 12).max(1),
        };
        if let Some(path) = rounded_rect_path(line, 0) {
            paint_fill(canvas, &path, col);
        }
    }
}

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

        if is_launcher {
            let halo_color = match state {
                WidgetState::Idle => None,
                WidgetState::Hovered => Some(
                    theme
                        .palette
                        .accent
                        .lerp(Color::rgb(0xff, 0xff, 0xff), 0.10),
                ),
                WidgetState::Pressed => Some(theme.palette.accent.lerp(Color::rgb(0, 0, 0), 0.18)),
            };
            if let Some(mut color) = halo_color {
                color.a = match state {
                    WidgetState::Hovered => 74,
                    WidgetState::Pressed => 96,
                    WidgetState::Idle => 0,
                };
                let halo = Rect {
                    x: area.x + 1,
                    y: 2,
                    width: area.width - 2,
                    height: PANEL_H - 4,
                };
                if let Some(ref path) = rounded_rect_path(halo, 8) {
                    paint_fill(canvas, path, color);
                }
            }
        } else {
            // Idle chips draw no frame so the frosted glass shows through —
            // only the icon floats. Active/hover/pressed get a soft, rounded,
            // translucent highlight that keeps the glass visible.
            let hl: Option<Color> = if self.active {
                let a = theme.palette.accent;
                Some(Color::rgba(a.r, a.g, a.b, 64))
            } else {
                match state {
                    WidgetState::Idle => None,
                    WidgetState::Hovered => Some(Color::rgba(0xFF, 0xFF, 0xFF, 30)),
                    WidgetState::Pressed => Some(Color::rgba(0, 0, 0, 56)),
                }
            };
            if let Some(color) = hl {
                if let Some(ref path) = rounded_rect_path(area, CHIP_HL_RADIUS) {
                    paint_fill(canvas, path, color);
                }
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
                (PANEL_H - ih) / 2 + if state == WidgetState::Pressed { 1 } else { 0 }
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
    window_count: usize,
    has_focused: bool,
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
        // Background — subtle highlight when this app has the focused window
        // No opaque idle frame — the icon floats on the glass. The focused
        // app gets a subtle translucent accent cushion; hover/press a soft
        // rounded highlight.
        let hl: Option<Color> = if self.has_focused {
            let a = theme.palette.accent;
            match state {
                WidgetState::Idle => Some(Color::rgba(a.r, a.g, a.b, 54)),
                WidgetState::Hovered => Some(Color::rgba(a.r, a.g, a.b, 80)),
                WidgetState::Pressed => Some(Color::rgba(0, 0, 0, 56)),
            }
        } else {
            match state {
                WidgetState::Idle => None,
                WidgetState::Hovered => Some(Color::rgba(0xFF, 0xFF, 0xFF, 30)),
                WidgetState::Pressed => Some(Color::rgba(0, 0, 0, 56)),
            }
        };
        if let Some(color) = hl {
            if let Some(ref path) = rounded_rect_path(area, CHIP_HL_RADIUS) {
                paint_fill(canvas, path, color);
            }
        }

        // Icon (centered, shifted up slightly to leave room for indicator)
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

        // Indicator: dot or pill at the bottom of the chip
        let chip_cx = (area.x + area.width / 2) as f32;
        let indicator_cy = (area.y + area.height - 2) as f32; // 2px from chip bottom

        match self.window_count {
            0 => {
                // No running window: dim accent line (subtle, just chip chrome)
                let dim = Color::rgba(
                    theme.palette.accent.r,
                    theme.palette.accent.g,
                    theme.palette.accent.b,
                    55,
                );
                let line = Rect {
                    x: area.x + 4,
                    y: area.y + area.height - ACCENT_LINE_H,
                    width: area.width - 8,
                    height: ACCENT_LINE_H,
                };
                if let Some(ref path) = rounded_rect_path(line, 1) {
                    paint_fill(canvas, path, dim);
                }
            }
            1 => {
                // Single window: small dot
                let dot_color = if self.has_focused {
                    Color::rgba(
                        theme.palette.text.r,
                        theme.palette.text.g,
                        theme.palette.text.b,
                        220,
                    )
                } else {
                    theme.palette.accent
                };
                draw_circle(canvas, chip_cx, indicator_cy, 2.5, dot_color);
            }
            n => {
                // Multiple windows: pill with count
                let dot_color = if self.has_focused {
                    theme.palette.text
                } else {
                    theme.palette.accent
                };
                let label: Box<str> = if n > 9 {
                    "9+".into()
                } else {
                    n.to_string().into()
                };
                let (text_w, _) = measure_text(&label, 9.0);
                let pill_w = (text_w + 8).max(14);
                let pill_h = 9;
                let pill_x = area.x + (area.width - pill_w) / 2;
                let pill_y = area.y + area.height - pill_h - 1;
                if let Some(ref path) = rounded_rect_path(
                    Rect {
                        x: pill_x,
                        y: pill_y,
                        width: pill_w,
                        height: pill_h,
                    },
                    4,
                ) {
                    paint_fill(canvas, path, dot_color);
                }
                let text_color = theme.palette.background;
                paint_text(
                    canvas,
                    &label,
                    pill_x + (pill_w - text_w) / 2,
                    pill_y + pill_h - 1,
                    9.0,
                    text_color,
                );
            }
        }
    }
}

// ── PanelWindowChip ─────────────────────────────────────────────────────────

#[cfg(test)]
struct PanelWindowChip {
    window_id: Box<str>,
    title: Box<str>,
    focused: bool,
    minimized: bool,
    width: i32,
}

#[cfg(test)]
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

// ── Helpers ─────────────────────────────────────────────────────────────────

fn draw_circle(canvas: &mut PixmapMut<'_>, cx: f32, cy: f32, radius: f32, color: Color) {
    use tiny_skia::{FillRule, Paint, PathBuilder, Transform};
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, radius);
    if let Some(path) = pb.finish() {
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color_rgba8(color.r, color.g, color.b, color.a);
        canvas.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn windows_for_pinned_app(app: &PinnedApp, windows: &[PanelWindowEntry]) -> (usize, bool) {
    let program_base = std::path::Path::new(&app.program)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&app.program)
        .to_lowercase();
    let label_lower = app.label.to_lowercase();

    windows.iter().fold((0usize, false), |(count, focused), w| {
        let matches = if let Some(ref app_id) = w.app_id {
            let aid = app_id.to_lowercase();
            aid == program_base
                || aid.ends_with(&format!(".{}", program_base))
                || aid == label_lower
                || aid.ends_with(&format!(".{}", label_lower))
        } else {
            // Fallback: title-based matching for when compositor hasn't sent app_id yet
            let title_lower = w.title.to_lowercase();
            !program_base.is_empty() && title_lower.contains(&program_base)
                || !label_lower.is_empty() && title_lower.contains(&label_lower)
        };
        if matches {
            (count + 1, focused || w.focused)
        } else {
            (count, focused)
        }
    })
}

// ── build_panel_widget_tree ─────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_panel_widget_tree(
    width: u32,
    pinned_apps: &[PinnedApp],
    window_entries: &[PanelWindowEntry],
    network_state: &NetworkState,
    audio_snapshot: &AudioSnapshot,
    status_notifier_items: &[StatusNotifierItem],
    network_popup_open: bool,
    audio_popup_open: bool,
    active_workspace: u8,
    total_workspaces: u8,
    clock: &str,
    icon_cache: &IconCache,
    screenshot_icon: Option<Pixmap>,
    theme: &Theme,
) -> Box<dyn Widget> {
    let network_icon = icon_cache
        .lookup(network_state.icon_name(), ICON_SIZE)
        .and_then(icon_image_to_pixmap);
    let audio_icon = icon_cache
        .lookup(audio_snapshot.icon_name(), ICON_SIZE)
        .and_then(icon_image_to_pixmap)
        .or_else(|| build_audio_icon(audio_snapshot, theme));

    // Left cluster
    let mut left_children: Vec<Box<dyn Widget>> = Vec::new();
    let launcher_icon = build_launcher_icon(theme);
    left_children.push(Box::new(PanelChip::new(
        "panel-launcher",
        "Apps".into(),
        launcher_icon,
        LAUNCHER_W,
        false,
    )));
    if !pinned_apps.is_empty() {
        left_children.push(Box::new(PanelDivider));
    }
    for (idx, app) in pinned_apps.iter().enumerate() {
        let (window_count, has_focused) = windows_for_pinned_app(app, window_entries);
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
            window_count,
            has_focused,
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

    // Center cluster — empty spacer; window indicators are now shown as badges on pinned icons
    let center_children: Vec<Box<dyn Widget>> = Vec::new();
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
    let mut right_children: Vec<Box<dyn Widget>> = Vec::new();
    for (idx, item) in status_notifier_items
        .iter()
        .take(SNI_PANEL_IDS.len())
        .enumerate()
    {
        let icon = item
            .icon_name
            .as_deref()
            .and_then(|name| icon_cache.lookup(name, ICON_SIZE))
            .and_then(icon_image_to_pixmap);
        right_children.push(Box::new(PanelChip::new(
            SNI_PANEL_IDS[idx],
            status_notifier_label(item).into_boxed_str(),
            icon,
            SNI_W,
            false,
        )));
    }
    if !right_children.is_empty() {
        right_children.push(Box::new(PanelDivider));
    }
    right_children.extend([
        Box::new(PanelChip::new(
            "panel-screenshot",
            "📷".into(),
            screenshot_icon,
            SCREENSHOT_W,
            false,
        )) as Box<dyn Widget>,
        Box::new(PanelChip::new(
            "panel-network",
            "NET".into(),
            network_icon,
            TRAY_W,
            network_popup_open,
        )),
        Box::new(PanelChip::new(
            "panel-sound",
            audio_snapshot.panel_label().into_boxed_str(),
            audio_icon,
            AUDIO_W,
            audio_popup_open,
        )),
        Box::new(PanelDivider) as Box<dyn Widget>,
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
    ]);
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

    // The bar holds the three clusters and is inset to the island content box.
    let inner_w = (width as i32 - 2 * SIDE_MARGIN).max(0);
    let bar = Container::new(
        WidgetStyle {
            flex_direction: FlexDirection::Row,
            justify_content: Some(JustifyContent::SpaceBetween),
            align_items: Some(AlignItems::Center),
            size: UiSize {
                width: ui_length(inner_w as f32),
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
    );

    // Outer surface wrapper: full width, inset on the sides + a bottom gap so
    // the island floats. The padding offsets the bar so render() and the click
    // zones inherit the inset automatically.
    Box::new(Container::new(
        WidgetStyle {
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::FlexStart),
            size: UiSize {
                width: ui_length(width as f32),
                height: ui_length(SURFACE_H as f32),
            },
            padding: TaffyRect {
                left: ui_length(SIDE_MARGIN as f32),
                right: ui_length(SIDE_MARGIN as f32),
                top: ui_length(0.0),
                bottom: ui_length(BOTTOM_GAP as f32),
            },
            ..Default::default()
        },
        vec![Box::new(bar) as Box<dyn Widget>],
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
            id: widget.id().map(str::to_string),
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

/// Add a fine, position-deterministic brightness grain to a rectangular
/// region of a tiny-skia RGBA(premultiplied) buffer. Deterministic so it does
/// not shimmer between redraws; only touches pixels that belong to the island
/// (alpha > 0). Fakes the texture of frosted glass without a backdrop blur.
fn apply_frost_noise(data: &mut [u8], w: usize, _h: usize, x0: i32, y0: i32, x1: i32, y1: i32) {
    for y in y0.max(0)..y1 {
        for x in x0.max(0)..x1 {
            let idx = (y as usize * w + x as usize) * 4;
            if idx + 4 > data.len() || data[idx + 3] == 0 {
                continue;
            }
            let mut n = (x as u32)
                .wrapping_mul(374_761_393)
                ^ (y as u32).wrapping_mul(668_265_263);
            n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
            n ^= n >> 16;
            let delta = ((n & 0xff) as i32 - 128) * PANEL_NOISE_STRENGTH / 128;
            for k in 0..3 {
                data[idx + k] = (data[idx + k] as i32 + delta).clamp(0, 255) as u8;
            }
        }
    }
}

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
    audio_snapshot: &AudioSnapshot,
    status_notifier_items: &[StatusNotifierItem],
    network_popup_open: bool,
    audio_popup_open: bool,
    active_workspace: u8,
    total_workspaces: u8,
    clock: &str,
    icon_cache: &IconCache,
    screenshot_icon: Option<Pixmap>,
    theme_config: &meridian_config::ThemeConfig,
    state_fn: &dyn Fn(&[usize]) -> WidgetState,
    clicks_out: &mut Vec<ClickZone>,
    intro_progress: f32,
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

    let theme = theme_from_config(theme_config);

    let root = build_panel_widget_tree(
        width,
        pinned_apps,
        window_entries,
        network_state,
        audio_snapshot,
        status_notifier_items,
        network_popup_open,
        audio_popup_open,
        active_workspace,
        total_workspaces,
        clock,
        icon_cache,
        screenshot_icon,
        &theme,
    );

    let Ok(layout) = compute_layout(&*root, PixelSize { width, height }) else {
        return;
    };

    let Some(mut pixmap) = Pixmap::new(width, height) else {
        return;
    };
    // Transparent everywhere; only the inset island is painted, so the
    // wallpaper shows through the side margins and the bottom gap.
    pixmap.fill(tiny_skia::Color::TRANSPARENT);

    // Frosted-glass island. Drawn in three passes so the grain lands on the
    // glass body but UNDER the icons: (1) border + semi-transparent body +
    // milky veil, (2) deterministic grain over the body, (3) top highlight +
    // the widget tree on top.
    let inner_w = (width as i32 - 2 * SIDE_MARGIN).max(0);
    let base = theme.palette.surface_alt;
    let body_col = Color::rgba(base.r, base.g, base.b, PANEL_ISLAND_ALPHA);
    let border = {
        let l = base.lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.18);
        Color::rgba(l.r, l.g, l.b, 235)
    };
    let outline = Rect {
        x: SIDE_MARGIN,
        y: 0,
        width: inner_w,
        height: PANEL_H,
    };
    let body = Rect {
        x: SIDE_MARGIN + 1,
        y: 1,
        width: (inner_w - 2).max(0),
        height: PANEL_H - 2,
    };
    {
        let mut pc = pixmap.as_mut();
        if let Some(path) = rounded_rect_path(outline, ISLAND_RADIUS) {
            paint_fill(&mut pc, &path, border);
        }
        if let Some(path) = rounded_rect_path(body, ISLAND_RADIUS - 1) {
            paint_fill(&mut pc, &path, body_col);
        }
        let frost = Color::rgba(0xFF, 0xFF, 0xFF, PANEL_FROST_ALPHA);
        if let Some(path) = rounded_rect_path(body, ISLAND_RADIUS - 1) {
            paint_fill(&mut pc, &path, frost);
        }
    }
    apply_frost_noise(
        pixmap.data_mut(),
        width as usize,
        height as usize,
        body.x,
        body.y,
        body.x + body.width,
        body.y + body.height,
    );
    {
        let mut pc = pixmap.as_mut();
        let hl = Color::rgba(0xFF, 0xFF, 0xFF, 26);
        let highlight = Rect {
            x: SIDE_MARGIN + ISLAND_RADIUS,
            y: 1,
            width: (inner_w - 2 * ISLAND_RADIUS).max(0),
            height: 1,
        };
        if let Some(path) = rounded_rect_path(highlight, 0) {
            paint_fill(&mut pc, &path, hl);
        }
        let _ = render(&*root, &layout, &mut pc, &theme, state_fn);
    }

    if intro_progress < 1.0 {
        // Login->desktop entrance: slide the panel up from the screen
        // bottom and fade it in. The area above the rising bar stays
        // transparent, so the compass wallpaper shows through.
        let offset_y = ((1.0 - intro_progress) * height as f32).round() as i32;
        if let Some(mut out) = Pixmap::new(width, height) {
            out.as_mut().draw_pixmap(
                0,
                offset_y,
                pixmap.as_ref(),
                &PixmapPaint {
                    opacity: intro_progress,
                    ..Default::default()
                },
                Transform::identity(),
                None,
            );
            blit_rgba_to_argb(out.data(), canvas);
        }
    } else {
        blit_rgba_to_argb(pixmap.data(), canvas);
    }

    clicks_out.clear();
    collect_click_zones(&*root, &layout.root, 0, 0, clicks_out);
}

#[cfg(test)]
mod tests {
    use meridian_ui::Widget;

    use super::*;
    use crate::{audio::AudioSnapshot, icons::IconCache, network::NetworkState};

    #[test]
    fn panel_chip_style_returns_correct_size() {
        let chip = PanelChip::new("test", "Test".into(), None, 58, false);
        let style = chip.style();
        assert_eq!(style.size.width, ui_length(58.0));
        assert_eq!(style.size.height, ui_length(CHIP_H as f32));
    }

    #[test]
    fn tray_chip_widths_match() {
        assert_eq!(AUDIO_W, TRAY_W);
        assert_eq!(SNI_W, TRAY_W);
        assert_eq!(SCREENSHOT_W, TRAY_W);
    }

    #[test]
    fn panel_pinned_chip_pinned_app_idx_returns_idx() {
        let chip = PanelPinnedChip {
            idx: 2,
            label: "App".into(),
            icon: None,
            program: "prog".into(),
            args: vec![],
            window_count: 0,
            has_focused: false,
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
            window_count: 0,
            has_focused: false,
        };
        assert_eq!(chip.launch_info(), Some(("firefox", &[] as &[String])));
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
    fn status_notifier_label_prefers_title_then_icon_then_service() {
        assert_eq!(
            status_notifier_label(&StatusNotifierItem {
                service: "org.example.Service".to_string(),
                title: Some("Dropbox".to_string()),
                icon_name: Some("cloud-sync".to_string()),
                menu_path: Some("/Menu".to_string()),
            }),
            "DR"
        );
        assert_eq!(
            status_notifier_label(&StatusNotifierItem {
                service: "org.example.Service".to_string(),
                title: None,
                icon_name: Some("cloud-sync".to_string()),
                menu_path: None,
            }),
            "CL"
        );
        assert_eq!(
            status_notifier_label(&StatusNotifierItem {
                service: "org.example.Service".to_string(),
                title: None,
                icon_name: None,
                menu_path: None,
            }),
            "SE"
        );
    }

    #[test]
    fn build_panel_widget_tree_root_has_three_children() {
        let icon_cache = IconCache::new();
        let network = NetworkState::Disconnected;
        let audio = AudioSnapshot::unavailable();
        let tree = build_panel_widget_tree(
            1920,
            &[],
            &[],
            &network,
            &audio,
            &[],
            false,
            false,
            1,
            9,
            "12:34",
            &icon_cache,
            None,
            &Theme::TOKYO_NIGHT_METRO,
        );
        assert_eq!(tree.children().len(), 3);
    }

    #[test]
    fn draw_panel_ui_modifies_canvas_and_fills_clicks() {
        let width = 1024u32;
        let height = PANEL_HEIGHT;
        let mut canvas = vec![0u8; (width * height * 4) as usize];
        let icon_cache = IconCache::new();
        let network = NetworkState::Disconnected;
        let audio = AudioSnapshot::unavailable();
        let mut clicks = Vec::new();
        let state_fn = |_: &[usize]| WidgetState::Idle;

        draw_panel_ui(
            &mut canvas,
            width,
            height,
            &[],
            &[],
            &network,
            &audio,
            &[],
            false,
            false,
            1,
            9,
            "12:34",
            &icon_cache,
            None,
            &meridian_config::ThemeConfig::default(),
            &state_fn,
            &mut clicks,
            1.0,
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
        assert!(matches!(
            action_for_id_as_click("panel-sound"),
            Some(ClickAction::ToggleAudioPopup)
        ));
    }
}
