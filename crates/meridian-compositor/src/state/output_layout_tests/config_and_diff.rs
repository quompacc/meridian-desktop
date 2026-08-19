#[test]
fn from_config_entries_builds_layout_in_order() {
    let entries = vec![
        config_entry("drm-1", OutputPositionConfig::Auto, false, true),
        config_entry("drm-0", OutputPositionConfig::Auto, true, true),
    ];

    let layout = OutputLayout::from_config_entries(&entries);

    assert_eq!(layout.placements.len(), 2);
    assert_eq!(layout.placements[0].name, "drm-1");
    assert_eq!(layout.placements[1].name, "drm-0");
}

#[test]
fn empty_layout_single_output_matches_legacy_x_offset_behavior() {
    let layout = OutputLayout::default();
    let connected = vec![connected("drm-0", 1920, 1080)];

    let resolved = layout.resolve(&connected);

    assert_eq!(resolved.len(), 1);
    assert_eq!((resolved[0].x, resolved[0].y), (0, 0));
    assert_eq!((resolved[0].width, resolved[0].height), (1920, 1080));
    assert!(resolved[0].primary);
    assert!(resolved[0].enabled);
}

#[test]
fn empty_layout_two_outputs_matches_legacy_x_offset_chain() {
    let layout = OutputLayout::default();
    let connected = vec![
        connected("drm-0", 1920, 1080),
        connected("drm-1", 2560, 1440),
    ];

    let resolved = layout.resolve(&connected);

    assert_eq!(resolved[0].x, 0);
    assert_eq!(resolved[1].x, 1920);
}

#[test]
fn empty_layout_three_outputs_matches_legacy_x_offset_chain() {
    let layout = OutputLayout::default();
    let connected = vec![
        connected("drm-0", 1920, 1080),
        connected("drm-1", 2560, 1440),
        connected("drm-2", 1440, 900),
    ];

    let resolved = layout.resolve(&connected);

    assert_eq!(resolved[0].x, 0);
    assert_eq!(resolved[1].x, 1920);
    assert_eq!(resolved[2].x, 4480);
}

#[test]
fn from_config_entries_empty_yields_empty_layout() {
    let layout = OutputLayout::from_config_entries(&[]);
    assert!(layout.placements.is_empty());
}

#[test]
fn parse_output_transform_recognizes_all_known_variants() {
    assert_eq!(super::parse_output_transform("normal"), Transform::Normal);
    assert_eq!(super::parse_output_transform("90"), Transform::_90);
    assert_eq!(super::parse_output_transform("180"), Transform::_180);
    assert_eq!(super::parse_output_transform("270"), Transform::_270);
    assert_eq!(super::parse_output_transform("flipped"), Transform::Flipped);
    assert_eq!(
        super::parse_output_transform("flipped-90"),
        Transform::Flipped90
    );
    assert_eq!(
        super::parse_output_transform("flipped-180"),
        Transform::Flipped180
    );
    assert_eq!(
        super::parse_output_transform("flipped-270"),
        Transform::Flipped270
    );
}

#[test]
fn parse_output_transform_is_case_insensitive() {
    assert_eq!(super::parse_output_transform("NoRmAl"), Transform::Normal);
    assert_eq!(
        super::parse_output_transform("FLIPPED-180"),
        Transform::Flipped180
    );
}

#[test]
fn parse_output_transform_unknown_falls_back_to_normal() {
    assert_eq!(super::parse_output_transform("weird"), Transform::Normal);
}

#[test]
fn parse_output_transform_trims_whitespace() {
    assert_eq!(super::parse_output_transform("  90  "), Transform::_90);
}

#[test]
fn safety_net_force_enables_when_all_disabled() {
    let layout = OutputLayout {
        placements: vec![
            placement("A", OutputPosition::Auto, false, false),
            placement("B", OutputPosition::Auto, false, false),
        ],
    };
    let connected = vec![connected("A", 100, 100), connected("B", 100, 100)];
    let mut resolved = layout.resolve(&connected);

    MeridianState::enforce_at_least_one_enabled(&mut resolved);

    assert!(resolved.iter().all(|output| output.enabled));
    assert!(resolved.iter().any(|output| output.primary));
}

#[test]
fn safety_net_noop_when_at_least_one_enabled() {
    let layout = OutputLayout {
        placements: vec![
            placement("A", OutputPosition::Auto, false, false),
            placement("B", OutputPosition::Auto, true, true),
        ],
    };
    let connected = vec![connected("A", 100, 100), connected("B", 100, 100)];
    let mut resolved = layout.resolve(&connected);
    let before = resolved.clone();

    MeridianState::enforce_at_least_one_enabled(&mut resolved);

    assert_eq!(resolved, before);
    assert!(!resolved[0].enabled);
    assert!(resolved[1].enabled);
    assert!(resolved[1].primary);
}

#[test]
fn safety_net_noop_when_resolved_empty() {
    let mut resolved: Vec<ResolvedOutput> = Vec::new();
    MeridianState::enforce_at_least_one_enabled(&mut resolved);
    assert!(resolved.is_empty());
}

#[test]
fn single_monitor_default_config_remains_enabled_with_normal_transform_scale_one() {
    let entries: Vec<OutputEntry> = Vec::new();
    let layout = OutputLayout::from_config_entries(&entries);
    let connected = vec![connected("drm-0", 1920, 1080)];

    let resolved = layout.resolve(&connected);

    assert_eq!(resolved.len(), 1);
    assert!(resolved[0].enabled);
    assert!(resolved[0].primary);
    assert_eq!((resolved[0].x, resolved[0].y), (0, 0));
}

#[test]
fn single_monitor_disabled_via_config_safety_net_re_enables() {
    let entries = vec![OutputEntry {
        name: "drm-0".to_string(),
        enabled: false,
        ..OutputEntry::defaults_for("drm-0")
    }];
    let layout = OutputLayout::from_config_entries(&entries);
    let connected = vec![connected("drm-0", 1920, 1080)];

    let raw = layout.resolve(&connected);
    assert!(!raw[0].enabled);

    let mut adjusted = raw;
    MeridianState::enforce_at_least_one_enabled(&mut adjusted);
    assert!(adjusted[0].enabled);
    assert!(adjusted[0].primary);
}

#[test]
fn diff_no_change_when_both_none() {
    let diff = detect_output_reload_diff(None, None);
    assert_eq!(
        diff,
        super::OutputReloadDiff {
            mode_changed: false,
            enabled_changed: false
        }
    );
}

#[test]
fn diff_no_change_when_identical_modes() {
    let prev = config_entry_with_mode("drm-0", Some(mode(1920, 1080, Some(60_000))), true);
    let next = config_entry_with_mode("drm-0", Some(mode(1920, 1080, Some(60_000))), true);

    let diff = detect_output_reload_diff(Some(&prev), Some(&next));

    assert!(!diff.mode_changed);
    assert!(!diff.enabled_changed);
}

#[test]
fn diff_mode_changed_when_dimensions_differ() {
    let prev = config_entry_with_mode("drm-0", Some(mode(1920, 1080, Some(60_000))), true);
    let next = config_entry_with_mode("drm-0", Some(mode(2560, 1440, Some(60_000))), true);

    let diff = detect_output_reload_diff(Some(&prev), Some(&next));

    assert!(diff.mode_changed);
    assert!(!diff.enabled_changed);
}

#[test]
fn diff_mode_changed_when_refresh_differs() {
    let prev = config_entry_with_mode("drm-0", Some(mode(1920, 1080, Some(60_000))), true);
    let next = config_entry_with_mode("drm-0", Some(mode(1920, 1080, Some(144_000))), true);

    let diff = detect_output_reload_diff(Some(&prev), Some(&next));

    assert!(diff.mode_changed);
    assert!(!diff.enabled_changed);
}

#[test]
fn diff_mode_changed_when_one_side_is_none() {
    let prev = config_entry_with_mode("drm-0", Some(mode(1920, 1080, Some(60_000))), true);
    let next = config_entry_with_mode("drm-0", None, true);

    let diff = detect_output_reload_diff(Some(&prev), Some(&next));

    assert!(diff.mode_changed);
    assert!(!diff.enabled_changed);
}

#[test]
fn diff_enabled_changed_toggles() {
    let prev = config_entry_with_mode("drm-0", None, true);
    let next = config_entry_with_mode("drm-0", None, false);

    let diff = detect_output_reload_diff(Some(&prev), Some(&next));

    assert!(!diff.mode_changed);
    assert!(diff.enabled_changed);
}

#[test]
fn diff_enabled_unchanged_when_default_true_both_sides() {
    let diff = detect_output_reload_diff(None, None);

    assert!(!diff.enabled_changed);
}

#[test]
fn diff_combined_mode_and_enabled_change() {
    let prev = config_entry_with_mode("drm-0", Some(mode(1920, 1080, Some(60_000))), true);
    let next = config_entry_with_mode("drm-0", Some(mode(2560, 1440, Some(75_000))), false);

    let diff = detect_output_reload_diff(Some(&prev), Some(&next));

    assert!(diff.mode_changed);
    assert!(diff.enabled_changed);
}
