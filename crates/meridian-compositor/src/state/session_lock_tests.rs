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
    manager.register_surface("out-0");
    assert!(manager.confirm_locked());
    assert!(manager.unlock());
    assert_eq!(manager.phase(), &LockPhase::Unlocked);
    assert_eq!(manager.surface_count(), 0);
}

#[test]
fn register_surfaces_for_multiple_outputs_then_unlock_clears_all() {
    let mut manager = LockManager::new();
    manager.register_surface("out-0");
    manager.register_surface("out-1");
    assert_eq!(manager.surface_count(), 2);
    assert!(!manager.unlock());
    assert_eq!(manager.surface_count(), 0);
}

#[test]
fn drop_surface_for_one_output_keeps_others() {
    let mut manager = LockManager::new();
    manager.register_surface("out-0");
    manager.register_surface("out-1");
    assert!(manager.drop_surface("out-1"));
    assert_eq!(manager.surface_count(), 1);
    assert!(manager.drop_surface("out-0"));
    assert_eq!(manager.surface_count(), 0);
}

#[test]
fn full_lifecycle_with_surfaces() {
    let mut manager = LockManager::new();
    assert!(manager.begin_lock());
    manager.register_surface("out-0");
    manager.register_surface("out-1");
    assert!(manager.confirm_locked());
    assert!(manager.drop_surface("out-1"));
    assert!(manager.unlock());
    assert_eq!(manager.phase(), &LockPhase::Unlocked);
    assert_eq!(manager.surface_count(), 0);
}
