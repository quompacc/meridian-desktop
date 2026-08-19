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

fn xwayland_restore_window_key(data: &MeridianState, window: &Window) -> Option<String> {
    let x11 = window.x11_surface()?;
    if x11.is_override_redirect() || window_is_output_fullscreen_shape(data, window) {
        return None;
    }
    Some(format!("x11:{}", x11.window_id()))
}

fn maybe_restore_xwayland_snapped_drag(
    data: &mut MeridianState,
    window: &Window,
    initial_window_location: Point<i32, Logical>,
    drag_start_location: Point<f64, Logical>,
    current_pointer_location: Point<f64, Logical>,
) -> Option<Point<i32, Logical>> {
    if !movement_crosses_restore_threshold(drag_start_location, current_pointer_location) {
        return None;
    }

    let window_key = xwayland_restore_window_key(data, window)?;
    let restore_geometry = data.half_snap_restore_locations.remove(&window_key)?;
    let x11 = window.x11_surface()?;
    let current_size = window.geometry().size;
    let restore_client_size = restore_geometry.client_size.unwrap_or(current_size);
    if restore_client_size.w <= 0 || restore_client_size.h <= 0 {
        return None;
    }
    let anchor = drag_restore_anchor_from_start_pointer(
        drag_start_location,
        initial_window_location,
        current_size,
        (0, 0, 0, 0),
    );
    let restored_client_location = anchored_restore_client_location(
        current_pointer_location,
        anchor,
        restore_client_size,
        (0, 0, 0, 0),
    );
    let restored_rect = Rectangle::new(restored_client_location, restore_client_size);
    if let Err(err) = x11.configure(restored_rect) {
        tracing::error!("xwayland drag-restore configure failed: {}", err);
        return None;
    }
    data.workspaces
        .active_space_mut()
        .map_element(window.clone(), restored_client_location, true);
    data.mark_all_outputs_dirty("xwayland-drag-restore");
    Some(restored_client_location)
}
