use meridian_config::ThemeSurface;
use meridian_tokens::{Elevation, Interaction};
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

use crate::ui::tokens::glass_theme_from_config;
use crate::{
    audio::AudioSnapshot,
    icons::{icon_image_to_pixmap, IconCache},
    network::NetworkState,
    panel::{PanelWindowEntry, PinnedApp},
    status_notifier::StatusNotifierItem,
    ClickAction, ClickZone, Rect as ShellRect, PANEL_BOTTOM_GAP, PANEL_HEIGHT, PANEL_SIDE_MARGIN,
    PANEL_SURFACE_HEIGHT, PANEL_TOP_SHADOW,
};

const CHIP_H: i32 = 28;
// Chip widths sized to comfortably hold a single 22px icon (ICON_SIZE)
// with breathing room — earlier values left a tray that fit the network
// icon three times.
const LAUNCHER_W: i32 = 40;
const PINNED_W: i32 = 30;
const TRAY_W: i32 = 30;
const AUDIO_W: i32 = TRAY_W;
// Battery chip needs room for the icon plus a "100%" label.
const BATTERY_W: i32 = 52;
const SNI_W: i32 = 30;
const SCREENSHOT_W: i32 = 30;
// Launcher gets a reduced, themed Meridian start symbol that sits visually
// raised above the chip outline (no bg fill, no accent strip) so it reads as
// the entry point rather than just another tile (manifest §9).
const LAUNCHER_ICON_SIZE: u32 = 36;
const WS_W: i32 = 56;
const CLOCK_PAD: i32 = 8;
const ICON_SIZE: u32 = 22;
const PANEL_H: i32 = PANEL_HEIGHT as i32;

const LEFT_PADDING: i32 = 8;
const RIGHT_PADDING: i32 = 10;
// Soft rounded highlight behind active/hovered chips (matches the island/launcher).
const CHIP_HL_RADIUS: i32 = meridian_tokens::Radius::DEFAULT.md;

// Panel taskbar-indicator opacities. The colour is ALWAYS a theme colour; only
// these fixed contrast values live here (the taskbar's "active app" design
// language, manifest §9). Named once instead of inline magic.
/// Segment divider hairline (over the theme text colour).
const SEGMENT_DIVIDER_OPACITY: u8 = 38;
/// Idle (no running window) accent indicator line.
const IDLE_INDICATOR_OPACITY: u8 = 55;
/// Focused single-window indicator dot (over the theme text colour).
const FOCUSED_DOT_OPACITY: u8 = 220;
const GAP: i32 = 4;

// Floating island
const SIDE_MARGIN: i32 = PANEL_SIDE_MARGIN as i32;
const BOTTOM_GAP: i32 = PANEL_BOTTOM_GAP as i32;
const SURFACE_H: i32 = PANEL_SURFACE_HEIGHT as i32;
const ISLAND_TOP: i32 = PANEL_TOP_SHADOW as i32;
// Segment divider chrome
const DIVIDER_W: i32 = 11;
// Frosted-glass island: transparent shell tint over the compositor-owned
// live backdrop blur. Noise stays off so the real blurred scene remains legible.
const PANEL_NOISE_STRENGTH: i32 = 0;

const FONT_SIZE: f32 = 14.0;
const ACCENT_LINE_H: i32 = 2;

/// Reduced Meridian start-symbol: a calm accent ring with a single north
/// pointer and a centre navigation dot. Per the design manifest §9 the start
/// button is "kein buntes Logo" — an abstract, themed mark (not the old faceted
/// compass badge); every colour comes from the theme so it flips with light/dark.
fn build_launcher_icon(theme: &Theme) -> Option<Pixmap> {
    use tiny_skia::{FillRule, Paint, PathBuilder, Stroke, Transform};
    let size = LAUNCHER_ICON_SIZE;
    let cx = (size as f32) / 2.0;
    let cy = (size as f32) / 2.0;
    let mut pm = Pixmap::new(size, size)?;
    let palette = &theme.palette;
    let ring_r = (size as f32) / 2.0 - 3.0;
    let waist = 2.6_f32;

    let paint_for = |color: Color| {
        let mut p = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        p.set_color_rgba8(color.r, color.g, color.b, color.a);
        p
    };

    // Calm accent ring — the "reduzierter Meridian-Kreis".
    if let Some(ref path) = PathBuilder::from_circle(cx, cy, ring_r) {
        let stroke = Stroke {
            width: 2.0,
            ..Stroke::default()
        };
        pm.as_mut().stroke_path(
            path,
            &paint_for(palette.accent),
            &stroke,
            Transform::identity(),
            None,
        );
    }

    // Single north pointer — the clear axis / navigation direction (abstract).
    let mut needle = PathBuilder::new();
    needle.move_to(cx, cy - ring_r + 1.0);
    needle.line_to(cx - waist, cy);
    needle.line_to(cx + waist, cy);
    needle.close();
    if let Some(ref path) = needle.finish() {
        pm.as_mut().fill_path(
            path,
            &paint_for(palette.accent),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    // Centre navigation dot.
    if let Some(ref path) = PathBuilder::from_circle(cx, cy, 2.2) {
        pm.as_mut().fill_path(
            path,
            &paint_for(palette.accent),
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
        "panel-battery" => Some(ClickAction::CyclePowerProfile),
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

// Thin vertical hairline that separates logical groups (segments) within a
// cluster. Purely decorative — no id, so it is never a click target.

include!("panel_view/chips.rs");
include!("panel_view/pinned.rs");
include!("panel_view/windows.rs");
include!("panel_view/layout.rs");
include!("panel_view/render.rs");

#[cfg(test)]
#[path = "panel_view_tests.rs"]
mod tests;
