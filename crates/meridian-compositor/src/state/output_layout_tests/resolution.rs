#[test]
fn empty_layout_two_outputs_chains_horizontally() {
    let layout = OutputLayout::default();
    let connected = vec![
        connected("eDP-1", 1920, 1080),
        connected("HDMI-A-1", 2560, 1440),
    ];

    let resolved_outputs = layout.resolve(&connected);

    assert_eq!(
        resolved_outputs,
        vec![
            expected_output("eDP-1", 0, 0, 1920, 1080, true, true),
            expected_output("HDMI-A-1", 1920, 0, 2560, 1440, false, true),
        ]
    );
}

#[test]
fn coord_position_places_at_exact_xy() {
    let layout = OutputLayout {
        placements: vec![placement(
            "HDMI-A-1",
            OutputPosition::Coord { x: 100, y: 200 },
            false,
            true,
        )],
    };
    let connected = vec![
        connected("eDP-1", 1920, 1080),
        connected("HDMI-A-1", 2560, 1440),
    ];

    let resolved = layout.resolve(&connected);

    assert_eq!(resolved[0].x, 0);
    assert_eq!(resolved[0].y, 0);
    assert_eq!(resolved[1].x, 100);
    assert_eq!(resolved[1].y, 200);
}

#[test]
fn right_of_resolves_to_target_right_edge() {
    let layout = OutputLayout {
        placements: vec![placement(
            "HDMI-A-1",
            OutputPosition::RightOf("eDP-1".to_string()),
            false,
            true,
        )],
    };
    let connected = vec![
        connected("eDP-1", 1920, 1080),
        connected("HDMI-A-1", 2560, 1440),
    ];

    let resolved = layout.resolve(&connected);

    assert_eq!(resolved[1].x, 1920);
    assert_eq!(resolved[1].y, 0);
}

#[test]
fn left_of_uses_self_width() {
    let layout = OutputLayout {
        placements: vec![
            placement(
                "eDP-1",
                OutputPosition::Coord { x: 2000, y: 0 },
                false,
                true,
            ),
            placement(
                "HDMI-A-1",
                OutputPosition::LeftOf("eDP-1".to_string()),
                false,
                true,
            ),
        ],
    };
    let connected = vec![
        connected("eDP-1", 1920, 1080),
        connected("HDMI-A-1", 1280, 720),
    ];

    let resolved = layout.resolve(&connected);

    assert_eq!(resolved[1].x, 720);
    assert_eq!(resolved[1].y, 0);
}

#[test]
fn below_stacks_vertically() {
    let layout = OutputLayout {
        placements: vec![placement(
            "HDMI-A-1",
            OutputPosition::Below("eDP-1".to_string()),
            false,
            true,
        )],
    };
    let connected = vec![
        connected("eDP-1", 1920, 1080),
        connected("HDMI-A-1", 1920, 1080),
    ];

    let resolved = layout.resolve(&connected);

    assert_eq!(resolved[1].x, 0);
    assert_eq!(resolved[1].y, 1080);
}

#[test]
fn above_uses_self_height() {
    let layout = OutputLayout {
        placements: vec![
            placement(
                "eDP-1",
                OutputPosition::Coord { x: 0, y: 1080 },
                false,
                true,
            ),
            placement(
                "HDMI-A-1",
                OutputPosition::Above("eDP-1".to_string()),
                false,
                true,
            ),
        ],
    };
    let connected = vec![
        connected("eDP-1", 1920, 1080),
        connected("HDMI-A-1", 1920, 720),
    ];

    let resolved = layout.resolve(&connected);

    assert_eq!(resolved[1].x, 0);
    assert_eq!(resolved[1].y, 360);
}

#[test]
fn dangling_reference_falls_back_to_auto() {
    let layout = OutputLayout {
        placements: vec![placement(
            "HDMI-A-1",
            OutputPosition::RightOf("UNKNOWN".to_string()),
            false,
            true,
        )],
    };
    let connected = vec![
        connected("eDP-1", 1920, 1080),
        connected("HDMI-A-1", 1920, 1080),
    ];

    let resolved = layout.resolve(&connected);

    assert_eq!(resolved[1].x, 1920);
    assert_eq!(resolved[1].y, 0);
}

#[test]
fn cycle_two_outputs_falls_back_to_auto_for_second() {
    let layout = OutputLayout {
        placements: vec![
            placement("A", OutputPosition::RightOf("B".to_string()), false, true),
            placement("B", OutputPosition::RightOf("A".to_string()), false, true),
        ],
    };
    let connected = vec![connected("A", 100, 100), connected("B", 100, 100)];

    let resolved = layout.resolve(&connected);

    assert_eq!(resolved[0].x, 0);
    assert_eq!(resolved[0].y, 0);
    assert_eq!(resolved[1].x, 100);
    assert_eq!(resolved[1].y, 0);
    assert_ne!(resolved[0].x, resolved[1].x);
}

#[test]
fn disabled_output_is_skipped_in_chain() {
    let layout = OutputLayout {
        placements: vec![placement("HDMI-A-1", OutputPosition::Auto, false, false)],
    };
    let connected = vec![
        connected("eDP-1", 1920, 1080),
        connected("HDMI-A-1", 1920, 1080),
        connected("DP-1", 1920, 1080),
    ];

    let resolved = layout.resolve(&connected);

    assert_eq!(resolved[0].x, 0);
    assert!(resolved[0].enabled);
    assert_eq!(resolved[1].x, 0);
    assert_eq!(resolved[1].y, 0);
    assert!(!resolved[1].enabled);
    assert_eq!(resolved[2].x, 1920);
    assert!(resolved[2].enabled);
}

#[test]
fn explicit_primary_overrides_first() {
    let layout = OutputLayout {
        placements: vec![placement("HDMI-A-1", OutputPosition::Auto, true, true)],
    };
    let connected = vec![
        connected("eDP-1", 1920, 1080),
        connected("HDMI-A-1", 1920, 1080),
    ];

    let resolved = layout.resolve(&connected);

    assert!(!resolved[0].primary);
    assert!(resolved[1].primary);
}

#[test]
fn multiple_primary_keeps_first_in_placements() {
    let layout = OutputLayout {
        placements: vec![
            placement("eDP-1", OutputPosition::Auto, true, true),
            placement("HDMI-A-1", OutputPosition::Auto, true, true),
        ],
    };
    let connected = vec![
        connected("eDP-1", 1920, 1080),
        connected("HDMI-A-1", 1920, 1080),
    ];

    let resolved = layout.resolve(&connected);

    assert!(resolved[0].primary);
    assert!(!resolved[1].primary);
}

#[test]
fn no_enabled_output_yields_no_primary() {
    let layout = OutputLayout {
        placements: vec![placement("eDP-1", OutputPosition::Auto, false, false)],
    };
    let connected = vec![connected("eDP-1", 1920, 1080)];

    let resolved = layout.resolve(&connected);

    assert!(!resolved[0].enabled);
    assert!(!resolved[0].primary);
}

#[test]
fn connected_order_preserved_in_result() {
    let layout = OutputLayout::default();
    let connected = vec![
        connected("c", 100, 100),
        connected("a", 100, 100),
        connected("b", 100, 100),
    ];

    let resolved = layout.resolve(&connected);

    assert_eq!(
        resolved.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(),
        vec!["c", "a", "b"]
    );
    assert_eq!(resolved[0].x, 0);
    assert_eq!(resolved[1].x, 100);
    assert_eq!(resolved[2].x, 200);
}

#[test]
fn placement_for_returns_existing_or_none() {
    let layout = OutputLayout {
        placements: vec![placement("eDP-1", OutputPosition::Auto, false, true)],
    };

    assert!(layout.placement_for("eDP-1").is_some());
    assert!(layout.placement_for("HDMI-A-1").is_none());
}

#[test]
fn from_config_entry_maps_auto_position() {
    let entry = config_entry("drm-0", OutputPositionConfig::Auto, false, true);

    let placement = OutputPlacement::from(&entry);

    assert_eq!(placement.position, OutputPosition::Auto);
}

#[test]
fn from_config_entry_maps_coord_position() {
    let entry = config_entry(
        "drm-0",
        OutputPositionConfig::Coord { x: 10, y: 20 },
        false,
        true,
    );

    let placement = OutputPlacement::from(&entry);

    assert_eq!(placement.position, OutputPosition::Coord { x: 10, y: 20 });
}

#[test]
fn from_config_entry_maps_each_relation() {
    let right = OutputPlacement::from(&config_entry(
        "a",
        OutputPositionConfig::RightOf("b".to_string()),
        false,
        true,
    ));
    let left = OutputPlacement::from(&config_entry(
        "a",
        OutputPositionConfig::LeftOf("b".to_string()),
        false,
        true,
    ));
    let below = OutputPlacement::from(&config_entry(
        "a",
        OutputPositionConfig::Below("b".to_string()),
        false,
        true,
    ));
    let above = OutputPlacement::from(&config_entry(
        "a",
        OutputPositionConfig::Above("b".to_string()),
        false,
        true,
    ));

    assert_eq!(right.position, OutputPosition::RightOf("b".to_string()));
    assert_eq!(left.position, OutputPosition::LeftOf("b".to_string()));
    assert_eq!(below.position, OutputPosition::Below("b".to_string()));
    assert_eq!(above.position, OutputPosition::Above("b".to_string()));
}

#[test]
fn from_config_entry_preserves_primary_and_enabled() {
    let entry = config_entry("drm-0", OutputPositionConfig::Auto, true, false);

    let placement = OutputPlacement::from(&entry);

    assert!(placement.primary);
    assert!(!placement.enabled);
}
