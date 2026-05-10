use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, InputBackend, PointerAxisEvent, PointerMotionEvent,
    },
    input::pointer::{AxisFrame, MotionEvent},
    utils::SERIAL_COUNTER,
    utils::{Logical, Point, Size},
};
use tracing::debug;

use crate::state::{MeridianState, OutputId, OutputRegistry};

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

    let serial = SERIAL_COUNTER.next_serial();
    let pointer = state.seat.get_pointer().unwrap();
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
    let pointer = state.seat.get_pointer().unwrap();
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
    if let Some(output) = registry.output_at_point(x, y) {
        return (Some(output), false);
    }
    registry
        .primary()
        .or_else(|| registry.first())
        .map(|output| (Some(output), true))
        .unwrap_or((None, true))
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

    let pointer = state.seat.get_pointer().unwrap();
    pointer.axis(state, frame);
    pointer.frame(state);
}

#[cfg(test)]
mod tests {
    use smithay::utils::{Logical, Point, Transform};

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
}
