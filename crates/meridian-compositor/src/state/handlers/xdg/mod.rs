use smithay::{
    desktop::{
        find_popup_root_surface, PopupKeyboardGrab, PopupKind, PopupPointerGrab,
        PopupUngrabStrategy,
    },
    input::{pointer::Focus, Seat},
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

    fn grab(&mut self, surface: PopupSurface, seat: WlSeat, serial: Serial) {
        let Some(seat) = Seat::<Self>::from_resource(&seat) else {
            tracing::warn!("popup grab: wl_seat not associated with a known seat");
            return;
        };

        let kind = PopupKind::Xdg(surface);
        let root_surface = match find_popup_root_surface(&kind) {
            Ok(surface) => surface,
            Err(err) => {
                tracing::debug!("popup grab: cannot find root surface: {:?}", err);
                return;
            }
        };

        let mut grab = match self.popups.grab_popup(root_surface, kind, &seat, serial) {
            Ok(grab) => grab,
            Err(err) => {
                tracing::debug!("popup grab denied: {:?}", err);
                return;
            }
        };

        if let Some(keyboard) = seat.get_keyboard() {
            if keyboard.is_grabbed()
                && !(keyboard.has_grab(serial)
                    || keyboard.has_grab(grab.previous_serial().unwrap_or(serial)))
            {
                tracing::debug!("popup grab: keyboard already grabbed by other serial");
                grab.ungrab(PopupUngrabStrategy::All);
                return;
            }
            keyboard.set_focus(self, grab.current_grab(), serial);
            keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
        }

        if let Some(pointer) = seat.get_pointer() {
            if pointer.is_grabbed()
                && !(pointer.has_grab(serial)
                    || pointer.has_grab(grab.previous_serial().unwrap_or_else(|| grab.serial())))
            {
                tracing::debug!("popup grab: pointer already grabbed by other serial");
                grab.ungrab(PopupUngrabStrategy::All);
                return;
            }
            pointer.set_grab(self, PopupPointerGrab::new(&grab), serial, Focus::Keep);
        }
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        surface.with_pending_state(|state| {
            let geometry = positioner.get_geometry();
            state.geometry = geometry;
            state.positioner = positioner;
        });
        surface.send_repositioned(token);
        if let Err(err) = surface.send_configure() {
            tracing::debug!("popup reposition: send_configure failed: {:?}", err);
        }
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

    fn minimize_request(&mut self, surface: ToplevelSurface) {
        requests::handle_minimize_request(self, surface);
    }
}
