use smithay::desktop::Window;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Point, Size, SERIAL_COUNTER};
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::shell::xdg::ToplevelSurface;

use crate::state::{
    clear_tiled_toplevel_states, maximized_client_loc_from_output,
    normal_window_workarea_from_output_geometry, remember_maximize_restore_geometry,
    resolve_unmaximize_restore_client_loc, take_maximize_restore_geometry, window_id,
    MaximizeRestoreGeometry, MeridianState, MinimizedWindowEntry, OutputGeometry, OutputInfo,
};

use super::window::find_active_window;

pub(crate) fn handle_maximize_request(state: &mut MeridianState, surface: ToplevelSurface) {
    tracing::debug!("maximize geometry requested");
    let is_maxed = surface.with_committed_state(|s| {
        s.is_some_and(|ts| ts.states.contains(xdg_toplevel::State::Maximized))
    });
    if let Some(selected) = select_output_for_surface(state, &surface) {
        tracing::debug!(
            "selected output for maximize: id={} name={} fallback_reason={}",
            selected.id.0,
            selected.name,
            selected.fallback_reason
        );
        let (loc, size) = normal_maximize_frame_for_output(selected.geometry);
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

fn normal_maximize_frame_for_output(
    output_geometry: OutputGeometry,
) -> (Point<i32, Logical>, Size<i32, Logical>) {
    let workarea = normal_window_workarea_from_output_geometry(output_geometry);
    (
        (workarea.x, workarea.y).into(),
        (workarea.width, workarea.height).into(),
    )
}

/// New maximized frame when a window's output geometry changes, or None if the
/// workarea size is unchanged (skip a redundant configure). Re-measuring keeps
/// a maximized window filling its output after a mode switch.
fn remeasured_maximized_frame(
    current_size: Size<i32, Logical>,
    new_output: OutputGeometry,
) -> Option<(Point<i32, Logical>, Size<i32, Logical>)> {
    let (loc, size) = normal_maximize_frame_for_output(new_output);
    (size != current_size).then_some((loc, size))
}

/// Re-apply maximized geometry to XDG toplevels after an output's geometry
/// changed (e.g. a mode switch), so a maximized window keeps filling its output
/// instead of keeping its old size. Idempotent: windows already at the right
/// size are skipped. The frame math is unit-tested; the toplevel configure
/// apply is compile-verified (a real mode switch needs hardware to feel out).
type MaximizedUpdate = (Window, Point<i32, Logical>, Size<i32, Logical>);

pub(crate) fn remeasure_maximized_windows(state: &mut MeridianState) {
    let mut updates: Vec<MaximizedUpdate> = Vec::new();
    for ws in 0..state.workspaces.count() {
        let space = state.workspaces.space_at(ws);
        for window in space.elements() {
            let Some(toplevel) = window.toplevel() else {
                continue;
            };
            let maximized = toplevel.with_committed_state(|committed| {
                committed.is_some_and(|s| s.states.contains(xdg_toplevel::State::Maximized))
            });
            if !maximized {
                continue;
            }
            let Some(loc) = space.element_location(window) else {
                continue;
            };
            let Some(output) = state
                .output_registry
                .output_at_point(loc.x as f64, loc.y as f64)
            else {
                continue;
            };
            if let Some((new_loc, new_size)) =
                remeasured_maximized_frame(window.geometry().size, output.geometry)
            {
                updates.push((window.clone(), new_loc, new_size));
            }
        }
    }
    for (window, loc, size) in updates {
        if let Some(toplevel) = window.toplevel() {
            toplevel.with_pending_state(|pending| pending.size = Some(size));
            toplevel.send_pending_configure();
        }
        if let Some((ws, _)) = state.workspaces.find_element_workspace(|w| w == &window) {
            state
                .workspaces
                .space_at_mut(ws)
                .map_element(window, loc, false);
        }
    }
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

pub(crate) fn handle_minimize_request(state: &mut MeridianState, surface: ToplevelSurface) {
    let Some(window) = find_active_window(state, &surface) else {
        return;
    };

    let workspace = state.workspaces.active;
    let restore_loc = state
        .workspaces
        .space_at(workspace)
        .element_location(&window)
        .unwrap_or_default();
    let window_key = window_id(surface.wl_surface());

    state.minimized_windows.insert(
        window_key,
        MinimizedWindowEntry {
            window: window.clone(),
            workspace,
            restore_loc,
        },
    );
    state.workspaces.space_at_mut(workspace).unmap_elem(&window);

    let serial = SERIAL_COUNTER.next_serial();
    let window_surface = window
        .wl_surface()
        .map(|wl_surface| wl_surface.into_owned());
    let was_focused = window_surface.as_ref().is_some_and(|window_surface| {
        state
            .seat
            .get_keyboard()
            .and_then(|keyboard| keyboard.current_focus())
            .as_ref()
            == Some(window_surface)
    });
    if was_focused {
        let fallback_surface = state
            .workspaces
            .space_at(workspace)
            .elements()
            .filter_map(|candidate| {
                candidate
                    .wl_surface()
                    .map(|wl_surface| wl_surface.into_owned())
            })
            .next_back();
        if let Some(fallback_surface) = fallback_surface {
            state.set_keyboard_focus_with_decorations(Some(fallback_surface.clone()), serial);
            state.update_focused_output_from_surface(
                &fallback_surface,
                "keyboard-focus-xdg-minimize-fallback",
            );
            state.broadcast_toplevel_focused(&fallback_surface);
        } else {
            state.set_keyboard_focus_with_decorations(None, serial);
            state.broadcast_toplevel_focus_cleared();
        }
    }

    state
        .workspaces
        .space_at(workspace)
        .elements()
        .for_each(|window| {
            if let Some(toplevel) = window.toplevel() {
                toplevel.send_pending_configure();
            }
        });
    state.mark_all_outputs_dirty("xdg-minimize-request");
    state.broadcast_window_snapshot();
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

    use crate::state::{OutputGeometry, OutputId, OutputInfo, NORMAL_WINDOW_BOTTOM_RESERVED_PX};

    use super::{
        normal_maximize_frame_for_output, remeasured_maximized_frame,
        select_output_from_infos_for_point,
    };

    #[test]
    fn remeasured_maximized_frame_tracks_output_and_skips_noop() {
        let big = OutputGeometry {
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
        };
        let small = OutputGeometry {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
        };
        let (_, big_size) = normal_maximize_frame_for_output(big);
        let (_, small_size) = normal_maximize_frame_for_output(small);
        // Output shrank: the maximized frame must follow to the smaller workarea.
        assert_eq!(
            remeasured_maximized_frame(big_size, small),
            Some(((0, 0).into(), small_size))
        );
        // Unchanged geometry: no re-measure (avoid a redundant configure).
        assert_eq!(remeasured_maximized_frame(small_size, small), None);
    }

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

    #[test]
    fn normal_maximize_frame_uses_panel_safe_height() {
        let output = OutputGeometry {
            x: 42,
            y: 7,
            width: 1600,
            height: 900,
        };
        let (loc, size) = normal_maximize_frame_for_output(output);
        assert_eq!(loc, (42, 7).into());
        assert_eq!(size.w, 1600);
        assert_eq!(size.h, 900 - NORMAL_WINDOW_BOTTOM_RESERVED_PX);
    }
}
