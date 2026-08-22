use std::collections::HashMap;

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
    utils::{Logical, Point, Rectangle, Size},
    wayland::shell::xdg::XdgShellHandler,
};

use crate::protocols::xwayland::{apply_x11_unmaximize, x11_window_key};
use crate::state::{
    clear_tiled_toplevel_states, half_snap_client_placement_from_output,
    maximized_client_loc_from_output, normal_window_workarea_from_output_geometry, window_id,
    HalfSnapDirection, HalfSnapRestoreGeometry, MaximizeRestoreGeometry, MeridianState,
    OutputGeometry, OutputInfo, OutputRegistry, WindowSnapState,
};

const TOP_EDGE_MAXIMIZE_THRESHOLD_PX: f64 = 12.0;
const SIDE_EDGE_SNAP_THRESHOLD_PX: f64 = 12.0;
const DRAG_RESTORE_THRESHOLD_PX: f64 = 8.0;
include!("move_grab/release.rs");
include!("move_grab/restore.rs");
pub struct MoveSurfaceGrab {
    pub start_data: PointerGrabStartData<MeridianState>,
    pub window: Window,
    /// Workspace the window lives on, captured at grab start. The drag maps
    /// the window into THIS space (not the active one), so switching
    /// workspace mid-drag cannot duplicate it across workspaces.
    pub workspace: usize,
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
                self.initial_window_location = restored_initial_window_location(
                    restored_client_location,
                    self.start_data.location,
                    event.location,
                );
                self.drag_restore_done = true;
                self.started_maximized = false;
            } else {
                return;
            }
        }
        if !self.drag_restore_done {
            let started_half_snapped = window_half_snap_direction(data, &self.window).is_some();
            if started_half_snapped {
                if let Some(restored_client_location) = maybe_restore_half_snapped_drag(
                    data,
                    &self.window,
                    self.initial_window_location,
                    self.start_data.location,
                    event.location,
                ) {
                    self.initial_window_location = restored_initial_window_location(
                        restored_client_location,
                        self.start_data.location,
                        event.location,
                    );
                    self.drag_restore_done = true;
                } else {
                    return;
                }
            }
        }
        if !self.drag_restore_done {
            let started_xwayland_snapped = xwayland_restore_window_key(data, &self.window)
                .is_some_and(|window_key| {
                    data.half_snap_restore_locations
                        .contains_key(window_key.as_str())
                });
            if started_xwayland_snapped {
                if let Some(restored_client_location) = maybe_restore_xwayland_snapped_drag(
                    data,
                    &self.window,
                    self.initial_window_location,
                    self.start_data.location,
                    event.location,
                ) {
                    self.initial_window_location = restored_initial_window_location(
                        restored_client_location,
                        self.start_data.location,
                        event.location,
                    );
                    self.drag_restore_done = true;
                } else {
                    return;
                }
            }
        }

        let delta = event.location - self.start_data.location;
        let new_location = self.initial_window_location.to_f64() + delta;
        data.workspaces.space_at_mut(self.workspace).map_element(
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
            let release_action =
                release_edge_action_on_move_release(data, self.latest_pointer_location);
            let should_maximize =
                should_maximize_on_move_release(&self.window, release_action.map(|(_, a)| a));
            handle.unset_grab(self, data, event.serial, event.time, true);
            // Release actions target the active (visible) workspace. If a
            // workspace switch happened mid-drag, the window stayed on its own
            // workspace (see motion); applying active-space actions here would
            // duplicate it, so leave it where it was dragged.
            if data.workspaces.active != self.workspace {
                return;
            }
            if should_maximize {
                maximize_window_from_move_release(data, &self.window);
            } else if let Some((output_geometry, action)) = release_action {
                if !apply_xwayland_snap_from_move_release(
                    data,
                    &self.window,
                    output_geometry,
                    action,
                ) {
                    if let MoveReleaseEdgeAction::HalfSnap(direction) = action {
                        apply_half_snap_from_move_release(
                            data,
                            &self.window,
                            output_geometry,
                            direction,
                        );
                    }
                }
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
#[path = "move_grab_tests.rs"]
mod tests;
