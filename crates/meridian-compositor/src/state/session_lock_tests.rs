use super::{LockManager, LockPhase};

#[test]
fn default_phase_is_unlocked() {
    let manager = LockManager::new();
    assert_eq!(manager.phase(), &LockPhase::Unlocked);
}

#[test]
fn lock_request_transitions_to_pending_then_locked() {
    let mut manager = LockManager::new();
    assert!(manager.begin_pending_for_test(["out-0".to_string()]));
    assert_eq!(manager.phase(), &LockPhase::Pending);
    assert!(manager.confirm_locked());
    assert_eq!(manager.phase(), &LockPhase::Locked);
}

#[test]
fn double_lock_request_is_rejected() {
    let mut manager = LockManager::new();
    assert!(manager.begin_pending_for_test(["out-0".to_string()]));
    assert!(!manager.begin_pending_for_test(["out-1".to_string()]));
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
    assert!(manager.begin_pending_for_test(["out-0".to_string()]));
    assert!(manager.confirm_locked());
    assert!(manager.unlock());
    assert_eq!(manager.phase(), &LockPhase::Unlocked);
    assert_eq!(manager.surface_count(), 0);
}

#[test]
fn unlock_from_pending_returns_to_unlocked() {
    let mut manager = LockManager::new();
    assert!(manager.begin_pending_for_test(["out-0".to_string()]));
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
    assert!(manager.begin_pending_for_test(["out-0".to_string()]));
    assert!(manager.confirm_locked());
    assert!(manager.unlock());
    assert_eq!(manager.phase(), &LockPhase::Unlocked);
    assert_eq!(manager.surface_count(), 0);
}

#[test]
fn lock_with_targets_via_test_helper_then_record_all_reaches_locked() {
    let mut manager = LockManager::new();
    assert!(manager.begin_pending_for_test(["out-0".to_string(), "out-1".to_string()]));
    assert_eq!(manager.pending_target_count(), 2);
    let _ = manager.record_pending_frame("out-0");
    assert_eq!(manager.pending_target_count(), 1);
    let _ = manager.record_pending_frame("out-1");
    assert_eq!(manager.pending_target_count(), 0);
    assert!(manager.confirm_locked());
    assert_eq!(manager.phase(), &LockPhase::Locked);
}

#[test]
fn lock_then_output_removed_drains_target() {
    let mut manager = LockManager::new();
    assert!(manager.begin_pending_for_test(["out-0".to_string(), "out-1".to_string()]));
    let _ = manager.forget_pending_target("out-1");
    assert_eq!(manager.pending_target_count(), 1);
    let _ = manager.forget_pending_target("out-0");
    assert_eq!(manager.pending_target_count(), 0);
}
