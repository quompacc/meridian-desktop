use smithay::{
    desktop::Window,
    input::pointer::{
        AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
        GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent,
        GestureSwipeEndEvent, GestureSwipeUpdateEvent, GrabStartData as PointerGrabStartData,
        MotionEvent, PointerGrab, PointerInnerHandle, RelativeMotionEvent,
    },
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
    wayland::shell::xdg::XdgShellHandler,
};

use crate::state::{MeridianState, OutputGeometry};

const TOP_EDGE_MAXIMIZE_THRESHOLD_PX: f64 = 12.0;

fn is_pointer_near_output_top_edge(
    output_geometry: OutputGeometry,
    pointer_location: Point<f64, Logical>,
) -> bool {
    output_geometry.contains(pointer_location.x, pointer_location.y)
        && pointer_location.y < output_geometry.y as f64 + TOP_EDGE_MAXIMIZE_THRESHOLD_PX
}

fn should_maximize_on_move_release(
    data: &MeridianState,
    window: &Window,
    pointer_location: Option<Point<f64, Logical>>,
) -> bool {
    let Some(pointer_location) = pointer_location else {
        return false;
    };

    let Some(output) = data
        .output_registry
        .output_at_point(pointer_location.x, pointer_location.y)
    else {
        return false;
    };

    if !is_pointer_near_output_top_edge(output.geometry, pointer_location) {
        return false;
    }

    let Some(toplevel) = window.toplevel() else {
        return false;
    };

    let is_fullscreen = toplevel.with_committed_state(|state| {
        state.map_or(false, |toplevel_state| {
            toplevel_state
                .states
                .contains(xdg_toplevel::State::Fullscreen)
        })
    }) || toplevel
        .with_pending_state(|state| state.states.contains(xdg_toplevel::State::Fullscreen));
    if is_fullscreen {
        return false;
    }

    let is_maximized = toplevel.with_committed_state(|state| {
        state.map_or(false, |toplevel_state| {
            toplevel_state
                .states
                .contains(xdg_toplevel::State::Maximized)
        })
    }) || toplevel
        .with_pending_state(|state| state.states.contains(xdg_toplevel::State::Maximized));
    if is_maximized {
        return false;
    }

    true
}

fn maximize_window_from_move_release(data: &mut MeridianState, window: &Window) {
    if let Some(toplevel) = window.toplevel() {
        XdgShellHandler::maximize_request(data, toplevel.clone());
    }
}

pub struct MoveSurfaceGrab {
    pub start_data: PointerGrabStartData<MeridianState>,
    pub window: Window,
    pub initial_window_location: Point<i32, Logical>,
    pub latest_pointer_location: Option<Point<f64, Logical>>,
}

impl PointerGrab<MeridianState> for MoveSurfaceGrab {
    fn motion(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        handle.motion(data, None, event);
        self.latest_pointer_location = Some(event.location);
        let delta = event.location - self.start_data.location;
        let new_location = self.initial_window_location.to_f64() + delta;
        data.workspaces.active_space_mut().map_element(
            self.window.clone(),
            new_location.to_i32_round(),
            true,
        );
    }

    fn relative_motion(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &ButtonEvent,
    ) {
        handle.button(data, event);
        const BTN_LEFT: u32 = 0x110;
        if !handle.current_pressed().contains(&BTN_LEFT) {
            let should_maximize =
                should_maximize_on_move_release(data, &self.window, self.latest_pointer_location);
            handle.unset_grab(self, data, event.serial, event.time, true);
            if should_maximize {
                maximize_window_from_move_release(data, &self.window);
            }
        }
    }

    fn axis(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        details: AxisFrame,
    ) {
        handle.axis(data, details);
    }

    fn frame(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
    ) {
        handle.frame(data);
    }

    fn gesture_swipe_begin(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event);
    }
    fn gesture_swipe_update(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event);
    }
    fn gesture_swipe_end(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event);
    }
    fn gesture_pinch_begin(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event);
    }
    fn gesture_pinch_update(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event);
    }
    fn gesture_pinch_end(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event);
    }
    fn gesture_hold_begin(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event);
    }
    fn gesture_hold_end(
        &mut self,
        data: &mut MeridianState,
        handle: &mut PointerInnerHandle<'_, MeridianState>,
        event: &GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event);
    }

    fn start_data(&self) -> &PointerGrabStartData<MeridianState> {
        &self.start_data
    }

    fn unset(&mut self, _data: &mut MeridianState) {}
}

#[cfg(test)]
mod tests {
    use smithay::utils::{Logical, Point};

    use crate::state::OutputGeometry;

    use super::is_pointer_near_output_top_edge;

    fn point(x: f64, y: f64) -> Point<f64, Logical> {
        Point::from((x, y))
    }

    #[test]
    fn top_edge_threshold_detects_near_top() {
        let output = OutputGeometry {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };

        assert!(is_pointer_near_output_top_edge(output, point(100.0, 6.0)));
    }

    #[test]
    fn top_edge_threshold_rejects_deeper_positions() {
        let output = OutputGeometry {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };

        assert!(!is_pointer_near_output_top_edge(output, point(100.0, 25.0)));
    }

    #[test]
    fn top_edge_threshold_requires_pointer_inside_output() {
        let output = OutputGeometry {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };

        assert!(!is_pointer_near_output_top_edge(output, point(-1.0, 4.0)));
    }
}
