use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Point};
use smithay::wayland::shell::xdg::ToplevelSurface;

use crate::state::{
    clear_tiled_toplevel_states, maximized_client_loc_from_output,
    remember_maximize_restore_geometry, resolve_unmaximize_restore_client_loc,
    take_maximize_restore_geometry, window_id, MaximizeRestoreGeometry, MeridianState,
    OutputGeometry, OutputInfo,
};

use super::window::find_active_window;

pub(crate) fn handle_maximize_request(state: &mut MeridianState, surface: ToplevelSurface) {
    tracing::debug!("maximize geometry requested");
    let is_maxed = surface.with_committed_state(|s| {
        s.map_or(false, |ts| {
            ts.states.contains(xdg_toplevel::State::Maximized)
        })
    });
    if let Some(selected) = select_output_for_surface(state, &surface) {
        tracing::debug!(
            "selected output for maximize: id={} name={} fallback_reason={}",
            selected.id.0,
            selected.name,
            selected.fallback_reason
        );
        let size = (selected.geometry.width, selected.geometry.height).into();
        let loc: Point<i32, Logical> = (selected.geometry.x, selected.geometry.y).into();
        surface.with_pending_state(|state| {
            clear_tiled_toplevel_states(state);
            state.states.set(xdg_toplevel::State::Maximized);
            state.size = Some(size);
        });
        state
            .decoration_manager
            .set_maximized(surface.wl_surface(), true);
        let theme = &state.theme_manager.current().config.decorations;
        let (x_off, y_off) = state
            .decoration_manager
            .decoration_offset(surface.wl_surface(), theme);
        let maximized_client_loc = maximized_client_loc_from_output(loc, (x_off, y_off));

        if let Some(window) = find_active_window(state, &surface) {
            if !is_maxed {
                if let Some(current_loc) = state.workspaces.active_space().element_location(&window)
                {
                    remember_maximize_restore_geometry(
                        &mut state.maximize_restore_locations,
                        window_id(surface.wl_surface()),
                        MaximizeRestoreGeometry::new(current_loc, Some(window.geometry().size)),
                    );
                }
            }
            state
                .workspaces
                .active_space_mut()
                .map_element(window, maximized_client_loc, true);
        }
    } else {
        tracing::debug!("selected output for maximize: none (registry empty)");
    }
    surface.send_pending_configure();
}

pub(crate) fn handle_unmaximize_request(state: &mut MeridianState, surface: ToplevelSurface) {
    state
        .decoration_manager
        .set_maximized(surface.wl_surface(), false);
    surface.with_pending_state(|state| {
        state.states.unset(xdg_toplevel::State::Maximized);
        state.size = None;
    });
    if let Some(window) = find_active_window(state, &surface) {
        let restore_geometry = take_maximize_restore_geometry(
            &mut state.maximize_restore_locations,
            surface.wl_surface(),
        );
        let (restore_loc, used_fallback) = if restore_geometry.is_some() {
            resolve_unmaximize_restore_client_loc(restore_geometry, (0, 0))
        } else {
            let theme = &state.theme_manager.current().config.decorations;
            let (x_off, y_off) = state
                .decoration_manager
                .decoration_offset(surface.wl_surface(), theme);
            resolve_unmaximize_restore_client_loc(None, (x_off, y_off))
        };
        if used_fallback {
            tracing::warn!(
                x = restore_loc.x,
                y = restore_loc.y,
                "unmaximize restore location missing in xdg request path; applying fallback client origin"
            );
        }
        state
            .workspaces
            .active_space_mut()
            .map_element(window, restore_loc, true);
    }
    surface.send_pending_configure();
}

pub(crate) fn handle_fullscreen_request(state: &mut MeridianState, surface: ToplevelSurface) {
    tracing::debug!("fullscreen geometry requested");
    if let Some(selected) = select_output_for_surface(state, &surface) {
        tracing::debug!(
            "selected output for fullscreen: id={} name={} fallback_reason={}",
            selected.id.0,
            selected.name,
            selected.fallback_reason
        );
        let size = (selected.geometry.width, selected.geometry.height).into();
        let loc: Point<i32, Logical> = (selected.geometry.x, selected.geometry.y).into();
        surface.with_pending_state(|state| {
            clear_tiled_toplevel_states(state);
            state.states.set(xdg_toplevel::State::Fullscreen);
            state.size = Some(size);
        });
        state
            .decoration_manager
            .set_fullscreen(surface.wl_surface(), true);

        if let Some(window) = find_active_window(state, &surface) {
            state
                .workspaces
                .active_space_mut()
                .map_element(window, loc, true);
        }
    } else {
        tracing::debug!("selected output for fullscreen: none (registry empty)");
    }
    surface.send_pending_configure();
}

pub(crate) fn handle_unfullscreen_request(state: &mut MeridianState, surface: ToplevelSurface) {
    state
        .decoration_manager
        .set_fullscreen(surface.wl_surface(), false);
    surface.with_pending_state(|state| {
        state.states.unset(xdg_toplevel::State::Fullscreen);
        state.size = None;
    });
    surface.send_pending_configure();
}

#[derive(Debug, Clone)]
struct SelectedOutput {
    id: crate::state::OutputId,
    name: String,
    geometry: OutputGeometry,
    fallback_reason: &'static str,
}

fn select_output_for_surface(
    state: &MeridianState,
    surface: &ToplevelSurface,
) -> Option<SelectedOutput> {
    let window_center = find_active_window(state, surface).and_then(|window| {
        let loc = state.workspaces.active_space().element_location(&window)?;
        let size = window.geometry().size;
        Some((
            loc.x as f64 + (size.w.max(1) as f64 * 0.5),
            loc.y as f64 + (size.h.max(1) as f64 * 0.5),
        ))
    });
    select_output_from_infos_for_point(state.output_registry.list(), window_center)
}

fn select_output_from_infos_for_point(
    infos: &[OutputInfo],
    point: Option<(f64, f64)>,
) -> Option<SelectedOutput> {
    if let Some((x, y)) = point {
        if let Some(info) = infos.iter().find(|info| info.geometry.contains(x, y)) {
            return Some(SelectedOutput {
                id: info.id,
                name: info.name.clone(),
                geometry: info.geometry,
                fallback_reason: "window-output",
            });
        }
    }

    if let Some(info) = infos.iter().find(|info| info.primary) {
        return Some(SelectedOutput {
            id: info.id,
            name: info.name.clone(),
            geometry: info.geometry,
            fallback_reason: "fallback-primary",
        });
    }

    infos.first().map(|info| SelectedOutput {
        id: info.id,
        name: info.name.clone(),
        geometry: info.geometry,
        fallback_reason: "fallback-first",
    })
}

#[cfg(test)]
mod tests {
    use smithay::utils::Transform;

    use crate::state::{OutputGeometry, OutputId, OutputInfo};

    use super::select_output_from_infos_for_point;

    fn info(id: u32, name: &str, primary: bool, x: i32) -> OutputInfo {
        OutputInfo {
            id: OutputId(id),
            name: name.to_string(),
            geometry: OutputGeometry {
                x,
                y: 0,
                width: 1920,
                height: 1080,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
            primary,
        }
    }

    #[test]
    fn selects_primary_on_fallback() {
        let infos = vec![info(1, "a", true, 0), info(2, "b", false, 1920)];
        let selected = select_output_from_infos_for_point(&infos, None).expect("selection");
        assert_eq!(selected.id.0, 1);
        assert_eq!(selected.fallback_reason, "fallback-primary");
    }

    #[test]
    fn selects_first_when_no_primary_marked() {
        let infos = vec![info(10, "first", false, 0), info(11, "second", false, 1920)];
        let selected =
            select_output_from_infos_for_point(&infos, Some((-10.0, -10.0))).expect("selection");
        assert_eq!(selected.id.0, 10);
        assert_eq!(selected.fallback_reason, "fallback-first");
    }

    #[test]
    fn empty_infos_is_safe() {
        assert!(select_output_from_infos_for_point(&[], None).is_none());
    }
}
