use std::collections::{HashMap, HashSet};

use smithay::{
    utils::SERIAL_COUNTER,
    wayland::session_lock::{LockSurface, SessionLocker},
};

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
    pending_locker: Option<SessionLocker>,
    pending_targets: HashSet<String>,
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

    /// Transition Unlocked → Pending with a confirmation locker and
    /// the output names that must each render one pending frame.
    ///
    /// Returns `Some(locker)` only for the zero-target fast path.
    pub fn begin_lock_with_targets(
        &mut self,
        locker: SessionLocker,
        targets: impl IntoIterator<Item = String>,
    ) -> Option<SessionLocker> {
        if !matches!(self.phase, LockPhase::Unlocked) {
            drop(locker);
            return None;
        }
        self.pending_targets = targets.into_iter().collect();
        self.phase = LockPhase::Pending;
        if self.pending_targets.is_empty() {
            return Some(locker);
        }
        self.pending_locker = Some(locker);
        None
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
        self.pending_locker = None;
        self.pending_targets.clear();
        was_locked
    }

    /// Mark one pending target as rendered.
    pub fn record_pending_frame(&mut self, output_name: &str) -> Option<SessionLocker> {
        if !matches!(self.phase, LockPhase::Pending) {
            return None;
        }
        self.pending_targets.remove(output_name);
        if self.pending_targets.is_empty() {
            self.pending_locker.take()
        } else {
            None
        }
    }

    /// Drop a pending target when its output disappears.
    pub fn forget_pending_target(&mut self, output_name: &str) -> Option<SessionLocker> {
        if !matches!(self.phase, LockPhase::Pending) {
            return None;
        }
        self.pending_targets.remove(output_name);
        if self.pending_targets.is_empty() {
            self.pending_locker.take()
        } else {
            None
        }
    }

    pub fn pending_target_count(&self) -> usize {
        self.pending_targets.len()
    }

    pub fn has_pending_locker(&self) -> bool {
        self.pending_locker.is_some()
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

#[cfg(test)]
impl LockManager {
    /// Test-only helper to enter pending state without constructing
    /// a real `SessionLocker`.
    pub fn begin_pending_for_test(&mut self, targets: impl IntoIterator<Item = String>) -> bool {
        if !matches!(self.phase, LockPhase::Unlocked) {
            return false;
        }
        self.pending_targets = targets.into_iter().collect();
        self.pending_locker = None;
        self.phase = LockPhase::Pending;
        true
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
        assert!(manager.begin_pending_for_test(["out-0".to_string()]));
        assert_eq!(manager.phase(), &LockPhase::Pending);
        assert!(manager.is_locked_or_pending());
    }

    #[test]
    fn begin_lock_from_pending_or_locked_is_noop() {
        let mut manager = LockManager::new();
        assert!(manager.begin_pending_for_test(["out-0".to_string()]));
        assert!(!manager.begin_pending_for_test(["out-1".to_string()]));
        assert_eq!(manager.phase(), &LockPhase::Pending);
        assert!(manager.confirm_locked());
        assert!(!manager.begin_pending_for_test(["out-2".to_string()]));
        assert_eq!(manager.phase(), &LockPhase::Locked);
    }

    #[test]
    fn confirm_locked_only_from_pending() {
        let mut manager = LockManager::new();
        assert!(!manager.confirm_locked());
        assert!(manager.begin_pending_for_test(["out-0".to_string()]));
        assert!(manager.confirm_locked());
        assert!(!manager.confirm_locked());
    }

    #[test]
    fn unlock_from_locked_clears_surfaces() {
        let mut manager = LockManager::new();
        manager.begin_pending_for_test(["out-0".to_string()]);
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
        assert!(manager.begin_pending_for_test(["out-0".to_string()]));
        assert_eq!(manager.phase(), &LockPhase::Pending);
        assert!(manager.confirm_locked());
        assert_eq!(manager.phase(), &LockPhase::Locked);
        assert!(manager.unlock());
        assert_eq!(manager.phase(), &LockPhase::Unlocked);
    }

    #[test]
    fn begin_pending_with_targets_keeps_phase_pending_until_all_frames() {
        let mut manager = LockManager::new();
        assert!(manager.begin_pending_for_test(["a".to_string(), "b".to_string()]));
        assert_eq!(manager.pending_target_count(), 2);
        assert_eq!(manager.phase(), &LockPhase::Pending);

        let _ = manager.record_pending_frame("a");
        assert_eq!(manager.pending_target_count(), 1);
        assert_eq!(manager.phase(), &LockPhase::Pending);
    }

    #[test]
    fn record_pending_frame_unknown_output_is_noop() {
        let mut manager = LockManager::new();
        assert!(manager.begin_pending_for_test(["a".to_string()]));
        let _ = manager.record_pending_frame("unknown");
        assert_eq!(manager.pending_target_count(), 1);
        assert_eq!(manager.phase(), &LockPhase::Pending);
    }

    #[test]
    fn record_pending_frame_last_target_returns_ready_signal() {
        let mut manager = LockManager::new();
        assert!(manager.begin_pending_for_test(["a".to_string(), "b".to_string()]));
        let _ = manager.record_pending_frame("a");
        assert_eq!(manager.pending_target_count(), 1);
        let _ = manager.record_pending_frame("b");
        assert_eq!(manager.pending_target_count(), 0);
    }

    #[test]
    fn forget_pending_target_drains_just_like_record() {
        let mut manager = LockManager::new();
        assert!(manager.begin_pending_for_test(["a".to_string(), "b".to_string()]));
        let _ = manager.forget_pending_target("a");
        assert_eq!(manager.pending_target_count(), 1);
        let _ = manager.forget_pending_target("b");
        assert_eq!(manager.pending_target_count(), 0);
    }

    #[test]
    fn unlock_during_pending_clears_targets_and_locker_state() {
        let mut manager = LockManager::new();
        assert!(manager.begin_pending_for_test(["a".to_string()]));
        assert_eq!(manager.pending_target_count(), 1);
        assert!(!manager.has_pending_locker());
        assert!(manager.unlock());
        assert_eq!(manager.pending_target_count(), 0);
        assert!(!manager.has_pending_locker());
        assert_eq!(manager.phase(), &LockPhase::Unlocked);
    }
}
