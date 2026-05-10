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
    let seat = Seat::from_resource(&seat).unwrap();
    let wl_surface = surface.wl_surface();
    if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
        let window = match find_active_window(state, &surface) {
            Some(window) => window,
            None => return,
        };
        let initial_window_location = state
            .workspaces
            .active_space()
            .element_location(&window)
            .unwrap();
        let grab = MoveSurfaceGrab {
            start_data,
            window,
            initial_window_location,
        };
        seat.get_pointer()
            .unwrap()
            .set_grab(state, grab, serial, Focus::Clear);
    }
}

pub(crate) fn handle_resize_request(
    state: &mut MeridianState,
    surface: ToplevelSurface,
    seat: WlSeat,
    serial: Serial,
    edges: xdg_toplevel::ResizeEdge,
) {
    let seat = Seat::from_resource(&seat).unwrap();
    let wl_surface = surface.wl_surface();
    if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
        let window = match find_active_window(state, &surface) {
            Some(window) => window,
            None => return,
        };
        let initial_window_location = state
            .workspaces
            .active_space()
            .element_location(&window)
            .unwrap();
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
        seat.get_pointer()
            .unwrap()
            .set_grab(state, grab, serial, Focus::Clear);
    }
}
