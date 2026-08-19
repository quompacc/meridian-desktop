#[test]
fn window_on_removed_output_stays_at_logical_position() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());

    let w1 = fixture.map_window("w1", 2000, 200, 300, 300, 0);
    assert_eq!(fixture.window_count(0), 1);
    assert_eq!(fixture.window_location("w1", 0), Some((2000, 200)));

    assert!(fixture.remove_output("drm-1"));
    fixture.refresh_all_spaces();

    assert_eq!(fixture.window_location("w1", 0), Some((2000, 200)));
    assert!(w1.entered().is_empty());
}

#[test]
fn window_on_remaining_output_undisturbed_by_sibling_removal() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());

    let w1 = fixture.map_window("w1", 200, 200, 300, 300, 0);
    assert_eq!(w1.entered(), vec!["drm-0".to_string()]);

    assert!(fixture.remove_output("drm-1"));
    fixture.refresh_all_spaces();

    assert_eq!(fixture.window_location("w1", 0), Some((200, 200)));
    assert_eq!(w1.entered(), vec!["drm-0".to_string()]);
}

#[test]
fn window_straddling_two_outputs_loses_one_on_removal() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());

    let w1 = fixture.map_window("w1", 1800, 100, 400, 400, 0);
    assert_eq!(w1.entered(), vec!["drm-0".to_string(), "drm-1".to_string()]);

    assert!(fixture.remove_output("drm-1"));
    fixture.refresh_all_spaces();

    assert_eq!(w1.entered(), vec!["drm-0".to_string()]);
    assert_eq!(fixture.window_location("w1", 0), Some((1800, 100)));
}

#[test]
fn reconfigure_geometry_keeps_window_position_updates_overlap() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());

    let w1 = fixture.map_window("w1", 1800, 100, 400, 400, 0);
    assert_eq!(w1.entered(), vec!["drm-0".to_string(), "drm-1".to_string()]);

    assert!(fixture.reconfigure_output("drm-0", 2400, 1080));
    fixture.refresh_all_spaces();

    assert_eq!(fixture.window_location("w1", 0), Some((1800, 100)));
    assert_eq!(w1.entered(), vec!["drm-0".to_string()]);
}

#[test]
fn cyclic_add_remove_add_does_not_leak_entered_outputs() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());

    let w1 = fixture.map_window("w1", 100, 100, 200, 200, 0);
    assert_eq!(w1.entered(), vec!["drm-0".to_string()]);

    for _ in 0..3 {
        assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
        fixture.refresh_all_spaces();
        assert!(fixture.remove_output("drm-1"));
        fixture.refresh_all_spaces();
    }

    assert_eq!(w1.entered(), vec!["drm-0".to_string()]);
}

#[test]
fn multiple_windows_distributed_across_outputs_snapshot() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());

    let w_left = fixture.map_window("w_left", 100, 100, 200, 200, 0);
    let w_right = fixture.map_window("w_right", 2100, 100, 200, 200, 0);
    fixture.refresh_all_spaces();

    assert_eq!(w_left.entered(), vec!["drm-0".to_string()]);
    assert_eq!(w_right.entered(), vec!["drm-1".to_string()]);

    assert!(fixture.remove_output("drm-0"));
    fixture.refresh_all_spaces();

    assert!(w_left.entered().is_empty());
    assert_eq!(w_right.entered(), vec!["drm-1".to_string()]);
}

#[test]
fn multi_workspace_window_survives_output_remove() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());

    let w_a = fixture.map_window("w_a", 2100, 100, 200, 200, 0);
    let w_b = fixture.map_window("w_b", 2100, 100, 200, 200, 5);
    fixture.refresh_all_spaces();

    assert_eq!(w_a.entered(), vec!["drm-1".to_string()]);
    assert_eq!(w_b.entered(), vec!["drm-1".to_string()]);

    assert!(fixture.remove_output("drm-1"));
    fixture.refresh_all_spaces();

    assert_eq!(fixture.window_location("w_a", 0), Some((2100, 100)));
    assert_eq!(fixture.window_location("w_b", 5), Some((2100, 100)));
    assert!(w_a.entered().is_empty());
    assert!(w_b.entered().is_empty());
}

#[test]
fn reload_position_change_to_below_window_loses_overlap() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());

    let w1 = fixture.map_window("w1", 2100, 100, 200, 200, 0);
    fixture.refresh_all_spaces();
    assert_eq!(w1.entered(), vec!["drm-1".to_string()]);

    fixture.reload_layout_from_entries(&[entry_with(
        "drm-1",
        OutputPositionConfig::Below("drm-0".to_string()),
        false,
        true,
    )]);
    fixture.refresh_all_spaces();

    assert_eq!(fixture.window_location("w1", 0), Some((2100, 100)));
    assert!(w1.entered().is_empty());
}

#[test]
fn reload_position_change_brings_output_under_window() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());

    let w1 = fixture.map_window("w1", 300, 1200, 200, 200, 0);
    fixture.refresh_all_spaces();
    assert!(w1.entered().is_empty());

    fixture.reload_layout_from_entries(&[entry_with(
        "drm-1",
        OutputPositionConfig::Coord { x: 0, y: 1080 },
        false,
        true,
    )]);
    fixture.refresh_all_spaces();

    assert_eq!(fixture.window_location("w1", 0), Some((300, 1200)));
    assert_eq!(w1.entered(), vec!["drm-1".to_string()]);
}

#[test]
fn disable_then_re_enable_output_window_overlap_restored() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());

    let w1 = fixture.map_window("w1", 2100, 100, 200, 200, 0);
    fixture.refresh_all_spaces();
    assert_eq!(w1.entered(), vec!["drm-1".to_string()]);

    fixture.reload_layout_from_entries(&[entry_with(
        "drm-1",
        OutputPositionConfig::Auto,
        false,
        false,
    )]);
    fixture.refresh_all_spaces();
    assert!(w1.entered().is_empty());

    fixture.reload_layout_from_entries(&[entry_with(
        "drm-1",
        OutputPositionConfig::Auto,
        false,
        true,
    )]);
    fixture.refresh_all_spaces();

    assert_eq!(w1.entered(), vec!["drm-1".to_string()]);
    assert_eq!(fixture.window_location("w1", 0), Some((2100, 100)));
}

#[test]
fn window_moved_between_workspaces_preserves_overlap() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());

    let w1 = fixture.map_window("w1", 2100, 100, 200, 200, 0);
    fixture.refresh_all_spaces();
    assert_eq!(w1.entered(), vec!["drm-1".to_string()]);
    assert_eq!(fixture.window_count(0), 1);
    assert_eq!(fixture.window_count(3), 0);

    assert!(fixture.move_window_between_workspaces("w1", 0, 3));

    assert_eq!(fixture.window_count(0), 0);
    assert_eq!(fixture.window_count(3), 1);
    assert_eq!(fixture.window_location("w1", 3), Some((2100, 100)));
    assert_eq!(w1.entered(), vec!["drm-1".to_string()]);
}

#[test]
fn reconfigure_grow_brings_window_into_output() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());

    let w1 = fixture.map_window("w1", 2000, 100, 200, 200, 0);
    fixture.refresh_all_spaces();
    assert!(w1.entered().is_empty());

    assert!(fixture.reconfigure_output("drm-0", 2560, 1080));
    fixture.refresh_all_spaces();

    assert_eq!(fixture.window_location("w1", 0), Some((2000, 100)));
    assert_eq!(w1.entered(), vec!["drm-0".to_string()]);
}

#[test]
fn safety_net_re_enable_keeps_window_overlap() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());

    let w1 = fixture.map_window("w1", 100, 100, 200, 200, 0);
    fixture.refresh_all_spaces();
    assert_eq!(w1.entered(), vec!["drm-0".to_string()]);

    fixture.reload_layout_from_entries(&[entry_with(
        "drm-0",
        OutputPositionConfig::Auto,
        false,
        false,
    )]);
    fixture.refresh_all_spaces();

    assert_eq!(fixture.window_location("w1", 0), Some((100, 100)));
    assert_eq!(w1.entered(), vec!["drm-0".to_string()]);
}

fn entry_with(
    name: &str,
    position: OutputPositionConfig,
    primary: bool,
    enabled: bool,
) -> OutputEntry {
    let mut entry = OutputEntry::defaults_for(name);
    entry.position = position;
    entry.primary = primary;
    entry.enabled = enabled;
    entry
}
