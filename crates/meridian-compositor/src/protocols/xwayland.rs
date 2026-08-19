use std::{os::unix::io::OwnedFd, process::Stdio};

use smithay::{
    desktop::Window,
    input::pointer::Focus,
    reexports::wayland_server::{protocol::wl_surface::WlSurface, Resource},
    utils::SERIAL_COUNTER,
    utils::{Logical, Point, Rectangle, Size},
    wayland::{
        seat::WaylandFocus,
        selection::{
            data_device::{
                clear_data_device_selection, current_data_device_selection_userdata,
                request_data_device_client_selection, set_data_device_selection,
            },
            primary_selection::{
                clear_primary_selection, current_primary_selection_userdata,
                request_primary_client_selection, set_primary_selection,
            },
            SelectionTarget,
        },
        xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    },
    xwayland::{
        xwm::{Reorder, ResizeEdge as X11ResizeEdge, WmWindowProperty, XwmId},
        X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler,
    },
};
use tracing::{debug, error, info, trace, warn};

use crate::grabs::{
    move_grab::MoveSurfaceGrab,
    resize_grab::{configure_interval_at, ResizeEdge, ResizeSurfaceGrab},
};
use crate::state::{
    maximized_client_loc_from_output, normal_window_workarea_from_output_geometry,
    remember_maximize_restore_geometry, window_list_entry, MaximizeRestoreGeometry, MeridianState,
    XwaylandOrDiagConfigureNotify, XwaylandOrDiagConfigureRequest, XwaylandOrDiagEntry,
};

include!("xwayland/helpers.rs");

trait DecorationSyncTarget {
    fn set_ssd(&mut self, ssd: bool);
    fn set_focused(&mut self, focused: bool);
    fn set_maximized(&mut self, maximized: bool);
    fn set_fullscreen(&mut self, fullscreen: bool);
}

struct SurfaceDecorationSyncTarget<'a> {
    decoration_manager: &'a mut crate::decoration::DecorationManager,
    wl_surface: &'a WlSurface,
}

impl DecorationSyncTarget for SurfaceDecorationSyncTarget<'_> {
    fn set_ssd(&mut self, ssd: bool) {
        self.decoration_manager.set_ssd(self.wl_surface, ssd);
    }

    fn set_focused(&mut self, focused: bool) {
        self.decoration_manager
            .set_focused(self.wl_surface, focused);
    }

    fn set_maximized(&mut self, maximized: bool) {
        self.decoration_manager
            .set_maximized(self.wl_surface, maximized);
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.decoration_manager
            .set_fullscreen(self.wl_surface, fullscreen);
    }
}

fn apply_managed_map_ssd(
    target: &mut impl DecorationSyncTarget,
    maximized: bool,
    fullscreen: bool,
) {
    target.set_ssd(true);
    target.set_focused(false);
    target.set_maximized(maximized);
    target.set_fullscreen(fullscreen);
}

fn apply_override_redirect_ssd(target: &mut impl DecorationSyncTarget) {
    target.set_ssd(false);
}

pub(crate) fn clear_managed_xwayland_maximized_state(
    state: &mut MeridianState,
    window: &X11Surface,
) {
    if window.is_override_redirect() || !window.is_maximized() {
        return;
    }
    if let Err(err) = window.set_maximized(false) {
        error!(
            "xwayland resize start: set_maximized(false) failed: {}",
            err
        );
    }
    state
        .maximize_restore_locations
        .remove(&x11_window_key(window));
}

pub(crate) fn apply_x11_maximize(state: &mut MeridianState, window: &X11Surface) {
    if window.is_override_redirect() {
        return;
    }

    let Some(mapped_window) = find_active_x11_window(state, window) else {
        return;
    };
    let Some(current_loc) = state
        .workspaces
        .space_at(state.workspaces.active)
        .element_location(&mapped_window)
    else {
        return;
    };
    let current_size = mapped_window.geometry().size;
    let requested_rect = Rectangle::new(current_loc, current_size);
    let Some(output_geometry) = select_output_geometry_for_rect(state, requested_rect) else {
        return;
    };
    let workarea = normal_window_workarea_from_output_geometry(output_geometry);

    let decoration_offset = if let Some(wl_surface) = window.wl_surface() {
        state.decoration_manager.decoration_offset(
            &wl_surface,
            &state.theme_manager.current().config.decorations,
        )
    } else {
        (0, 0)
    };
    let content_loc =
        maximized_client_loc_from_output((workarea.x, workarea.y).into(), decoration_offset);
    let content_size = maximized_x11_content_size(
        (workarea.width.max(1), workarea.height.max(1)).into(),
        decoration_offset,
    );
    let target_rect = Rectangle::new(content_loc, content_size);

    remember_maximize_restore_geometry(
        &mut state.maximize_restore_locations,
        x11_window_key(window),
        MaximizeRestoreGeometry::new(current_loc, Some(current_size)),
    );
    if let Err(err) = window.set_maximized(true) {
        error!("xwayland maximize request: set_maximized failed: {}", err);
    }
    if let Some(wl_surface) = window.wl_surface() {
        state.decoration_manager.set_maximized(&wl_surface, true);
    }
    if let Err(err) = window.configure(target_rect) {
        error!("xwayland maximize request: configure failed: {}", err);
        return;
    }
    state
        .workspaces
        .space_at_mut(state.workspaces.active)
        .map_element(mapped_window, content_loc, true);
    state.mark_all_outputs_dirty("xwayland-maximize-request");
}

struct X11Remeasure {
    window: Window,
    loc: Point<i32, Logical>,
    size: Size<i32, Logical>,
}

/// Re-apply maximized geometry to X11 windows after an output's geometry
/// changed (mode switch / removal), so a maximized X11 window keeps filling
/// its output. Mirrors apply_x11_maximize's frame math but leaves the restore
/// geometry untouched (the window is already maximized) and skips windows
/// whose size is unchanged. Compile-verified; a real X11 mode switch needs
/// hardware to feel out.
pub(crate) fn remeasure_maximized_x11_windows(state: &mut MeridianState) {
    let mut updates: Vec<X11Remeasure> = Vec::new();
    for ws in 0..state.workspaces.count() {
        let space = state.workspaces.space_at(ws);
        for window in space.elements() {
            let Some(x11) = window.x11_surface() else {
                continue;
            };
            if x11.is_override_redirect() || !x11.is_maximized() {
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
            let workarea = normal_window_workarea_from_output_geometry(output.geometry);
            let decoration_offset = x11
                .wl_surface()
                .map(|wl| {
                    state
                        .decoration_manager
                        .decoration_offset(&wl, &state.theme_manager.current().config.decorations)
                })
                .unwrap_or((0, 0));
            let content_loc = maximized_client_loc_from_output(
                (workarea.x, workarea.y).into(),
                decoration_offset,
            );
            let content_size = maximized_x11_content_size(
                (workarea.width.max(1), workarea.height.max(1)).into(),
                decoration_offset,
            );
            if content_size != window.geometry().size {
                updates.push(X11Remeasure {
                    window: window.clone(),
                    loc: content_loc,
                    size: content_size,
                });
            }
        }
    }
    for update in updates {
        if let Some(x11) = update.window.x11_surface() {
            if let Err(err) = x11.configure(Rectangle::new(update.loc, update.size)) {
                error!("xwayland re-measure configure failed: {}", err);
                continue;
            }
        }
        if let Some((ws, _)) = state
            .workspaces
            .find_element_workspace(|w| w == &update.window)
        {
            state
                .workspaces
                .space_at_mut(ws)
                .map_element(update.window, update.loc, false);
        }
    }
}

pub(crate) fn apply_x11_unmaximize(state: &mut MeridianState, window: &X11Surface) {
    if window.is_override_redirect() {
        return;
    }

    let Some(mapped_window) = find_active_x11_window(state, window) else {
        return;
    };
    if let Err(err) = window.set_maximized(false) {
        error!("xwayland unmaximize request: set_maximized failed: {}", err);
    }
    if let Some(wl_surface) = window.wl_surface() {
        state.decoration_manager.set_maximized(&wl_surface, false);
    }

    let restore = state
        .maximize_restore_locations
        .remove(&x11_window_key(window));
    let Some(restore) = restore else {
        return;
    };
    let restore_size = restore
        .client_size
        .unwrap_or_else(|| mapped_window.geometry().size);
    let restore_rect = Rectangle::new(restore.client_loc, restore_size);
    if let Err(err) = window.configure(restore_rect) {
        error!("xwayland unmaximize request: configure failed: {}", err);
        return;
    }
    state
        .workspaces
        .space_at_mut(state.workspaces.active)
        .map_element(mapped_window, restore.client_loc, true);
    state.mark_all_outputs_dirty("xwayland-unmaximize-request");
}

pub fn start_xwayland(state: &mut MeridianState) {
    let (xwayland, client) = match XWayland::spawn(
        &state.display_handle,
        None,
        std::iter::empty::<(String, String)>(),
        true,
        Stdio::null(),
        Stdio::null(),
        |_| (),
    ) {
        Ok(x) => x,
        Err(e) => {
            error!("Failed to spawn XWayland: {}", e);
            return;
        }
    };

    let display_handle = state.display_handle.clone();
    let handle = state.loop_handle.clone();

    if let Err(e) = state
        .loop_handle
        .insert_source(xwayland, move |event, _, state| match event {
            XWaylandEvent::Ready {
                x11_socket,
                display_number,
            } => {
                match X11Wm::start_wm(handle.clone(), &display_handle, x11_socket, client.clone()) {
                    Ok(wm) => {
                        // SAFETY: this updates process env once XWayland reports its assigned display number.
                        unsafe {
                            std::env::set_var("DISPLAY", format!(":{}", display_number));
                        }
                        info!("XWayland ready on DISPLAY=:{}", display_number);
                        state.xwm = Some(wm);
                    }
                    Err(e) => error!("Failed to start X11 WM: {}", e),
                }
            }
            XWaylandEvent::Error => warn!("XWayland crashed on startup"),
        })
    {
        error!("Failed to insert XWayland event source: {}", e);
    }
}

impl XWaylandShellHandler for MeridianState {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.xwayland_shell_state
    }

    fn surface_associated(&mut self, _xwm: XwmId, wl_surface: WlSurface, window: X11Surface) {
        let is_override_redirect = window.is_override_redirect();
        let is_maximized = window.is_maximized();
        let is_fullscreen = window.is_fullscreen();

        let mut decoration_target = SurfaceDecorationSyncTarget {
            decoration_manager: &mut self.decoration_manager,
            wl_surface: &wl_surface,
        };

        if is_override_redirect {
            apply_override_redirect_ssd(&mut decoration_target);
        } else {
            apply_managed_map_ssd(&mut decoration_target, is_maximized, is_fullscreen);
        }

        info!(
            event = "xwayland.surface_associated",
            window_id = window.window_id(),
            override_redirect = is_override_redirect,
            maximized = is_maximized,
            fullscreen = is_fullscreen,
            wl_surface_id = wl_surface.id().protocol_id(),
            "applied SSD state on surface_associated"
        );
    }
}

include!("xwayland/window_lifecycle.rs");
include!("xwayland/configure.rs");
include!("xwayland/state_requests.rs");
include!("xwayland/input_selection.rs");

impl XwmHandler for MeridianState {
    xwm_window_lifecycle_methods!();
    xwm_configure_methods!();
    xwm_state_request_methods!();
    xwm_input_selection_methods!();
}

#[cfg(test)]
#[path = "xwayland_tests.rs"]
mod tests;
