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
    if let Some(current_loc) = data.workspaces.active_space().element_location(window) {
        let current_size = window.geometry().size;
        if current_size.w > 0 && current_size.h > 0 {
            let window_key = format!("x11:{}", x11.window_id());
            data.half_snap_restore_locations
                .entry(window_key)
                .or_insert(HalfSnapRestoreGeometry::new(
                    current_loc,
                    Some(current_size),
                ));
        }
    }

    let target_rect = if let Some(wl_surface) = x11.wl_surface() {
        let theme = &data.theme_manager.current().config.decorations;
        match action {
            MoveReleaseEdgeAction::Maximize => {
                let decoration_offset = data
                    .decoration_manager
                    .decoration_offset(&wl_surface, theme);
                let content_loc = maximized_client_loc_from_output(
                    (output_geometry.x, output_geometry.y).into(),
                    decoration_offset,
                );
                let content_size = Size::from((
                    output_geometry
                        .width
                        .saturating_sub(decoration_offset.0.saturating_mul(2))
                        .max(1),
                    output_geometry
                        .height
                        .saturating_sub(decoration_offset.1.saturating_add(decoration_offset.0))
                        .max(1),
                ));
                Rectangle::new(content_loc, content_size)
            }
            MoveReleaseEdgeAction::HalfSnap(direction) => {
                let decoration_offset = data
                    .decoration_manager
                    .decoration_offset(&wl_surface, theme);
                let decoration_inset = data.decoration_manager.decoration_inset(&wl_surface, theme);
                let placement = half_snap_client_placement_from_output(
                    output_geometry,
                    direction,
                    decoration_offset,
                    decoration_inset,
                );
                Rectangle::new(placement.client_loc, placement.client_size)
            }
        }
    } else {
        xwayland_snap_rect_for_action(output_geometry, action)
    };
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
