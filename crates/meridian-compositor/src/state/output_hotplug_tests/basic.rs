#[test]
fn single_output_add_yields_zero_origin_primary() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0"#
    );
}

#[test]
fn two_outputs_default_layout_chains_horizontally() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 2560, 1440).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (1920,0 2560x1440) primary=false workspace=0"#
    );
}

#[test]
fn remove_primary_falls_back_to_remaining() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 2560, 1440).is_some());
    assert!(fixture.remove_output("drm-0"));
    assert_eq!(
        fixture.snapshot(),
        r#"drm-1: (1920,0 2560x1440) primary=true workspace=0"#
    );
}

#[test]
fn add_remove_add_is_idempotent() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    assert!(fixture.remove_output("drm-0"));
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-1: (0,0 1920x1080) primary=true workspace=0
drm-0: (1920,0 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn remove_unknown_output_is_safe_noop() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(!fixture.remove_output("does-not-exist"));
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0"#
    );
}

#[test]
fn reconfigure_changes_width_keeps_chain_after() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    assert!(fixture.reconfigure_output("drm-0", 2560, 1440));
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 2560x1440) primary=true workspace=0
drm-1: (2560,0 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn reconfigure_unknown_output_is_safe_noop() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(!fixture.reconfigure_output("does-not-exist", 100, 100));
}

#[test]
fn reconfigure_same_size_is_stable() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    let before = fixture.snapshot();
    assert!(fixture.reconfigure_output("drm-0", 1920, 1080));
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn layout_with_explicit_primary_overrides_first_default() {
    let entries = vec![entry_with("drm-1", OutputPositionConfig::Auto, true, true)];
    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&entries);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=false workspace=0
drm-1: (1920,0 1920x1080) primary=true workspace=0"#
    );
}

#[test]
fn layout_right_of_chain_three_outputs() {
    let entries = vec![
        entry_with(
            "drm-1",
            OutputPositionConfig::RightOf("drm-0".to_string()),
            false,
            true,
        ),
        entry_with(
            "drm-2",
            OutputPositionConfig::RightOf("drm-1".to_string()),
            false,
            true,
        ),
    ];
    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&entries);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-2", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (1920,0 1920x1080) primary=false workspace=0
drm-2: (3840,0 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn layout_below_chain_two_outputs() {
    let entries = vec![entry_with(
        "drm-1",
        OutputPositionConfig::Below("drm-0".to_string()),
        false,
        true,
    )];
    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&entries);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (0,1080 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn layout_coord_position_pins_exact_xy() {
    let entries = vec![entry_with(
        "drm-1",
        OutputPositionConfig::Coord { x: 500, y: 300 },
        false,
        true,
    )];
    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&entries);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (500,300 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn layout_dangling_reference_falls_back_to_auto() {
    let entries = vec![entry_with(
        "drm-1",
        OutputPositionConfig::RightOf("UNKNOWN".to_string()),
        false,
        true,
    )];
    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&entries);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (1920,0 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn safety_net_triggers_when_all_outputs_disabled_in_layout() {
    let entries = vec![entry_with(
        "drm-0",
        OutputPositionConfig::Auto,
        false,
        false,
    )];
    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&entries);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0"#
    );
}

#[test]
fn cyclic_add_remove_stability() {
    let mut fixture = OutputHotplugFixture::new();
    for _ in 0..5 {
        assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
        assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
        assert!(fixture.remove_output("drm-1"));
        assert!(fixture.remove_output("drm-0"));
        assert!(fixture.registry.list().is_empty());
        assert!(fixture
            .workspaces
            .focused_output(&fixture.registry)
            .is_none());
        assert!(!fixture
            .workspaces
            .has_stale_focused_output(&fixture.registry));
    }
}

#[test]
fn four_outputs_complex_layout_snapshot() {
    let entries = vec![
        entry_with(
            "drm-1",
            OutputPositionConfig::RightOf("drm-0".to_string()),
            false,
            true,
        ),
        entry_with(
            "drm-2",
            OutputPositionConfig::Below("drm-0".to_string()),
            false,
            true,
        ),
        entry_with(
            "drm-3",
            OutputPositionConfig::RightOf("drm-2".to_string()),
            false,
            true,
        ),
    ];
    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&entries);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-2", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-3", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (1920,0 1920x1080) primary=false workspace=0
drm-2: (0,1080 1920x1080) primary=false workspace=0
drm-3: (1920,1080 1920x1080) primary=false workspace=0"#
    );
}
