use std::collections::HashMap;

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
    surfaces_by_output: HashMap<String, bool>,
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

    /// Register lock surface marker for an output.
    pub fn register_surface(&mut self, output_name: &str) -> bool {
        self.surfaces_by_output
            .insert(output_name.to_string(), true);
        true
    }

    /// Drop lock surface marker for an output.
    pub fn drop_surface(&mut self, output_name: &str) -> bool {
        self.surfaces_by_output.remove(output_name).is_some()
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
        manager.register_surface("out-0");
        manager.confirm_locked();
        assert_eq!(manager.surface_count(), 1);
        assert!(manager.unlock());
        assert_eq!(manager.phase(), &LockPhase::Unlocked);
        assert_eq!(manager.surface_count(), 0);
        assert!(!manager.is_locked_or_pending());
    }

    #[test]
    fn register_surface_then_drop_surface() {
        let mut manager = LockManager::new();
        assert!(manager.register_surface("out-0"));
        assert_eq!(manager.surface_count(), 1);
        assert!(manager.drop_surface("out-0"));
        assert_eq!(manager.surface_count(), 0);
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
