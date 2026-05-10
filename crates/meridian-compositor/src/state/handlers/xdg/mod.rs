use smithay::{
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::protocol::{wl_output::WlOutput, wl_seat::WlSeat},
    },
    utils::Serial,
    wayland::shell::xdg::{
        PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
    },
};

use super::super::MeridianState;

mod lifecycle;
mod requests;

impl XdgShellHandler for MeridianState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        lifecycle::handle_new_toplevel(self, surface);
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        lifecycle::handle_new_popup(self, surface);
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        lifecycle::handle_toplevel_destroyed(self, surface);
    }

    fn app_id_changed(&mut self, surface: ToplevelSurface) {
        lifecycle::handle_surface_metadata_changed(self, surface);
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        lifecycle::handle_surface_metadata_changed(self, surface);
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {}

    fn reposition_request(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: WlSeat, serial: Serial) {
        requests::handle_move_request(self, surface, seat, serial);
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        requests::handle_resize_request(self, surface, seat, serial, edges);
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        requests::handle_maximize_request(self, surface);
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        requests::handle_unmaximize_request(self, surface);
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, _output: Option<WlOutput>) {
        requests::handle_fullscreen_request(self, surface);
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        requests::handle_unfullscreen_request(self, surface);
    }
}
