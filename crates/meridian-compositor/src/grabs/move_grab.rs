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

use crate::state::{
    clear_tiled_toplevel_states, half_snap_client_placement_from_output,
    normal_window_workarea_from_output_geometry, window_id, HalfSnapDirection,
    HalfSnapRestoreGeometry, MaximizeRestoreGeometry, MeridianState, OutputGeometry, OutputInfo,
    OutputRegistry, WindowSnapState,
};

const TOP_EDGE_MAXIMIZE_THRESHOLD_PX: f64 = 12.0;
const SIDE_EDGE_SNAP_THRESHOLD_PX: f64 = 12.0;
const DRAG_RESTORE_THRESHOLD_PX: f64 = 8.0;

fn is_pointer_near_output_top_edge(
    output_geometry: OutputGeometry,
    pointer_location: Point<f64, Logical>,
) -> bool {
    output_geometry.contains(pointer_location.x, pointer_location.y)
        && pointer_location.y < output_geometry.y as f64 + TOP_EDGE_MAXIMIZE_THRESHOLD_PX
}

fn is_pointer_near_output_left_edge(
    output_geometry: OutputGeometry,
    pointer_location: Point<f64, Logical>,
) -> bool {
    output_geometry.contains(pointer_location.x, pointer_location.y)
        && pointer_location.x < output_geometry.x as f64 + SIDE_EDGE_SNAP_THRESHOLD_PX
}

fn is_pointer_near_output_right_edge(
    output_geometry: OutputGeometry,
    pointer_location: Point<f64, Logical>,
) -> bool {
    let output_right = output_geometry.x as f64 + output_geometry.width as f64;
    output_geometry.contains(pointer_location.x, pointer_location.y)
        && pointer_location.x >= output_right - SIDE_EDGE_SNAP_THRESHOLD_PX
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MoveReleaseEdgeAction {
    Maximize,
    HalfSnap(HalfSnapDirection),
}

fn release_edge_action_for_output(
    output_geometry: OutputGeometry,
    pointer_location: Point<f64, Logical>,
) -> Option<MoveReleaseEdgeAction> {
    if is_pointer_near_output_top_edge(output_geometry, pointer_location) {
        return Some(MoveReleaseEdgeAction::Maximize);
    }
    if is_pointer_near_output_left_edge(output_geometry, pointer_location) {
        return Some(MoveReleaseEdgeAction::HalfSnap(HalfSnapDirection::Left));
    }
    if is_pointer_near_output_right_edge(output_geometry, pointer_location) {
        return Some(MoveReleaseEdgeAction::HalfSnap(HalfSnapDirection::Right));
    }
    None
}

fn select_move_release_output(
    registry: &OutputRegistry,
    pointer_location: Point<f64, Logical>,
) -> Option<&OutputInfo> {
    registry.select_for_point_with_fallback(pointer_location.x, pointer_location.y)
}

fn move_release_workarea_geometry(output_geometry: OutputGeometry) -> OutputGeometry {
    normal_window_workarea_from_output_geometry(output_geometry)
}

fn release_edge_action_on_move_release(
    data: &MeridianState,
    pointer_location: Option<Point<f64, Logical>>,
) -> Option<(OutputGeometry, MoveReleaseEdgeAction)> {
    let pointer_location = pointer_location?;
    let output = select_move_release_output(&data.output_registry, pointer_location)?;
    let workarea = move_release_workarea_geometry(output.geometry);
    let action = release_edge_action_for_output(workarea, pointer_location)?;
    Some((workarea, action))
}

fn should_maximize_on_move_release(window: &Window, action: Option<MoveReleaseEdgeAction>) -> bool {
    if !matches!(action, Some(MoveReleaseEdgeAction::Maximize)) {
        return false;
    }

    let Some(toplevel) = window.toplevel() else {
        return false;
    };

    let is_fullscreen = toplevel.with_committed_state(|state| {
        state.is_some_and(|toplevel_state| {
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
        state.is_some_and(|toplevel_state| {
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

fn apply_half_snap_tiled_states(
    state: &mut smithay::wayland::shell::xdg::ToplevelState,
    direction: HalfSnapDirection,
) {
    clear_tiled_toplevel_states(state);
    state.states.set(xdg_toplevel::State::TiledTop);
    state.states.set(xdg_toplevel::State::TiledBottom);
    match direction {
        HalfSnapDirection::Left => {
            state.states.set(xdg_toplevel::State::TiledLeft);
        }
        HalfSnapDirection::Right => {
            state.states.set(xdg_toplevel::State::TiledRight);
        }
    };
}

fn apply_half_snap_from_move_release(
    data: &mut MeridianState,
    window: &Window,
    output_geometry: OutputGeometry,
    direction: HalfSnapDirection,
) {
    let Some(toplevel) = window.toplevel() else {
        return;
    };

    let key = window_id(toplevel.wl_surface());
    let maximize_restore = data.maximize_restore_locations.get(&key).copied();
    let current_loc = data.workspaces.active_space().element_location(window);
    if let Some(restore_geometry) =
        half_snap_restore_geometry_source(maximize_restore, current_loc, window.geometry().size)
    {
        data.half_snap_restore_locations
            .entry(key.clone())
            .or_insert(restore_geometry);
    }
    data.active_window_snap_states
        .insert(key, WindowSnapState::Half(direction));

    let theme = data.theme_manager.current().config.decorations.clone();
    let decoration_offset = data
        .decoration_manager
        .decoration_offset(toplevel.wl_surface(), &theme);
    let decoration_inset = data
        .decoration_manager
        .decoration_inset(toplevel.wl_surface(), &theme);
    let placement = half_snap_client_placement_from_output(
        output_geometry,
        direction,
        decoration_offset,
        decoration_inset,
    );

    toplevel.with_pending_state(|state| {
        state.states.unset(xdg_toplevel::State::Maximized);
        apply_half_snap_tiled_states(state, direction);
        state.size = Some(placement.client_size);
    });
    data.decoration_manager
        .set_maximized(toplevel.wl_surface(), false);
    data.workspaces
        .active_space_mut()
        .map_element(window.clone(), placement.client_loc, true);
    toplevel.send_pending_configure();
}

fn select_output_geometry_for_rect_center(
    data: &MeridianState,
    rect: Rectangle<i32, Logical>,
) -> Option<OutputGeometry> {
    let center_x = rect.loc.x as f64 + (rect.size.w.max(1) as f64 * 0.5);
    let center_y = rect.loc.y as f64 + (rect.size.h.max(1) as f64 * 0.5);
    data.output_registry
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

fn window_is_output_fullscreen_shape(data: &MeridianState, window: &Window) -> bool {
    let Some(window_loc) = data.workspaces.active_space().element_location(window) else {
        return false;
    };
    let rect = Rectangle::new(window_loc, window.geometry().size);
    select_output_geometry_for_rect_center(data, rect)
        .is_some_and(|output_geometry| rect_matches_output_fullscreen_shape(rect, output_geometry))
}

fn xwayland_snap_rect_for_action(
    output_geometry: OutputGeometry,
    action: MoveReleaseEdgeAction,
) -> Rectangle<i32, Logical> {
    match action {
        MoveReleaseEdgeAction::Maximize => Rectangle::new(
            (output_geometry.x, output_geometry.y).into(),
            (output_geometry.width, output_geometry.height).into(),
        ),
        MoveReleaseEdgeAction::HalfSnap(direction) => {
            let left_width = output_geometry.width / 2;
            let (x, width) = match direction {
                HalfSnapDirection::Left => (output_geometry.x, left_width),
                HalfSnapDirection::Right => (
                    output_geometry.x + left_width,
                    output_geometry.width - left_width,
                ),
            };
            Rectangle::new(
                (x, output_geometry.y).into(),
                (width.max(1), output_geometry.height.max(1)).into(),
            )
        }
    }
}

fn apply_xwayland_snap_from_move_release(
    data: &mut MeridianState,
    window: &Window,
    output_geometry: OutputGeometry,
    action: MoveReleaseEdgeAction,
) -> bool {
    let Some(x11) = window.x11_surface() else {
        return false;
    };
    if x11.is_override_redirect() || window_is_output_fullscreen_shape(data, window) {
        return false;
    }

    let target_rect = xwayland_snap_rect_for_action(output_geometry, action);
    if let Err(err) = x11.configure(target_rect) {
        tracing::error!("xwayland move-release snap configure failed: {}", err);
        return false;
    }

    data.workspaces
        .active_space_mut()
        .map_element(window.clone(), target_rect.loc, true);
    data.mark_all_outputs_dirty("xwayland-move-release-snap");
    true
}

fn half_snap_restore_geometry_source(
    maximize_restore: Option<MaximizeRestoreGeometry>,
    current_client_loc: Option<Point<i32, Logical>>,
    current_client_size: Size<i32, Logical>,
) -> Option<HalfSnapRestoreGeometry> {
    if let Some(geometry) = maximize_restore {
        return Some(HalfSnapRestoreGeometry::new(
            geometry.client_loc,
            geometry.client_size,
        ));
    }

    current_client_loc.map(|loc| HalfSnapRestoreGeometry::new(loc, Some(current_client_size)))
}

fn movement_crosses_restore_threshold(
    start: Point<f64, Logical>,
    current: Point<f64, Logical>,
) -> bool {
    let dx = current.x - start.x;
    let dy = current.y - start.y;
    dx.hypot(dy) >= DRAG_RESTORE_THRESHOLD_PX
}

fn restored_initial_window_location(
    restored_client_location: Point<i32, Logical>,
    drag_start_location: Point<f64, Logical>,
    current_pointer_location: Point<f64, Logical>,
) -> Point<i32, Logical> {
    let delta = current_pointer_location - drag_start_location;
    restored_client_location - delta.to_i32_round()
}

fn pointer_ratio_within_frame_x(pointer_x: f64, frame_left: i32, frame_width: i32) -> f64 {
    if frame_width <= 0 {
        return 0.5;
    }
    ((pointer_x - frame_left as f64) / frame_width as f64).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DragRestorePointerAnchor {
    pointer_frame_ratio_x: f64,
    pointer_frame_offset_y: f64,
}

fn frame_geometry_from_client(
    client_loc: Point<i32, Logical>,
    client_size: Size<i32, Logical>,
    frame_insets: (i32, i32, i32, i32),
) -> (i32, i32, i32) {
    let (left, top, right, _bottom) = frame_insets;
    let frame_left = client_loc.x - left;
    let frame_top = client_loc.y - top;
    let frame_width = (client_size.w + left + right).max(1);
    (frame_left, frame_top, frame_width)
}

fn drag_restore_anchor_from_start_pointer(
    drag_start_pointer: Point<f64, Logical>,
    maximized_client_loc: Point<i32, Logical>,
    maximized_client_size: Size<i32, Logical>,
    maximized_frame_insets: (i32, i32, i32, i32),
) -> DragRestorePointerAnchor {
    let (maximized_frame_left, maximized_frame_top, maximized_frame_width) =
        frame_geometry_from_client(
            maximized_client_loc,
            maximized_client_size,
            maximized_frame_insets,
        );
    DragRestorePointerAnchor {
        pointer_frame_ratio_x: pointer_ratio_within_frame_x(
            drag_start_pointer.x,
            maximized_frame_left,
            maximized_frame_width,
        ),
        pointer_frame_offset_y: drag_start_pointer.y - maximized_frame_top as f64,
    }
}

fn anchored_client_location_from_pointer(
    pointer: Point<f64, Logical>,
    pointer_frame_offset_y: f64,
    frame_ratio_x: f64,
    client_size: Size<i32, Logical>,
    frame_insets: (i32, i32, i32, i32),
) -> Point<i32, Logical> {
    let (frame_left_inset, frame_top_inset, _right, _bottom) = frame_insets;
    let (_, _, frame_width) = frame_geometry_from_client((0, 0).into(), client_size, frame_insets);
    let frame_left = pointer.x - frame_ratio_x * frame_width as f64;
    let frame_top = pointer.y - pointer_frame_offset_y;
    let client_x = frame_left + frame_left_inset as f64;
    let client_y = frame_top + frame_top_inset as f64;
    Point::from((client_x.round() as i32, client_y.round() as i32))
}

fn anchored_restore_client_location(
    pointer: Point<f64, Logical>,
    anchor: DragRestorePointerAnchor,
    restore_client_size: Size<i32, Logical>,
    floating_insets: (i32, i32, i32, i32),
) -> Point<i32, Logical> {
    anchored_client_location_from_pointer(
        pointer,
        anchor.pointer_frame_offset_y,
        anchor.pointer_frame_ratio_x,
        restore_client_size,
        floating_insets,
    )
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
    let anchor = drag_restore_anchor_from_start_pointer(
        drag_start_location,
        initial_window_location,
        window.geometry().size,
        maximized_insets,
    );

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
    Some(anchored_restore_client_location(
        current_pointer_location,
        anchor,
        restore_client_size,
        floating_insets,
    ))
}

fn window_half_snap_direction(
    data: &MeridianState,
    window: &Window,
) -> Option<(String, HalfSnapDirection)> {
    let toplevel = window.toplevel()?;
    let key = window_id(toplevel.wl_surface());
    data.active_window_snap_states
        .get(&key)
        .copied()
        .map(|state| match state {
            WindowSnapState::Half(direction) => (key, direction),
        })
}

fn consume_half_snap_restore_geometry(
    restore_map: &mut HashMap<String, HalfSnapRestoreGeometry>,
    window_key: &str,
    fallback_client_loc: Point<i32, Logical>,
    fallback_client_size: Size<i32, Logical>,
) -> HalfSnapRestoreGeometry {
    restore_map.remove(window_key).unwrap_or_else(|| {
        HalfSnapRestoreGeometry::new(fallback_client_loc, Some(fallback_client_size))
    })
}

fn apply_half_snap_drag_restore_states(
    state: &mut smithay::wayland::shell::xdg::ToplevelState,
    restore_client_size: Size<i32, Logical>,
) {
    state.states.unset(xdg_toplevel::State::Maximized);
    clear_tiled_toplevel_states(state);
    state.size = Some(restore_client_size);
}

fn maybe_restore_half_snapped_drag(
    data: &mut MeridianState,
    window: &Window,
    initial_window_location: Point<i32, Logical>,
    drag_start_location: Point<f64, Logical>,
    current_pointer_location: Point<f64, Logical>,
) -> Option<Point<i32, Logical>> {
    let toplevel = window.toplevel()?;
    let (window_key, _direction) = window_half_snap_direction(data, window)?;
    if !movement_crosses_restore_threshold(drag_start_location, current_pointer_location) {
        return None;
    }

    let theme = data.theme_manager.current().config.decorations.clone();
    let snapped_insets = data
        .decoration_manager
        .decoration_inset(toplevel.wl_surface(), &theme);
    let anchor = drag_restore_anchor_from_start_pointer(
        drag_start_location,
        initial_window_location,
        window.geometry().size,
        snapped_insets,
    );

    let restore_geometry = consume_half_snap_restore_geometry(
        &mut data.half_snap_restore_locations,
        &window_key,
        initial_window_location,
        window.geometry().size,
    );
    let restore_client_size = restore_geometry
        .client_size
        .unwrap_or(window.geometry().size);

    data.active_window_snap_states.remove(&window_key);
    toplevel.with_pending_state(|state| {
        apply_half_snap_drag_restore_states(state, restore_client_size);
    });
    toplevel.send_pending_configure();

    let floating_insets = data
        .decoration_manager
        .decoration_inset(toplevel.wl_surface(), &theme);
    Some(anchored_restore_client_location(
        current_pointer_location,
        anchor,
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
            let release_action =
                release_edge_action_on_move_release(data, self.latest_pointer_location);
            let should_maximize =
                should_maximize_on_move_release(&self.window, release_action.map(|(_, a)| a));
            handle.unset_grab(self, data, event.serial, event.time, true);
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
mod tests {
    use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
    use smithay::utils::{Logical, Point, Size};

    use crate::state::{HalfSnapRestoreGeometry, MaximizeRestoreGeometry, OutputGeometry};

    use super::{
        anchored_client_location_from_pointer, apply_half_snap_drag_restore_states,
        apply_half_snap_tiled_states, consume_half_snap_restore_geometry,
        drag_restore_anchor_from_start_pointer, half_snap_restore_geometry_source,
        is_pointer_near_output_top_edge, move_release_workarea_geometry,
        movement_crosses_restore_threshold, release_edge_action_for_output, HalfSnapDirection,
        MoveReleaseEdgeAction,
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

    #[test]
    fn drag_restore_anchor_clamps_pointer_ratio_near_left_edge() {
        let drag_start_pointer = point(-20.0, 100.0);
        let maximized_client_loc: Point<i32, Logical> = (0, 32).into();
        let maximized_client_size: Size<i32, Logical> = (1920, 1048).into();
        let maximized_insets = (0, 32, 0, 0);

        let anchor = drag_restore_anchor_from_start_pointer(
            drag_start_pointer,
            maximized_client_loc,
            maximized_client_size,
            maximized_insets,
        );
        assert_eq!(anchor.pointer_frame_ratio_x, 0.0);
        assert_eq!(anchor.pointer_frame_offset_y, 100.0);
    }

    #[test]
    fn drag_restore_anchor_clamps_pointer_ratio_near_right_edge() {
        let drag_start_pointer = point(2500.0, 100.0);
        let maximized_client_loc: Point<i32, Logical> = (0, 32).into();
        let maximized_client_size: Size<i32, Logical> = (1920, 1048).into();
        let maximized_insets = (0, 32, 0, 0);

        let anchor = drag_restore_anchor_from_start_pointer(
            drag_start_pointer,
            maximized_client_loc,
            maximized_client_size,
            maximized_insets,
        );
        assert_eq!(anchor.pointer_frame_ratio_x, 1.0);
        assert_eq!(anchor.pointer_frame_offset_y, 100.0);
    }

    #[test]
    fn anchored_restore_location_applies_floating_insets_after_frame_anchor() {
        let pointer = point(960.0, 200.0);
        let anchor = drag_restore_anchor_from_start_pointer(
            point(960.0, 100.0),
            (0, 32).into(),
            (1920, 1048).into(),
            (0, 32, 0, 0),
        );

        let restored_client_loc = anchored_client_location_from_pointer(
            pointer,
            anchor.pointer_frame_offset_y,
            anchor.pointer_frame_ratio_x,
            (800, 600).into(),
            (2, 34, 2, 2),
        );

        assert_eq!(restored_client_loc, Point::from((560, 134)));
    }

    #[test]
    fn left_edge_release_triggers_left_half_snap() {
        let output = OutputGeometry {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };

        let action = release_edge_action_for_output(output, point(2.0, 400.0));
        assert_eq!(
            action,
            Some(MoveReleaseEdgeAction::HalfSnap(HalfSnapDirection::Left))
        );
    }

    #[test]
    fn right_edge_release_triggers_right_half_snap() {
        let output = OutputGeometry {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };

        let action = release_edge_action_for_output(output, point(1919.0, 400.0));
        assert_eq!(
            action,
            Some(MoveReleaseEdgeAction::HalfSnap(HalfSnapDirection::Right))
        );
    }

    #[test]
    fn top_edge_maximize_precedes_side_snap() {
        let output = OutputGeometry {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };

        let action = release_edge_action_for_output(output, point(2.0, 2.0));
        assert_eq!(action, Some(MoveReleaseEdgeAction::Maximize));
    }

    #[test]
    fn release_away_from_edges_does_not_snap() {
        let output = OutputGeometry {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };

        assert_eq!(
            release_edge_action_for_output(output, point(800.0, 400.0)),
            None
        );
    }

    #[test]
    fn move_release_workarea_subtracts_panel_reservation() {
        let output = OutputGeometry {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let workarea = move_release_workarea_geometry(output);
        assert_eq!(workarea.x, 0);
        assert_eq!(workarea.y, 0);
        assert_eq!(workarea.width, 1920);
        assert_eq!(workarea.height, 1044);
    }

    #[test]
    fn half_snap_tiled_states_are_set_for_left_and_right() {
        let mut left = smithay::wayland::shell::xdg::ToplevelState::default();
        apply_half_snap_tiled_states(&mut left, HalfSnapDirection::Left);
        assert!(left.states.contains(xdg_toplevel::State::TiledLeft));
        assert!(!left.states.contains(xdg_toplevel::State::TiledRight));
        assert!(left.states.contains(xdg_toplevel::State::TiledTop));
        assert!(left.states.contains(xdg_toplevel::State::TiledBottom));

        let mut right = smithay::wayland::shell::xdg::ToplevelState::default();
        apply_half_snap_tiled_states(&mut right, HalfSnapDirection::Right);
        assert!(!right.states.contains(xdg_toplevel::State::TiledLeft));
        assert!(right.states.contains(xdg_toplevel::State::TiledRight));
        assert!(right.states.contains(xdg_toplevel::State::TiledTop));
        assert!(right.states.contains(xdg_toplevel::State::TiledBottom));
    }

    #[test]
    fn half_snap_restore_prefers_maximize_restore_geometry() {
        let maximize_restore = Some(MaximizeRestoreGeometry::new(
            (40, 50).into(),
            Some((800, 600).into()),
        ));
        let fallback_current_loc: Option<Point<i32, Logical>> = Some((700, 200).into());
        let fallback_current_size: Size<i32, Logical> = (1200, 900).into();

        let selected = half_snap_restore_geometry_source(
            maximize_restore,
            fallback_current_loc,
            fallback_current_size,
        )
        .expect("restore geometry");

        assert_eq!(selected.client_loc, Point::from((40, 50)));
        assert_eq!(selected.client_size, Some(Size::from((800, 600))));
    }

    #[test]
    fn consume_half_snap_restore_geometry_prefers_and_consumes_stored_entry() {
        let mut restore_map = std::collections::HashMap::new();
        restore_map.insert(
            "window-a".to_string(),
            HalfSnapRestoreGeometry::new((40, 50).into(), Some((900, 700).into())),
        );

        let selected = consume_half_snap_restore_geometry(
            &mut restore_map,
            "window-a",
            (10, 20).into(),
            (800, 600).into(),
        );

        assert_eq!(selected.client_loc, Point::from((40, 50)));
        assert_eq!(selected.client_size, Some(Size::from((900, 700))));
        assert!(!restore_map.contains_key("window-a"));
    }

    #[test]
    fn consume_half_snap_restore_geometry_falls_back_to_current_geometry() {
        let mut restore_map = std::collections::HashMap::new();
        let selected = consume_half_snap_restore_geometry(
            &mut restore_map,
            "window-b",
            (120, 140).into(),
            (700, 500).into(),
        );

        assert_eq!(selected.client_loc, Point::from((120, 140)));
        assert_eq!(selected.client_size, Some(Size::from((700, 500))));
    }

    #[test]
    fn half_snap_drag_restore_clears_tiled_bits_and_preserves_other_states() {
        let mut state = smithay::wayland::shell::xdg::ToplevelState::default();
        state.states.set(xdg_toplevel::State::Maximized);
        state.states.set(xdg_toplevel::State::TiledLeft);
        state.states.set(xdg_toplevel::State::TiledTop);
        state.states.set(xdg_toplevel::State::TiledBottom);
        state.states.set(xdg_toplevel::State::Activated);

        apply_half_snap_drag_restore_states(&mut state, (800, 600).into());

        assert!(!state.states.contains(xdg_toplevel::State::Maximized));
        assert!(!state.states.contains(xdg_toplevel::State::TiledLeft));
        assert!(!state.states.contains(xdg_toplevel::State::TiledRight));
        assert!(!state.states.contains(xdg_toplevel::State::TiledTop));
        assert!(!state.states.contains(xdg_toplevel::State::TiledBottom));
        assert!(state.states.contains(xdg_toplevel::State::Activated));
        assert_eq!(state.size, Some(Size::from((800, 600))));
    }

    #[test]
    fn xwayland_snap_rect_for_maximize_uses_workarea_geometry() {
        let output = OutputGeometry {
            x: 10,
            y: 20,
            width: 1600,
            height: 900,
        };
        let rect = super::xwayland_snap_rect_for_action(output, MoveReleaseEdgeAction::Maximize);
        assert_eq!(rect.loc, Point::from((10, 20)));
        assert_eq!(rect.size, Size::from((1600, 900)));
    }

    #[test]
    fn xwayland_snap_rect_for_half_snap_splits_width() {
        let output = OutputGeometry {
            x: 0,
            y: 0,
            width: 1601,
            height: 900,
        };
        let left = super::xwayland_snap_rect_for_action(
            output,
            MoveReleaseEdgeAction::HalfSnap(HalfSnapDirection::Left),
        );
        let right = super::xwayland_snap_rect_for_action(
            output,
            MoveReleaseEdgeAction::HalfSnap(HalfSnapDirection::Right),
        );
        assert_eq!(left.loc, Point::from((0, 0)));
        assert_eq!(left.size, Size::from((800, 900)));
        assert_eq!(right.loc, Point::from((800, 0)));
        assert_eq!(right.size, Size::from((801, 900)));
    }
}
