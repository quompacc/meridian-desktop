use std::collections::HashMap;

use smithay::{utils::SERIAL_COUNTER, wayland::session_lock::LockSurface};

use super::MeridianState;

#[derive(Debug, PartialEq, Eq, Default)]
pub enum LockPhase {
    #[default]
    Unlocked,
    Pending,
    Locked,
}

/// Pure-data state. Side-effects (Smithay calls, render dirty
/// flag) live in `MeridianState`; `LockManager` only tracks lock phase
/// and per-output lock surface markers.
#[derive(Debug, Default)]
pub struct LockManager {
    phase: LockPhase,
    surfaces_by_output: HashMap<String, LockSurface>,
}

impl LockManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn phase(&self) -> &LockPhase {
        &self.phase
    }

    pub fn is_locked_or_pending(&self) -> bool {
        !matches!(self.phase, LockPhase::Unlocked)
    }

    pub fn surface_count(&self) -> usize {
        self.surfaces_by_output.len()
    }

    /// Transition Unlocked → Pending. Returns true iff previous state
    /// was Unlocked.
    pub fn begin_lock(&mut self) -> bool {
        if !matches!(self.phase, LockPhase::Unlocked) {
            return false;
        }
        self.phase = LockPhase::Pending;
        true
    }

    /// Transition Pending → Locked. Returns true iff successful.
    pub fn confirm_locked(&mut self) -> bool {
        if !matches!(self.phase, LockPhase::Pending) {
            return false;
        }
        self.phase = LockPhase::Locked;
        true
    }

    /// Transition * → Unlocked and clear all output surface markers.
    pub fn unlock(&mut self) -> bool {
        let was_locked = !matches!(self.phase, LockPhase::Unlocked);
        self.phase = LockPhase::Unlocked;
        self.surfaces_by_output.clear();
        was_locked
    }

    /// Register lock surface for an output.
    pub fn register_surface(&mut self, output_name: &str, surface: LockSurface) -> bool {
        self.surfaces_by_output
            .insert(output_name.to_string(), surface);
        true
    }

    pub fn surface_for_output(&self, output_name: &str) -> Option<&LockSurface> {
        self.surfaces_by_output
            .get(output_name)
            .filter(|surface| surface.alive())
    }

    pub fn surfaces_iter(&self) -> impl Iterator<Item = (&str, &LockSurface)> {
        self.surfaces_by_output
            .iter()
            .filter(|(_, surface)| surface.alive())
            .map(|(name, surface)| (name.as_str(), surface))
    }

    pub fn prune_dead_surfaces(&mut self) -> usize {
        let before = self.surfaces_by_output.len();
        self.surfaces_by_output.retain(|_, surface| surface.alive());
        before.saturating_sub(self.surfaces_by_output.len())
    }

    /// Drop lock surface for an output.
    pub fn drop_surface(&mut self, output_name: &str) -> bool {
        self.surfaces_by_output.remove(output_name).is_some()
    }
}

impl MeridianState {
    pub fn refresh_lock_focus(&mut self) {
        if !matches!(self.lock_manager.phase(), LockPhase::Locked) {
            return;
        }

        let dropped = self.lock_manager.prune_dead_surfaces();
        if dropped > 0 {
            tracing::debug!("pruned dead lock surfaces: {}", dropped);
        }

        let Some(focused_output_id) = self
            .workspace_output_state
            .focused_output(&self.output_registry)
        else {
            return;
        };
        let Some(focused_output_name) = self
            .output_registry
            .by_id(focused_output_id)
            .map(|info| info.name.clone())
        else {
            return;
        };

        let new_focus = self
            .lock_manager
            .surface_for_output(&focused_output_name)
            .map(|surface| surface.wl_surface().clone());
        let serial = SERIAL_COUNTER.next_serial();
        self.set_keyboard_focus_with_decorations(new_focus, serial);
    }
}

#[cfg(test)]
mod tests {
    use super::{LockManager, LockPhase};

    #[test]
    fn default_is_unlocked() {
        let manager = LockManager::new();
        assert_eq!(manager.phase(), &LockPhase::Unlocked);
        assert!(!manager.is_locked_or_pending());
        assert_eq!(manager.surface_count(), 0);
    }

    #[test]
    fn begin_lock_from_unlocked_transitions_to_pending() {
        let mut manager = LockManager::new();
        assert!(manager.begin_lock());
        assert_eq!(manager.phase(), &LockPhase::Pending);
        assert!(manager.is_locked_or_pending());
    }

    #[test]
    fn begin_lock_from_pending_or_locked_is_noop() {
        let mut manager = LockManager::new();
        assert!(manager.begin_lock());
        assert!(!manager.begin_lock());
        assert_eq!(manager.phase(), &LockPhase::Pending);
        assert!(manager.confirm_locked());
        assert!(!manager.begin_lock());
        assert_eq!(manager.phase(), &LockPhase::Locked);
    }

    #[test]
    fn confirm_locked_only_from_pending() {
        let mut manager = LockManager::new();
        assert!(!manager.confirm_locked());
        assert!(manager.begin_lock());
        assert!(manager.confirm_locked());
        assert!(!manager.confirm_locked());
    }

    #[test]
    fn unlock_from_locked_clears_surfaces() {
        let mut manager = LockManager::new();
        manager.begin_lock();
        manager.confirm_locked();
        assert!(manager.unlock());
        assert_eq!(manager.phase(), &LockPhase::Unlocked);
        assert_eq!(manager.surface_count(), 0);
        assert!(!manager.is_locked_or_pending());
    }

    #[test]
    fn full_lifecycle_unlocked_pending_locked_unlocked() {
        let mut manager = LockManager::new();
        assert_eq!(manager.phase(), &LockPhase::Unlocked);
        assert!(manager.begin_lock());
        assert_eq!(manager.phase(), &LockPhase::Pending);
        assert!(manager.confirm_locked());
        assert_eq!(manager.phase(), &LockPhase::Locked);
        assert!(manager.unlock());
        assert_eq!(manager.phase(), &LockPhase::Unlocked);
    }
}
