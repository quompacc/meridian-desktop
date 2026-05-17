use smithay::{
    reexports::wayland_server::protocol::wl_output::WlOutput,
    wayland::session_lock::{
        LockSurface, SessionLockHandler, SessionLockManagerState, SessionLocker,
    },
};

use crate::state::MeridianState;

impl SessionLockHandler for MeridianState {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.session_lock_state
    }

    fn lock(&mut self, confirmation: SessionLocker) {
        if !self.lock_manager.begin_lock() {
            tracing::warn!(
                "session lock requested but state was already locked/pending — drop confirmation"
            );
            drop(confirmation);
            return;
        }
        tracing::info!("session lock requested → phase=Pending");

        // MVP-2a: immediate lock confirmation.
        confirmation.lock();
        let _ = self.lock_manager.confirm_locked();
        tracing::info!("session lock confirmed → phase=Locked (MVP: immediate)");
        self.mark_all_outputs_dirty("session-lock-engaged");
    }

    fn unlock(&mut self) {
        if self.lock_manager.unlock() {
            tracing::info!("session unlock → phase=Unlocked");
            self.mark_all_outputs_dirty("session-lock-released");
        }
    }

    fn new_surface(&mut self, surface: LockSurface, output: WlOutput) {
        let output_name = self
            .outputs
            .iter()
            .find(|o| o.owns(&output))
            .map(|o| o.name())
            .unwrap_or_else(|| {
                tracing::warn!("session lock surface output mapping unresolved; using fallback id");
                "unknown".to_string()
            });
        self.lock_manager.register_surface(&output_name);
        tracing::debug!(
            "session lock surface registered for output={} (MVP: not rendered yet)",
            output_name
        );
        let _ = surface;
    }
}
