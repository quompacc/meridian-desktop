use smithay_client_toolkit::{
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::OutputState,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::SeatState,
};

use super::MeridianShell;

mod compositor;
mod keyboard;
mod layer;
mod output;
mod pointer;
mod pointer_state;
mod pointer_translate;
mod seat;
mod shm;

delegate_compositor!(MeridianShell);
delegate_output!(MeridianShell);
delegate_shm!(MeridianShell);
delegate_seat!(MeridianShell);
delegate_keyboard!(MeridianShell);
delegate_pointer!(MeridianShell);
delegate_layer!(MeridianShell);
delegate_registry!(MeridianShell);

impl ProvidesRegistryState for MeridianShell {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState, SeatState];
}
