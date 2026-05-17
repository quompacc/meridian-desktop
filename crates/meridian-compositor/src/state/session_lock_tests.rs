use super::{LockManager, LockPhase};

#[test]
fn default_phase_is_unlocked() {
    let manager = LockManager::new();
    assert_eq!(manager.phase(), &LockPhase::Unlocked);
}

#[test]
fn lock_request_transitions_to_pending_then_locked() {
    let mut manager = LockManager::new();
    assert!(manager.begin_lock());
    assert_eq!(manager.phase(), &LockPhase::Pending);
    assert!(manager.confirm_locked());
    assert_eq!(manager.phase(), &LockPhase::Locked);
}

#[test]
fn double_lock_request_is_rejected() {
    let mut manager = LockManager::new();
    assert!(manager.begin_lock());
    assert!(!manager.begin_lock());
    assert_eq!(manager.phase(), &LockPhase::Pending);
}

#[test]
fn confirm_without_pending_is_noop() {
    let mut manager = LockManager::new();
    assert!(!manager.confirm_locked());
    assert_eq!(manager.phase(), &LockPhase::Unlocked);
}

#[test]
fn unlock_from_locked_returns_to_unlocked_and_clears_surfaces() {
    let mut manager = LockManager::new();
    assert!(manager.begin_lock());
    assert!(manager.confirm_locked());
    assert!(manager.unlock());
    assert_eq!(manager.phase(), &LockPhase::Unlocked);
    assert_eq!(manager.surface_count(), 0);
}

#[test]
fn unlock_from_pending_returns_to_unlocked() {
    let mut manager = LockManager::new();
    assert!(manager.begin_lock());
    assert!(manager.unlock());
    assert_eq!(manager.phase(), &LockPhase::Unlocked);
}

#[test]
fn unlock_from_unlocked_is_noop() {
    let mut manager = LockManager::new();
    assert!(!manager.unlock());
    assert_eq!(manager.surface_count(), 0);
    assert_eq!(manager.phase(), &LockPhase::Unlocked);
}

#[test]
fn drop_surface_for_unknown_output_is_noop() {
    let mut manager = LockManager::new();
    assert!(!manager.drop_surface("out-unknown"));
    assert_eq!(manager.surface_count(), 0);
}

#[test]
fn full_lifecycle_without_surfaces() {
    let mut manager = LockManager::new();
    assert!(manager.begin_lock());
    assert!(manager.confirm_locked());
    assert!(manager.unlock());
    assert_eq!(manager.phase(), &LockPhase::Unlocked);
    assert_eq!(manager.surface_count(), 0);
}
