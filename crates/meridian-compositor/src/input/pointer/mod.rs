use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, InputBackend, PointerAxisEvent, PointerMotionEvent,
    },
    desktop::Window,
    input::pointer::{AxisFrame, MotionEvent},
    utils::SERIAL_COUNTER,
    utils::{Logical, Point, Rectangle, Size},
    wayland::seat::WaylandFocus,
};
use tracing::debug;

use crate::{
    backend::drm::DrmCursorIcon,
    cursor::CursorImage,
    decoration::{DecorationHit, DecorationResizeEdge, HoveredButton},
    state::{MeridianState, OutputGeometry, OutputId, OutputRegistry},
};

mod button;

pub use button::handle_pointer_button;

pub fn handle_pointer_motion_absolute<I: InputBackend>(
    state: &mut MeridianState,
    event: &impl AbsolutePositionEvent<I>,
) {
    let (origin_x, origin_y, width, height) = match desktop_bounds(state) {
        Some(bounds) => bounds,
        None => return,
    };
    let desktop_size: Size<i32, Logical> = (width, height).into();
    let transformed: Point<f64, Logical> = event.position_transformed(desktop_size);
    let origin: Point<f64, Logical> = (origin_x as f64, origin_y as f64).into();
    let pos = transformed + origin;

    if output_id_at_point_for_focus(&state.output_registry, pos.x, pos.y).is_some() {
        state.update_focused_output_from_point(pos, "pointer-motion", false);
    }
    let (selected_output, fallback_used) =
        select_output_from_registry_for_point(&state.output_registry, pos.x, pos.y);
    if let Some(output) = selected_output {
        debug!(
            "pointer absolute motion: x={:.2} y={:.2} selected_output_id={} name={} fallback={}",
            pos.x, pos.y, output.id.0, output.name, fallback_used
        );
    } else {
        debug!(
            "pointer absolute motion: x={:.2} y={:.2} selected_output=none fallback=true",
            pos.x, pos.y
        );
    }
    if fallback_used {
        debug!("pointer absolute motion fallback: no output contains point");
    }
    update_hover_cursor_feedback(state, pos);

    let serial = SERIAL_COUNTER.next_serial();
    let Some(pointer) = state.seat.get_pointer() else {
        debug!("pointer absolute motion ignored: seat has no pointer");
        return;
    };
    let under = state.surface_under(pos);

    pointer.motion(
        state,
        under,
        &MotionEvent {
            location: pos,
            serial,
            time: event.time_msec(),
        },
    );
    pointer.frame(state);
}

pub fn handle_pointer_motion_relative<I: InputBackend>(
    state: &mut MeridianState,
    event: &impl PointerMotionEvent<I>,
) {
    let Some(pointer) = state.seat.get_pointer() else {
        debug!("pointer relative motion ignored: seat has no pointer");
        return;
    };
    let old_pos = pointer.current_location();
    let delta = event.delta();
    let raw_new_pos = old_pos + delta;

    let Some(bounds) = desktop_bounds(state) else {
        debug!(
            "pointer relative motion ignored reason=no-desktop-bounds dx={:.3} dy={:.3}",
            delta.x, delta.y
        );
        return;
    };
    let (new_pos, clamped) = clamp_point_to_desktop_bounds(raw_new_pos, bounds);

    if output_id_at_point_for_focus(&state.output_registry, new_pos.x, new_pos.y).is_some() {
        state.update_focused_output_from_point(new_pos, "pointer-relative-motion", false);
    }
    let (selected_output, fallback_used) =
        select_output_from_registry_for_point(&state.output_registry, new_pos.x, new_pos.y);
    if let Some(output) = selected_output {
        debug!(
            "pointer relative motion: dx={:.3} dy={:.3} old_x={:.2} old_y={:.2} new_x={:.2} new_y={:.2} selected_output_id={} name={} fallback={} clamped={}",
            delta.x,
            delta.y,
            old_pos.x,
            old_pos.y,
            new_pos.x,
            new_pos.y,
            output.id.0,
            output.name,
            fallback_used,
            clamped
        );
    } else {
        debug!(
            "pointer relative motion: dx={:.3} dy={:.3} old_x={:.2} old_y={:.2} new_x={:.2} new_y={:.2} selected_output=none fallback=true clamped={}",
            delta.x, delta.y, old_pos.x, old_pos.y, new_pos.x, new_pos.y, clamped
        );
    }
    if fallback_used {
        debug!("pointer relative motion fallback: no output contains point");
    }
    update_hover_cursor_feedback(state, new_pos);

    let serial = SERIAL_COUNTER.next_serial();
    let under = state.surface_under(new_pos);
    pointer.motion(
        state,
        under,
        &MotionEvent {
            location: new_pos,
            serial,
            time: event.time_msec(),
        },
    );
    pointer.frame(state);
}

const CURSOR_EW_RESIZE_NAMES: &[&str] = &[
    "ew-resize",
    "size_hor",
    "sb_h_double_arrow",
    "h_double_arrow",
    "col-resize",
];
const CURSOR_NS_RESIZE_NAMES: &[&str] = &[
    "ns-resize",
    "size_ver",
    "sb_v_double_arrow",
    "v_double_arrow",
    "row-resize",
];
const CURSOR_NESW_RESIZE_NAMES: &[&str] = &[
    "nesw-resize",
    "size_bdiag",
    "bottom_left_corner",
    "sw-resize",
];
const CURSOR_NWSE_RESIZE_NAMES: &[&str] = &[
    "nwse-resize",
    "size_fdiag",
    "bottom_right_corner",
    "se-resize",
];

// Keep this aligned with SSD resize hit thickness for consistent hover affordance.
const XWAYLAND_EDGE_RESIZE_THICKNESS_PX: i32 = 8;

fn output_geometry_for_rect_center(
    state: &MeridianState,
    rect: Rectangle<i32, Logical>,
) -> Option<OutputGeometry> {
    let center_x = rect.loc.x as f64 + (rect.size.w.max(1) as f64 * 0.5);
    let center_y = rect.loc.y as f64 + (rect.size.h.max(1) as f64 * 0.5);
    state
        .output_registry
        .select_for_point_with_fallback(center_x, center_y)
        .map(|info| info.geometry)
}

fn rect_matches_output_fullscreen_shape(
    rect: Rectangle<i32, Logical>,
    output_geometry: OutputGeometry,
) -> bool {
    rect.loc.x == output_geometry.x
        && rect.loc.y == output_geometry.y
        && rect.size.w == output_geometry.width
        && rect.size.h == output_geometry.height
}

fn xwayland_resize_edge_from_rect(
    rect: Rectangle<i32, Logical>,
    pointer: Point<f64, Logical>,
) -> Option<DecorationResizeEdge> {
    if rect.size.w <= 0 || rect.size.h <= 0 {
        return None;
    }

    let left = rect.loc.x;
    let top = rect.loc.y;
    let right = left + rect.size.w;
    let bottom = top + rect.size.h;
    let px = pointer.x as i32;
    let py = pointer.y as i32;

    if px < left || py < top || px >= right || py >= bottom {
        return None;
    }

    let edge_x = XWAYLAND_EDGE_RESIZE_THICKNESS_PX.min((rect.size.w / 2).max(1));
    let edge_y = XWAYLAND_EDGE_RESIZE_THICKNESS_PX.min((rect.size.h / 2).max(1));
    let hit_left = px < left + edge_x;
    let hit_right = px >= right - edge_x;
    let hit_top = py < top + edge_y;
    let hit_bottom = py >= bottom - edge_y;

    match (hit_left, hit_right, hit_top, hit_bottom) {
        (true, _, true, _) => Some(DecorationResizeEdge::TopLeft),
        (_, true, true, _) => Some(DecorationResizeEdge::TopRight),
        (true, _, _, true) => Some(DecorationResizeEdge::BottomLeft),
        (_, true, _, true) => Some(DecorationResizeEdge::BottomRight),
        (true, _, _, _) => Some(DecorationResizeEdge::Left),
        (_, true, _, _) => Some(DecorationResizeEdge::Right),
        (_, _, true, _) => Some(DecorationResizeEdge::Top),
        (_, _, _, true) => Some(DecorationResizeEdge::Bottom),
        _ => None,
    }
}

pub(super) fn xwayland_resize_edge_hit_for_pointer(
    state: &MeridianState,
    location: Point<f64, Logical>,
) -> Option<(
    Window,
    DecorationResizeEdge,
    smithay::utils::Point<i32, Logical>,
)> {
    let space = state.workspaces.active_space();
    let (window, window_loc) = space.element_under(location)?;
    let x11 = window.x11_surface()?;
    if x11.is_override_redirect() {
        return None;
    }

    let rect = Rectangle::new(window_loc, window.geometry().size);
    if output_geometry_for_rect_center(state, rect)
        .is_some_and(|output_geometry| rect_matches_output_fullscreen_shape(rect, output_geometry))
    {
        return None;
    }

    let edge = xwayland_resize_edge_from_rect(rect, location)?;
    Some((window.clone(), edge, window_loc))
}

fn decoration_hit_for_pointer(
    state: &MeridianState,
    location: Point<f64, Logical>,
) -> Option<DecorationHit> {
    let space = state.workspaces.active_space();
    let theme = &state.theme_manager.current().config.decorations;

    let hit_for_window = |window: &Window, window_loc: smithay::utils::Point<i32, Logical>| {
        let wl_surf = window.wl_surface()?.into_owned();
        let content_size = window.geometry().size;
        state
            .decoration_manager
            .hit_test(&wl_surf, location, window_loc, content_size, theme)
    };

    space
        .element_under(location)
        .and_then(|(window, window_loc)| hit_for_window(window, window_loc))
        .or_else(|| {
            let windows: Vec<_> = space.elements().cloned().collect();
            windows.iter().rev().find_map(|window| {
                let window_loc = space.element_location(window)?;
                hit_for_window(window, window_loc)
            })
        })
}

fn cursor_icon_for_decoration_hit(hit: Option<DecorationHit>) -> DrmCursorIcon {
    match hit {
        Some(DecorationHit::Resize(DecorationResizeEdge::Left))
        | Some(DecorationHit::Resize(DecorationResizeEdge::Right)) => DrmCursorIcon::EwResize,
        Some(DecorationHit::Resize(DecorationResizeEdge::Top))
        | Some(DecorationHit::Resize(DecorationResizeEdge::Bottom)) => DrmCursorIcon::NsResize,
        Some(DecorationHit::Resize(DecorationResizeEdge::TopLeft)) => DrmCursorIcon::NwseResize,
        Some(DecorationHit::Resize(DecorationResizeEdge::TopRight)) => DrmCursorIcon::NeswResize,
        Some(DecorationHit::Resize(DecorationResizeEdge::BottomLeft)) => DrmCursorIcon::NeswResize,
        Some(DecorationHit::Resize(DecorationResizeEdge::BottomRight)) => DrmCursorIcon::NwseResize,
        _ => DrmCursorIcon::Default,
    }
}

fn cursor_icon_for_resize_edge(edge: DecorationResizeEdge) -> DrmCursorIcon {
    cursor_icon_for_decoration_hit(Some(DecorationHit::Resize(edge)))
}

fn update_hover_cursor_feedback(state: &mut MeridianState, location: Point<f64, Logical>) {
    let decoration_hit = decoration_hit_for_pointer(state, location);
    let desired_cursor = match decoration_hit {
        Some(DecorationHit::Resize(edge)) => cursor_icon_for_resize_edge(edge),
        _ => xwayland_resize_edge_hit_for_pointer(state, location)
            .map(|(_, edge, _)| cursor_icon_for_resize_edge(edge))
            .unwrap_or(DrmCursorIcon::Default),
    };
    let hovered_button = match decoration_hit {
        Some(DecorationHit::CloseButton) => Some(HoveredButton::Close),
        Some(DecorationHit::MaximizeButton) => Some(HoveredButton::Maximize),
        Some(DecorationHit::MinimizeButton) => Some(HoveredButton::Minimize),
        _ => None,
    };
    let mut hover_changed = state.decoration_manager.clear_hover_buttons();
    if let Some((window, _)) = state.workspaces.active_space().element_under(location) {
        if let (Some(wl_surface), Some(hovered)) = (window.wl_surface(), hovered_button) {
            if state
                .decoration_manager
                .update_hover_button(&wl_surface, Some(hovered))
            {
                hover_changed = true;
            }
        }
    }
    if hover_changed {
        state.mark_all_outputs_dirty("decoration-button-hover-change");
        tracing::info!(
            "decoration hover change: hovered_button={:?}",
            hovered_button
        );
    }
    // TODO(phase-3): Motion updates hover state, but non-motion pointer leave still needs explicit clear.

    let cursor_cfg = &state.theme_manager.current().config.cursor;
    let cursor_theme = std::env::var("XCURSOR_THEME").unwrap_or_else(|_| cursor_cfg.theme.clone());
    let cursor_size = std::env::var("XCURSOR_SIZE")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(cursor_cfg.size);

    let mut changed = false;
    if let Some(drm) = state.drm_backend.as_mut() {
        if drm.cursor_icon != desired_cursor {
            let new_cursor = match desired_cursor {
                DrmCursorIcon::Default => CursorImage::load_theme(&cursor_theme, cursor_size),
                DrmCursorIcon::EwResize => {
                    CursorImage::load_theme_icon(&cursor_theme, cursor_size, CURSOR_EW_RESIZE_NAMES)
                }
                DrmCursorIcon::NsResize => {
                    CursorImage::load_theme_icon(&cursor_theme, cursor_size, CURSOR_NS_RESIZE_NAMES)
                }
                DrmCursorIcon::NeswResize => CursorImage::load_theme_icon(
                    &cursor_theme,
                    cursor_size,
                    CURSOR_NESW_RESIZE_NAMES,
                ),
                DrmCursorIcon::NwseResize => CursorImage::load_theme_icon(
                    &cursor_theme,
                    cursor_size,
                    CURSOR_NWSE_RESIZE_NAMES,
                ),
            };
            drm.cursor_buffer = new_cursor.to_memory_buffer();
            drm.cursor_image = new_cursor;
            drm.cursor_icon = desired_cursor;
            changed = true;
        }
    }

    if changed {
        state.mark_all_outputs_dirty("pointer-cursor-icon-change");
    }
}

fn desktop_bounds(state: &MeridianState) -> Option<(i32, i32, i32, i32)> {
    let mut iter = state.output_registry.list().iter();
    let first = iter.next()?;
    let mut left = first.geometry.x;
    let mut top = first.geometry.y;
    let mut right = first.geometry.x + first.geometry.width;
    let mut bottom = first.geometry.y + first.geometry.height;

    for output in iter {
        left = left.min(output.geometry.x);
        top = top.min(output.geometry.y);
        right = right.max(output.geometry.x + output.geometry.width);
        bottom = bottom.max(output.geometry.y + output.geometry.height);
    }

    Some((left, top, right - left, bottom - top))
}

fn clamp_point_to_desktop_bounds(
    point: Point<f64, Logical>,
    bounds: (i32, i32, i32, i32),
) -> (Point<f64, Logical>, bool) {
    let (origin_x, origin_y, width, height) = bounds;
    if width <= 0 || height <= 0 {
        return (point, false);
    }
    let min_x = origin_x as f64;
    let min_y = origin_y as f64;
    let max_x = (origin_x + width - 1) as f64;
    let max_y = (origin_y + height - 1) as f64;

    let clamped_x = point.x.clamp(min_x, max_x);
    let clamped_y = point.y.clamp(min_y, max_y);
    let clamped = clamped_x != point.x || clamped_y != point.y;
    ((clamped_x, clamped_y).into(), clamped)
}

fn select_output_from_registry_for_point(
    registry: &OutputRegistry,
    x: f64,
    y: f64,
) -> (Option<&crate::state::OutputInfo>, bool) {
    let fallback_used = registry.output_at_point(x, y).is_none();
    (registry.select_for_point_with_fallback(x, y), fallback_used)
}

pub(super) fn output_id_at_point_for_focus(
    registry: &OutputRegistry,
    x: f64,
    y: f64,
) -> Option<OutputId> {
    registry.output_at_point(x, y).map(|output| output.id)
}

pub fn handle_pointer_axis<I: InputBackend>(
    state: &mut MeridianState,
    event: &impl PointerAxisEvent<I>,
) {
    let source = event.source();

    let h = event
        .amount(Axis::Horizontal)
        .unwrap_or_else(|| event.amount_v120(Axis::Horizontal).unwrap_or(0.0) * 15.0 / 120.0);
    let v = event
        .amount(Axis::Vertical)
        .unwrap_or_else(|| event.amount_v120(Axis::Vertical).unwrap_or(0.0) * 15.0 / 120.0);
    let h120 = event.amount_v120(Axis::Horizontal);
    let v120 = event.amount_v120(Axis::Vertical);

    let mut frame = AxisFrame::new(event.time_msec()).source(source);
    if h != 0.0 {
        frame = frame.value(Axis::Horizontal, h);
        if let Some(d) = h120 {
            frame = frame.v120(Axis::Horizontal, d as i32);
        }
    }
    if v != 0.0 {
        frame = frame.value(Axis::Vertical, v);
        if let Some(d) = v120 {
            frame = frame.v120(Axis::Vertical, d as i32);
        }
    }
    if source == AxisSource::Finger {
        if event.amount(Axis::Horizontal) == Some(0.0) {
            frame = frame.stop(Axis::Horizontal);
        }
        if event.amount(Axis::Vertical) == Some(0.0) {
            frame = frame.stop(Axis::Vertical);
        }
    }

    let Some(pointer) = state.seat.get_pointer() else {
        debug!("pointer axis ignored: seat has no pointer");
        return;
    };
    pointer.axis(state, frame);
    pointer.frame(state);
}

#[cfg(test)]
mod tests {
    use smithay::utils::{Logical, Point, Rectangle, Transform};

    use crate::state::{OutputGeometry, OutputRegistration, OutputRegistry};

    fn reg(name: &str, x: i32, y: i32, width: i32, height: i32) -> OutputRegistration {
        OutputRegistration {
            name: name.to_string(),
            geometry: OutputGeometry {
                x,
                y,
                width,
                height,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
        }
    }

    #[test]
    fn absolute_point_selects_output_one() {
        let mut registry = OutputRegistry::new();
        registry.upsert(reg("left", 0, 0, 1920, 1080));
        registry.upsert(reg("right", 1920, 0, 2560, 1440));
        let (output, fallback) =
            super::select_output_from_registry_for_point(&registry, 100.0, 200.0);
        assert_eq!(output.map(|o| o.name.as_str()), Some("left"));
        assert!(!fallback);
    }

    #[test]
    fn absolute_point_selects_output_two() {
        let mut registry = OutputRegistry::new();
        registry.upsert(reg("left", 0, 0, 1920, 1080));
        registry.upsert(reg("right", 1920, 0, 2560, 1440));
        let (output, fallback) =
            super::select_output_from_registry_for_point(&registry, 2300.0, 100.0);
        assert_eq!(output.map(|o| o.name.as_str()), Some("right"));
        assert!(!fallback);
    }

    #[test]
    fn resize_hit_maps_to_expected_cursor_icons() {
        assert_eq!(
            super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::Resize(
                crate::decoration::DecorationResizeEdge::Left,
            ))),
            crate::backend::drm::DrmCursorIcon::EwResize
        );
        assert_eq!(
            super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::Resize(
                crate::decoration::DecorationResizeEdge::Right,
            ))),
            crate::backend::drm::DrmCursorIcon::EwResize
        );
        assert_eq!(
            super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::Resize(
                crate::decoration::DecorationResizeEdge::Top,
            ))),
            crate::backend::drm::DrmCursorIcon::NsResize
        );
        assert_eq!(
            super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::Resize(
                crate::decoration::DecorationResizeEdge::Bottom,
            ))),
            crate::backend::drm::DrmCursorIcon::NsResize
        );
        assert_eq!(
            super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::Resize(
                crate::decoration::DecorationResizeEdge::TopLeft,
            ))),
            crate::backend::drm::DrmCursorIcon::NwseResize
        );
        assert_eq!(
            super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::Resize(
                crate::decoration::DecorationResizeEdge::TopRight,
            ))),
            crate::backend::drm::DrmCursorIcon::NeswResize
        );
        assert_eq!(
            super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::Resize(
                crate::decoration::DecorationResizeEdge::BottomLeft,
            ))),
            crate::backend::drm::DrmCursorIcon::NeswResize
        );
        assert_eq!(
            super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::Resize(
                crate::decoration::DecorationResizeEdge::BottomRight,
            ))),
            crate::backend::drm::DrmCursorIcon::NwseResize
        );
    }

    #[test]
    fn non_resize_hit_maps_to_default_cursor_icon() {
        assert_eq!(
            super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::TitleBar)),
            crate::backend::drm::DrmCursorIcon::Default
        );
        assert_eq!(
            super::cursor_icon_for_decoration_hit(None),
            crate::backend::drm::DrmCursorIcon::Default
        );
    }

    #[test]
    fn absolute_point_outside_uses_primary_fallback() {
        let mut registry = OutputRegistry::new();
        registry.upsert(reg("primary", 0, 0, 1920, 1080));
        registry.upsert(reg("second", 1920, 0, 2560, 1440));
        let (output, fallback) =
            super::select_output_from_registry_for_point(&registry, -100.0, -100.0);
        assert_eq!(output.map(|o| o.name.as_str()), Some("primary"));
        assert!(fallback);
    }

    #[test]
    fn focus_update_candidate_is_none_outside_outputs() {
        let mut registry = OutputRegistry::new();
        registry.upsert(reg("left", 0, 0, 1920, 1080));
        registry.upsert(reg("right", 1920, 0, 2560, 1440));
        assert!(super::output_id_at_point_for_focus(&registry, -10.0, 0.0).is_none());
    }

    #[test]
    fn relative_clamp_keeps_point_inside_bounds() {
        let bounds = (0, 0, 1920, 1080);
        let point: Point<f64, Logical> = (2500.0, 1500.0).into();
        let (clamped, was_clamped) = super::clamp_point_to_desktop_bounds(point, bounds);
        assert!(was_clamped);
        assert_eq!(clamped.x, 1919.0);
        assert_eq!(clamped.y, 1079.0);
    }

    #[test]
    fn relative_clamp_noop_when_inside_bounds() {
        let bounds = (0, 0, 1920, 1080);
        let point: Point<f64, Logical> = (1200.0, 800.0).into();
        let (clamped, was_clamped) = super::clamp_point_to_desktop_bounds(point, bounds);
        assert!(!was_clamped);
        assert_eq!(clamped, point);
    }

    #[test]
    fn xwayland_edge_hit_detects_corners_and_edges() {
        let rect: Rectangle<i32, Logical> = Rectangle::new((100, 100).into(), (400, 300).into());

        assert_eq!(
            super::xwayland_resize_edge_from_rect(rect, (101.0, 101.0).into()),
            Some(crate::decoration::DecorationResizeEdge::TopLeft)
        );
        assert_eq!(
            super::xwayland_resize_edge_from_rect(rect, (498.0, 101.0).into()),
            Some(crate::decoration::DecorationResizeEdge::TopRight)
        );
        assert_eq!(
            super::xwayland_resize_edge_from_rect(rect, (101.0, 398.0).into()),
            Some(crate::decoration::DecorationResizeEdge::BottomLeft)
        );
        assert_eq!(
            super::xwayland_resize_edge_from_rect(rect, (498.0, 398.0).into()),
            Some(crate::decoration::DecorationResizeEdge::BottomRight)
        );
        assert_eq!(
            super::xwayland_resize_edge_from_rect(rect, (101.0, 250.0).into()),
            Some(crate::decoration::DecorationResizeEdge::Left)
        );
        assert_eq!(
            super::xwayland_resize_edge_from_rect(rect, (498.0, 250.0).into()),
            Some(crate::decoration::DecorationResizeEdge::Right)
        );
        assert_eq!(
            super::xwayland_resize_edge_from_rect(rect, (250.0, 101.0).into()),
            Some(crate::decoration::DecorationResizeEdge::Top)
        );
        assert_eq!(
            super::xwayland_resize_edge_from_rect(rect, (250.0, 398.0).into()),
            Some(crate::decoration::DecorationResizeEdge::Bottom)
        );
    }

    #[test]
    fn xwayland_edge_hit_ignores_interior_and_outside_points() {
        let rect: Rectangle<i32, Logical> = Rectangle::new((50, 50).into(), (300, 200).into());

        assert_eq!(
            super::xwayland_resize_edge_from_rect(rect, (200.0, 150.0).into()),
            None
        );
        assert_eq!(
            super::xwayland_resize_edge_from_rect(rect, (49.0, 150.0).into()),
            None
        );
    }
}
