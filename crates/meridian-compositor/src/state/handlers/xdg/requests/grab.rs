use smithay::{
    input::{pointer::Focus, Seat},
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::protocol::wl_seat::WlSeat,
    },
    utils::{Rectangle, Serial},
    wayland::shell::xdg::ToplevelSurface,
};

use crate::{
    grabs::{
        move_grab::MoveSurfaceGrab,
        resize_grab::{ResizeEdge, ResizeSurfaceGrab},
    },
    state::{handlers::core::check_grab, MeridianState},
};

use super::window::find_active_window;

pub(crate) fn handle_move_request(
    state: &mut MeridianState,
    surface: ToplevelSurface,
    seat: WlSeat,
    serial: Serial,
) {
    let Some(seat) = Seat::from_resource(&seat) else {
        tracing::warn!(
            "ignoring move request: wl_seat resource is not associated with a known seat"
        );
        return;
    };
    let wl_surface = surface.wl_surface();
    if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
        let window = match find_active_window(state, &surface) {
            Some(window) => window,
            None => return,
        };
        let Some(initial_window_location) =
            state.workspaces.active_space().element_location(&window)
        else {
            tracing::debug!("ignoring move request: active window location is unavailable");
            return;
        };
        let started_maximized = surface.with_committed_state(|s| {
            s.map_or(false, |ts| {
                ts.states.contains(xdg_toplevel::State::Maximized)
            })
        }) || surface
            .with_pending_state(|s| s.states.contains(xdg_toplevel::State::Maximized));
        let started_fullscreen = surface.with_committed_state(|s| {
            s.map_or(false, |ts| {
                ts.states.contains(xdg_toplevel::State::Fullscreen)
            })
        }) || surface
            .with_pending_state(|s| s.states.contains(xdg_toplevel::State::Fullscreen));
        let grab = MoveSurfaceGrab {
            start_data,
            window,
            initial_window_location,
            latest_pointer_location: None,
            started_maximized,
            started_fullscreen,
            drag_restore_done: false,
        };
        let Some(pointer) = seat.get_pointer() else {
            tracing::debug!("ignoring move request: seat has no pointer");
            return;
        };
        pointer.set_grab(state, grab, serial, Focus::Clear);
    }
}

pub(crate) fn handle_resize_request(
    state: &mut MeridianState,
    surface: ToplevelSurface,
    seat: WlSeat,
    serial: Serial,
    edges: xdg_toplevel::ResizeEdge,
) {
    let Some(seat) = Seat::from_resource(&seat) else {
        tracing::warn!(
            "ignoring resize request: wl_seat resource is not associated with a known seat"
        );
        return;
    };
    let wl_surface = surface.wl_surface();
    if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
        let window = match find_active_window(state, &surface) {
            Some(window) => window,
            None => return,
        };
        let Some(initial_window_location) =
            state.workspaces.active_space().element_location(&window)
        else {
            tracing::debug!("ignoring resize request: active window location is unavailable");
            return;
        };
        let initial_window_size = window.geometry().size;
        surface.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Resizing);
        });
        surface.send_pending_configure();
        let grab = ResizeSurfaceGrab::start(
            start_data,
            window,
            ResizeEdge::from(edges),
            Rectangle::new(initial_window_location, initial_window_size),
        );
        let Some(pointer) = seat.get_pointer() else {
            tracing::debug!("ignoring resize request: seat has no pointer");
            return;
        };
        pointer.set_grab(state, grab, serial, Focus::Clear);
    }
}
