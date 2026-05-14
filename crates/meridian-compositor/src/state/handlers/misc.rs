use smithay::{
    delegate_dispatch2,
    input::{dnd::DndGrabHandler, pointer::CursorImageStatus, Seat, SeatHandler},
    reexports::{
        wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode,
        wayland_server::protocol::wl_surface::WlSurface,
    },
    utils::Serial,
    wayland::{
        output::OutputHandler,
        selection::{
            data_device::{DataDeviceHandler, DataDeviceState, WaylandDndGrabHandler},
            SelectionHandler,
        },
        shell::xdg::{decoration::XdgDecorationHandler, ToplevelSurface},
    },
};
use tracing::debug;

use super::super::MeridianState;

impl SeatHandler for MeridianState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut smithay::input::SeatState<Self> {
        &mut self.seat_state
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        match image {
            CursorImageStatus::Named(icon) => {
                debug!(
                    "Cursor update: client requested named cursor ({:?}); compositor cursor path remains active",
                    icon
                );
            }
            CursorImageStatus::Hidden => {
                debug!("Cursor update: client requested hidden cursor");
            }
            CursorImageStatus::Surface(surface) => {
                debug!(
                    "Cursor update: client requested surface cursor ({:?}); compositor cursor path remains active",
                    surface
                );
            }
        }
    }
}

impl OutputHandler for MeridianState {}

impl SelectionHandler for MeridianState {
    type SelectionUserData = ();
}

impl WaylandDndGrabHandler for MeridianState {}

impl DataDeviceHandler for MeridianState {
    fn data_device_state(&mut self) -> &mut DataDeviceState {
        &mut self.data_device_state
    }
}

impl DndGrabHandler for MeridianState {}

impl XdgDecorationHandler for MeridianState {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = None;
        });
        toplevel.send_configure();
        self.decoration_manager
            .set_ssd(toplevel.wl_surface(), false);
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: DecorationMode) {
        let ssd = mode == DecorationMode::ServerSide;
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(mode);
        });
        self.decoration_manager.set_ssd(toplevel.wl_surface(), ssd);
        toplevel.send_configure();
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = None;
        });
        self.decoration_manager
            .set_ssd(toplevel.wl_surface(), false);
        toplevel.send_configure();
    }
}

impl MeridianState {
    pub fn update_focus_decoration(&mut self, old: Option<&WlSurface>, new: Option<&WlSurface>) {
        if let Some(old_surf) = old {
            self.decoration_manager.set_focused(old_surf, false);
        }
        if let Some(new_surf) = new {
            self.decoration_manager.set_focused(new_surf, true);
        }
    }

    pub fn set_keyboard_focus_with_decorations(
        &mut self,
        new_focus: Option<WlSurface>,
        serial: Serial,
    ) {
        let Some(keyboard) = self.seat.get_keyboard() else {
            return;
        };

        let old_focus = keyboard.current_focus();
        if old_focus != new_focus {
            self.update_focus_decoration(old_focus.as_ref(), new_focus.as_ref());
            self.mark_all_outputs_dirty("keyboard-focus-change");
        }

        keyboard.set_focus(self, new_focus, serial);
    }
}

delegate_dispatch2!(MeridianState);
