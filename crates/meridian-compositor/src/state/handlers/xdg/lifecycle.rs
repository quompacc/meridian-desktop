use smithay::{
    desktop::{PopupKind, Window},
    reexports::{wayland_protocols::xdg::shell::server::xdg_toplevel, wayland_server::Resource},
    utils::{Logical, Point, SERIAL_COUNTER},
    wayland::{
        compositor::with_states,
        shell::xdg::{PopupSurface, PositionerState, ToplevelSurface, XdgToplevelSurfaceData},
    },
};

use meridian_wm::WorkspaceMode;

use crate::state::{
    maximized_client_loc_from_output, normal_window_workarea_from_rect,
    remember_maximize_restore_geometry, window_id, MaximizeRestoreGeometry, MeridianState,
};

fn initial_maximized_client_origin(
    state: &MeridianState,
    surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
) -> Option<Point<i32, Logical>> {
    let selected_output_name = state
        .output_registry
        .list()
        .iter()
        .find(|info| info.primary)
        .or_else(|| state.output_registry.list().first())
        .map(|info| info.name.as_str())?;
    let output = state
        .outputs
        .iter()
        .find(|candidate| candidate.name() == selected_output_name)?;
    let geo =
        normal_window_workarea_from_rect(state.workspaces.active_space().output_geometry(output)?);
    let theme = &state.theme_manager.current().config.decorations;
    let (x_off, y_off) = state.decoration_manager.decoration_offset(surface, theme);
    Some(maximized_client_loc_from_output(geo.loc, (x_off, y_off)))
}

pub(super) fn handle_new_toplevel(state: &mut MeridianState, surface: ToplevelSurface) {
    tracing::info!(
        "new xdg toplevel: {}",
        crate::state::toplevel_title(&surface)
    );
    let initial_states = surface.with_pending_state(|s| Vec::from(s.states.clone()));
    let initial_size = surface.with_pending_state(|s| s.size);
    let initial_bounds = surface.with_pending_state(|s| s.bounds);
    let initial_app_id = with_states(surface.wl_surface(), |states| {
        states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .and_then(|data| data.lock().unwrap().app_id.clone())
    });
    tracing::info!(
        title = %crate::state::toplevel_title(&surface),
        app_id = ?initial_app_id,
        pending_states = ?initial_states,
        pending_size = ?initial_size,
        pending_bounds = ?initial_bounds,
        "diagnostic: new xdg toplevel pending state"
    );
    state.broadcast_toplevel_opened(&surface);
    let wl_surface = surface.wl_surface().clone();
    let window = Window::new_wayland_window(surface.clone());

    // Default to no compositor-side decorations until a client decoration mode is negotiated.
    state.decoration_manager.set_ssd(&wl_surface, false);

    let active = state.workspaces.active;
    if state.wm_workspaces[active].mode == WorkspaceMode::Tiling {
        state
            .workspaces
            .active_space_mut()
            .map_element(window.clone(), (0, 0), true);
        state.decoration_manager.set_tiled(&wl_surface, true);
        let focused = state.focused_window();
        state.wm_workspaces[active].add_tiled(window, focused.as_ref());
        state.tile_workspace(active);
    } else {
        let theme = &state.theme_manager.current().config.decorations;
        let (x_off, y_off) = state
            .decoration_manager
            .decoration_offset(&wl_surface, theme);
        let initial_client_origin: Point<i32, Logical> = (x_off, y_off).into();
        let starts_maximized = surface.with_committed_state(|s| {
            s.is_some_and(|ts| ts.states.contains(xdg_toplevel::State::Maximized))
        }) || surface
            .with_pending_state(|s| s.states.contains(xdg_toplevel::State::Maximized));

        if starts_maximized {
            state.decoration_manager.set_maximized(&wl_surface, true);
            remember_maximize_restore_geometry(
                &mut state.maximize_restore_locations,
                window_id(&wl_surface),
                MaximizeRestoreGeometry::new(initial_client_origin, None),
            );
            let maximized_origin = initial_maximized_client_origin(state, &wl_surface)
                .unwrap_or(initial_client_origin);
            state
                .workspaces
                .active_space_mut()
                .map_element(window, maximized_origin, true);
        } else {
            state
                .workspaces
                .active_space_mut()
                .map_element(window, initial_client_origin, true);
        }
    }

    let serial = SERIAL_COUNTER.next_serial();
    if state.seat.get_keyboard().is_some() {
        state.set_keyboard_focus_with_decorations(Some(wl_surface.clone()), serial);
        state.update_focused_output_from_surface(&wl_surface, "keyboard-focus-new-toplevel");
        state.broadcast_toplevel_focused(&wl_surface);
    }
    state.mark_all_outputs_dirty("xdg-new-toplevel");
}

pub(super) fn handle_new_popup(
    state: &mut MeridianState,
    surface: PopupSurface,
    positioner: PositionerState,
) {
    super::configure_popup_pending_state(state, &surface, positioner);
    let _ = state.popups.track_popup(PopupKind::Xdg(surface));
}

pub(super) fn handle_toplevel_destroyed(state: &mut MeridianState, surface: ToplevelSurface) {
    let id = window_id(surface.wl_surface());
    state
        .diag_logged_toplevels
        .remove(&surface.wl_surface().id());
    state.clear_window_runtime_state(&id);
    state.decoration_manager.remove(surface.wl_surface());
    state.broadcast_toplevel_closed(&surface);
    state.mark_all_outputs_dirty("xdg-toplevel-destroyed");
}

pub(super) fn handle_surface_metadata_changed(state: &mut MeridianState, surface: ToplevelSurface) {
    state.broadcast_toplevel_opened(&surface);
    state.mark_all_outputs_dirty("xdg-surface-metadata-changed");
}
