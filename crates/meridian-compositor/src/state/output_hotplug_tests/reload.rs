#[test]
fn reload_layout_changes_primary_assignment() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-1",
        OutputPositionConfig::Auto,
        true,
        true,
    )]);
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=false workspace=0
drm-1: (1920,0 1920x1080) primary=true workspace=0"#
    );
}

#[test]
fn reload_layout_repositions_output_via_below() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-1",
        OutputPositionConfig::Below("drm-0".to_string()),
        false,
        true,
    )]);
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (0,1080 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn reload_layout_coord_pin_overrides_chain() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-1",
        OutputPositionConfig::Coord { x: 500, y: 300 },
        false,
        true,
    )]);
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (500,300 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn reload_layout_empty_entries_falls_back_to_auto_chain() {
    let entries = vec![entry_with(
        "drm-1",
        OutputPositionConfig::Coord { x: 500, y: 300 },
        false,
        true,
    )];
    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&entries);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    fixture.reload_layout_from_entries(&[]);
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (1920,0 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn reload_layout_safety_net_re_enables_all_disabled() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-0",
        OutputPositionConfig::Auto,
        false,
        false,
    )]);
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0"#
    );
}

#[test]
fn reload_layout_noop_when_registry_empty() {
    let mut fixture = OutputHotplugFixture::new();
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-0",
        OutputPositionConfig::Coord { x: 120, y: 80 },
        false,
        true,
    )]);
    assert_eq!(fixture.snapshot(), "");
}

#[test]
fn reload_layout_with_mode_change_safely_logs_only_in_harness() {
    let mut initial = entry_with("drm-0", OutputPositionConfig::Auto, true, true);
    initial.mode = Some(OutputModeConfig {
        width: 1920,
        height: 1080,
        refresh_millihz: Some(60_000),
    });
    let mut updated = entry_with("drm-0", OutputPositionConfig::Auto, true, true);
    updated.mode = Some(OutputModeConfig {
        width: 2560,
        height: 1440,
        refresh_millihz: Some(60_000),
    });

    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&[initial]);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    let before = fixture.snapshot();
    fixture.reload_layout_from_entries(&[updated]);
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn reload_with_enabled_false_simulates_disable_via_registry() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-1",
        OutputPositionConfig::Auto,
        false,
        false,
    )]);
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0"#
    );
}

#[test]
fn reload_re_enabling_output_re_adds_to_registry() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-1",
        OutputPositionConfig::Auto,
        false,
        false,
    )]);
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-1",
        OutputPositionConfig::Auto,
        false,
        true,
    )]);
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (1920,0 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn reload_safety_net_re_enables_all_disabled_via_harness() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-0",
        OutputPositionConfig::Auto,
        false,
        false,
    )]);
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0"#
    );
}
