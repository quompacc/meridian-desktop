use smithay::{
    reexports::wayland_server::protocol::wl_output::WlOutput,
    utils::SERIAL_COUNTER,
    wayland::session_lock::{
        LockSurface, SessionLockHandler, SessionLockManagerState, SessionLocker,
    },
};

use crate::state::{LockPhase, MeridianState};

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
        self.refresh_lock_focus();
        self.mark_all_outputs_dirty("session-lock-engaged");
    }

    fn unlock(&mut self) {
        let serial = SERIAL_COUNTER.next_serial();
        self.set_keyboard_focus_with_decorations(None, serial);
        if self.lock_manager.unlock() {
            tracing::info!("session unlock → phase=Unlocked");
            self.mark_all_outputs_dirty("session-lock-released");
        }
    }

    fn new_surface(&mut self, surface: LockSurface, output: WlOutput) {
        let matched_output = self.outputs.iter().find(|o| o.owns(&output)).cloned();
        let output_name = matched_output
            .as_ref()
            .map(|o| o.name())
            .unwrap_or_else(|| {
                tracing::warn!("session lock surface output mapping unresolved; using fallback id");
                "unknown".to_string()
            });
        let (width, height) = matched_output
            .as_ref()
            .and_then(|output| output.current_mode().map(|mode| (mode.size.w, mode.size.h)))
            .unwrap_or((1, 1));
        surface.with_pending_state(|state| {
            state.size = Some((width.max(1) as u32, height.max(1) as u32).into());
        });
        surface.send_configure();
        self.lock_manager.register_surface(&output_name, surface);
        tracing::debug!(
            "session lock surface registered for output={} size={}x{}",
            output_name,
            width.max(1),
            height.max(1)
        );
        if matches!(self.lock_manager.phase(), LockPhase::Locked) {
            self.refresh_lock_focus();
        }
    }
}
