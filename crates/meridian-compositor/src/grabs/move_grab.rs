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
    utils::{Logical, Point, Size},
    wayland::shell::xdg::XdgShellHandler,
};

use crate::state::{window_id, MeridianState, OutputGeometry};

const TOP_EDGE_MAXIMIZE_THRESHOLD_PX: f64 = 12.0;
const DRAG_RESTORE_THRESHOLD_PX: f64 = 8.0;

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

fn movement_crosses_restore_threshold(
    start: Point<f64, Logical>,
    current: Point<f64, Logical>,
) -> bool {
    let dx = current.x - start.x;
    let dy = current.y - start.y;
    dx.hypot(dy) >= DRAG_RESTORE_THRESHOLD_PX
}

fn pointer_ratio_within_frame_x(pointer_x: f64, frame_left: i32, frame_width: i32) -> f64 {
    if frame_width <= 0 {
        return 0.5;
    }
    ((pointer_x - frame_left as f64) / frame_width as f64).clamp(0.0, 1.0)
}

fn anchored_client_location_from_pointer(
    pointer: Point<f64, Logical>,
    pointer_frame_offset_y: f64,
    frame_ratio_x: f64,
    client_size: Size<i32, Logical>,
    frame_insets: (i32, i32, i32, i32),
) -> Point<i32, Logical> {
    let (left, top, right, _bottom) = frame_insets;
    let frame_width = (client_size.w + left + right).max(1);
    let frame_left = pointer.x - frame_ratio_x * frame_width as f64;
    let frame_top = pointer.y - pointer_frame_offset_y;
    let client_x = frame_left + left as f64;
    let client_y = frame_top + top as f64;
    Point::from((client_x.round() as i32, client_y.round() as i32))
}

fn maybe_restore_maximized_drag(
    data: &mut MeridianState,
    window: &Window,
    initial_window_location: Point<i32, Logical>,
    drag_start_location: Point<f64, Logical>,
    current_pointer_location: Point<f64, Logical>,
) -> Option<Point<i32, Logical>> {
    let toplevel = window.toplevel()?;
    if !movement_crosses_restore_threshold(drag_start_location, current_pointer_location) {
        return None;
    }

    let theme = data.theme_manager.current().config.decorations.clone();
    let maximized_insets = data
        .decoration_manager
        .decoration_inset(toplevel.wl_surface(), &theme);
    let maximized_frame_left = initial_window_location.x - maximized_insets.0;
    let maximized_frame_top = initial_window_location.y - maximized_insets.1;
    let maximized_frame_width =
        (window.geometry().size.w + maximized_insets.0 + maximized_insets.2).max(1);
    let pointer_ratio_x = pointer_ratio_within_frame_x(
        drag_start_location.x,
        maximized_frame_left,
        maximized_frame_width,
    );
    let pointer_frame_offset_y = drag_start_location.y - maximized_frame_top as f64;

    let restore_geometry = data
        .maximize_restore_locations
        .get(&window_id(toplevel.wl_surface()))
        .copied();
    let restore_client_size = restore_geometry
        .and_then(|geometry| geometry.client_size)
        .unwrap_or(window.geometry().size);

    XdgShellHandler::unmaximize_request(data, toplevel.clone());

    let floating_insets = data
        .decoration_manager
        .decoration_inset(toplevel.wl_surface(), &theme);
    Some(anchored_client_location_from_pointer(
        current_pointer_location,
        pointer_frame_offset_y,
        pointer_ratio_x,
        restore_client_size,
        floating_insets,
    ))
}

pub struct MoveSurfaceGrab {
    pub start_data: PointerGrabStartData<MeridianState>,
    pub window: Window,
    pub initial_window_location: Point<i32, Logical>,
    pub latest_pointer_location: Option<Point<f64, Logical>>,
    pub started_maximized: bool,
    pub started_fullscreen: bool,
    pub drag_restore_done: bool,
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

        if self.started_maximized && !self.started_fullscreen && !self.drag_restore_done {
            if let Some(restored_client_location) = maybe_restore_maximized_drag(
                data,
                &self.window,
                self.initial_window_location,
                self.start_data.location,
                event.location,
            ) {
                let delta = event.location - self.start_data.location;
                self.initial_window_location = restored_client_location - delta.to_i32_round();
                self.drag_restore_done = true;
                self.started_maximized = false;
            } else {
                return;
            }
        }

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
    use smithay::utils::{Logical, Point, Size};

    use crate::state::OutputGeometry;

    use super::{
        anchored_client_location_from_pointer, is_pointer_near_output_top_edge,
        movement_crosses_restore_threshold,
    };

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

    #[test]
    fn drag_restore_threshold_requires_real_drag_distance() {
        let start = point(100.0, 100.0);
        let below = point(105.0, 105.0);
        let beyond = point(110.0, 106.0);

        assert!(!movement_crosses_restore_threshold(start, below));
        assert!(movement_crosses_restore_threshold(start, beyond));
    }

    #[test]
    fn anchored_restore_location_preserves_pointer_horizontal_ratio() {
        let pointer = point(960.0, 120.0);
        let pointer_frame_offset_y = 10.0;
        let frame_ratio_x = 0.5;
        let client_size: Size<i32, Logical> = (800, 600).into();
        let insets = (2, 34, 2, 2);

        let client_loc = anchored_client_location_from_pointer(
            pointer,
            pointer_frame_offset_y,
            frame_ratio_x,
            client_size,
            insets,
        );
        assert_eq!(client_loc.x, 560);
        assert_eq!(client_loc.y, 144);
    }
}
