#[test]
fn local_capture_output_empty_state_is_none() {
    assert_eq!(select_local_capture_output_name(&[]), None);
}

#[test]
fn local_capture_output_prefers_primary_over_first() {
    let outputs = vec![
        OutputWorkspaceState {
            output_id: 1,
            output_name: Some("HDMI-A-1".to_string()),
            primary: false,
            ..Default::default()
        },
        OutputWorkspaceState {
            output_id: 2,
            output_name: Some("eDP-1".to_string()),
            primary: true,
            ..Default::default()
        },
    ];
    assert_eq!(select_local_capture_output_name(&outputs), Some("eDP-1"));
}

#[test]
fn local_capture_output_falls_back_to_first_without_primary() {
    let outputs = vec![
        OutputWorkspaceState {
            output_id: 1,
            output_name: Some("DP-1".to_string()),
            primary: false,
            ..Default::default()
        },
        OutputWorkspaceState {
            output_id: 2,
            output_name: Some("DP-2".to_string()),
            primary: false,
            ..Default::default()
        },
    ];
    assert_eq!(select_local_capture_output_name(&outputs), Some("DP-1"));
}

#[test]
fn local_capture_output_none_when_selected_entry_has_no_name() {
    // Synthetic entries (OutputWorkspaceChanged for an unknown output) can
    // carry no name; the caller then falls back to the first wl_output.
    let outputs = vec![OutputWorkspaceState {
        output_id: 7,
        output_name: None,
        primary: true,
        ..Default::default()
    }];
    assert_eq!(select_local_capture_output_name(&outputs), None);
}

#[test]
fn output_workspace_changed_updates_known_output() {
    let mut focused_output_id = Some(1);
    let mut output_workspaces = vec![OutputWorkspaceState {
        output_id: 1,
        output_name: Some("eDP-1".to_string()),
        active_workspace: 1,
        primary: true,
        focused: true,
        ..Default::default()
    }];
    let mut output_workspace_state_available = false;
    let mut workspace_indicator_dirty = false;

    apply_output_workspace_changed_state(
        &mut focused_output_id,
        &mut output_workspaces,
        &mut output_workspace_state_available,
        &mut workspace_indicator_dirty,
        OutputWorkspaceChangedInput {
            output_id: 1,
            output_name: Some("eDP-1".to_string()),
            workspace: 3,
            focused: true,
        },
    );

    assert_eq!(output_workspaces.len(), 1);
    assert_eq!(output_workspaces[0].active_workspace, 3);
    assert_eq!(focused_output_id, Some(1));
    assert!(output_workspace_state_available);
    assert!(workspace_indicator_dirty);
}

#[test]
fn output_workspace_changed_unknown_output_is_added_safely() {
    let mut focused_output_id = None;
    let mut output_workspaces = Vec::new();
    let mut output_workspace_state_available = false;
    let mut workspace_indicator_dirty = false;

    apply_output_workspace_changed_state(
        &mut focused_output_id,
        &mut output_workspaces,
        &mut output_workspace_state_available,
        &mut workspace_indicator_dirty,
        OutputWorkspaceChangedInput {
            output_id: 7,
            output_name: None,
            workspace: 5,
            focused: false,
        },
    );

    assert_eq!(output_workspaces.len(), 1);
    assert_eq!(output_workspaces[0].output_id, 7);
    assert_eq!(output_workspaces[0].active_workspace, 5);
    assert_eq!(output_workspaces[0].output_name, None);
    assert!(!output_workspaces[0].focused);
    assert!(output_workspace_state_available);
    assert!(workspace_indicator_dirty);
}

#[test]
fn output_workspace_changed_clamps_workspace_and_handles_focus_drop() {
    let mut focused_output_id = Some(3);
    let mut output_workspaces = vec![OutputWorkspaceState {
        output_id: 3,
        output_name: Some("DP-1".to_string()),
        active_workspace: 2,
        primary: false,
        focused: true,
        ..Default::default()
    }];
    let mut output_workspace_state_available = false;
    let mut workspace_indicator_dirty = false;

    apply_output_workspace_changed_state(
        &mut focused_output_id,
        &mut output_workspaces,
        &mut output_workspace_state_available,
        &mut workspace_indicator_dirty,
        OutputWorkspaceChangedInput {
            output_id: 3,
            output_name: None,
            workspace: 42,
            focused: false,
        },
    );

    assert_eq!(output_workspaces[0].active_workspace, 9);
    assert_eq!(focused_output_id, None);
    assert!(output_workspace_state_available);
    assert!(workspace_indicator_dirty);
}

#[test]
fn output_workspace_snapshot_clamps_workspace_values() {
    let mut focused_output_id = None;
    let mut output_workspaces = Vec::new();
    let mut output_workspace_state_available = false;
    let mut workspace_indicator_dirty = false;

    apply_output_workspace_snapshot_state(
        &mut focused_output_id,
        &mut output_workspaces,
        &mut output_workspace_state_available,
        &mut workspace_indicator_dirty,
        None,
        vec![OutputWorkspaceState {
            output_id: 1,
            output_name: Some("eDP-1".to_string()),
            active_workspace: 0,
            primary: true,
            focused: false,
            ..Default::default()
        }],
    );

    assert_eq!(output_workspaces[0].active_workspace, 1);
    assert!(output_workspace_state_available);
    assert!(workspace_indicator_dirty);
}

#[test]
fn legacy_workspace_changed_still_works_with_and_without_output_aware_state() {
    let mut active = 1u8;
    apply_workspace_changed(&mut active, 3);
    assert_eq!(active, 3);

    // Legacy update remains valid even if output-aware state exists;
    // this must not mutate output-aware structures.
    let output_workspaces = vec![OutputWorkspaceState {
        output_id: 1,
        output_name: Some("eDP-1".to_string()),
        active_workspace: 2,
        primary: true,
        focused: true,
        ..Default::default()
    }];
    let before = output_workspaces.clone();
    apply_workspace_changed(&mut active, 4);
    assert_eq!(active, 4);
    assert_eq!(output_workspaces, before);
}

#[test]
fn panel_active_workspace_prefers_focused_output_id() {
    let active = select_panel_active_workspace(
        1,
        true,
        Some(2),
        &[
            OutputWorkspaceState {
                output_id: 1,
                output_name: Some("eDP-1".to_string()),
                active_workspace: 3,
                primary: true,
                focused: false,
                ..Default::default()
            },
            OutputWorkspaceState {
                output_id: 2,
                output_name: Some("HDMI-A-1".to_string()),
                active_workspace: 5,
                primary: false,
                focused: true,
                ..Default::default()
            },
        ],
    );
    assert_eq!(active, 5);
}

#[test]
fn panel_active_workspace_falls_back_to_focused_flag() {
    let active = select_panel_active_workspace(
        1,
        true,
        Some(99),
        &[OutputWorkspaceState {
            output_id: 1,
            output_name: Some("eDP-1".to_string()),
            active_workspace: 4,
            primary: true,
            focused: true,
            ..Default::default()
        }],
    );
    assert_eq!(active, 4);
}

#[test]
fn panel_active_workspace_falls_back_to_primary_output() {
    let active = select_panel_active_workspace(
        2,
        true,
        None,
        &[
            OutputWorkspaceState {
                output_id: 1,
                output_name: Some("eDP-1".to_string()),
                active_workspace: 6,
                primary: true,
                focused: false,
                ..Default::default()
            },
            OutputWorkspaceState {
                output_id: 2,
                output_name: Some("HDMI-A-1".to_string()),
                active_workspace: 3,
                primary: false,
                focused: false,
                ..Default::default()
            },
        ],
    );
    assert_eq!(active, 6);
}

#[test]
fn panel_active_workspace_falls_back_to_first_output() {
    let active = select_panel_active_workspace(
        2,
        true,
        None,
        &[
            OutputWorkspaceState {
                output_id: 10,
                output_name: Some("left".to_string()),
                active_workspace: 7,
                primary: false,
                focused: false,
                ..Default::default()
            },
            OutputWorkspaceState {
                output_id: 11,
                output_name: Some("right".to_string()),
                active_workspace: 1,
                primary: false,
                focused: false,
                ..Default::default()
            },
        ],
    );
    assert_eq!(active, 7);
}

#[test]
fn panel_active_workspace_falls_back_to_legacy_when_unavailable() {
    let active = select_panel_active_workspace(
        8,
        false,
        Some(2),
        &[OutputWorkspaceState {
            output_id: 2,
            output_name: Some("HDMI-A-1".to_string()),
            active_workspace: 3,
            primary: false,
            focused: true,
            ..Default::default()
        }],
    );
    assert_eq!(active, 8);
}

#[test]
fn panel_active_workspace_normalizes_out_of_range() {
    let active = select_panel_active_workspace(
        1,
        true,
        Some(1),
        &[OutputWorkspaceState {
            output_id: 1,
            output_name: Some("eDP-1".to_string()),
            active_workspace: 99,
            primary: true,
            focused: true,
            ..Default::default()
        }],
    );
    assert_eq!(active, 9);
}
