use super::{DesktopApp, LauncherState};

#[test]
fn launcher_state_tracks_open_close_and_apps() {
    let apps = vec![DesktopApp::new(
        "Firefox".to_string(),
        vec!["firefox".to_string()],
        false,
    )];
    let mut state = LauncherState::new_with_apps(apps);
    assert!(!state.open);
    assert_eq!(state.apps.len(), 1);
    assert!(state.toggle());
    state.close();
    assert!(!state.open);
}
