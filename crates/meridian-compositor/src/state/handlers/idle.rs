use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    wayland::{
        idle_inhibit::IdleInhibitHandler,
        idle_notify::{IdleNotifierHandler, IdleNotifierState},
    },
};

use crate::state::MeridianState;

impl IdleNotifierHandler for MeridianState {
    fn idle_notifier_state(&mut self) -> &mut IdleNotifierState<Self> {
        &mut self.idle_notifier
    }
}

impl IdleInhibitHandler for MeridianState {
    fn inhibit(&mut self, surface: WlSurface) {
        if self.idle_inhibitors.add(surface) {
            self.idle_notifier.set_is_inhibited(true);
            tracing::debug!(
                "idle inhibit activated (count={})",
                self.idle_inhibitors.len()
            );
        }
    }

    fn uninhibit(&mut self, surface: WlSurface) {
        if self.idle_inhibitors.remove(&surface) {
            self.idle_notifier.set_is_inhibited(false);
            tracing::debug!("idle inhibit cleared (count=0)");
        }
    }
}
