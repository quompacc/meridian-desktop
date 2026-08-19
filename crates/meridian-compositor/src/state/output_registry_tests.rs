use smithay::utils::Transform;

use super::{
    OutputGeometry, OutputId, OutputInfo, OutputReconfigure, OutputRegistration, OutputRegistry,
};

fn reg(name: &str, x: i32, y: i32, width: i32, height: i32) -> OutputRegistration {
    OutputRegistration {
        name: name.to_string(),
        geometry: OutputGeometry {
            x,
            y,
            width,
            height,
        },
        scale: 1.0,
        transform: Transform::Normal,
        refresh_millihz: Some(60_000),
    }
}

fn reconfigure(x: i32, y: i32, width: i32, height: i32, scale: f64) -> OutputReconfigure {
    OutputReconfigure {
        geometry: OutputGeometry {
            x,
            y,
            width,
            height,
        },
        scale,
        transform: Transform::Flipped180,
        refresh_millihz: Some(75_000),
        primary: None,
    }
}

#[test]
fn register_and_list_outputs() {
    let mut registry = OutputRegistry::new();
    let id1 = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
    let id2 = registry.upsert(reg("HDMI-A-1", 1920, 0, 1920, 1080));
    assert_ne!(id1, id2);
    assert_eq!(registry.list().len(), 2);
}

#[test]
fn output_modes_are_stored_by_output_id() {
    let mut registry = OutputRegistry::new();
    let id = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
    assert!(registry.set_modes_by_id(
        id,
        vec![super::OutputModeInfo {
            width: 1920,
            height: 1080,
            refresh_millihz: Some(60_000),
            current: true,
            preferred: true,
        }],
    ));
    assert_eq!(registry.modes_for_id(id).len(), 1);
    assert!(registry.remove_by_id(id).is_some());
    assert!(registry.modes_for_id(id).is_empty());
}

#[test]
fn primary_and_first_fallback_work() {
    let mut registry = OutputRegistry::new();
    assert!(registry.primary().is_none());
    registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
    registry.upsert(reg("HDMI-A-1", 1920, 0, 1920, 1080));
    assert_eq!(
        registry.first().map(|info| info.name.as_str()),
        Some("eDP-1")
    );
    assert_eq!(
        registry.primary().map(|info| info.name.as_str()),
        Some("eDP-1")
    );
}

#[test]
fn lookup_by_id_works() {
    let mut registry = OutputRegistry::new();
    let id = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
    assert_eq!(
        registry.by_id(id).map(|info| info.name.as_str()),
        Some("eDP-1")
    );
    assert!(registry.by_id(OutputId(9999)).is_none());
}

#[test]
fn output_at_point_handles_two_horizontal_outputs() {
    let mut registry = OutputRegistry::new();
    registry.upsert(reg("left", 0, 0, 1920, 1080));
    registry.upsert(reg("right", 1920, 0, 2560, 1440));
    assert_eq!(
        registry
            .output_at_point(100.0, 100.0)
            .map(|info| info.name.as_str()),
        Some("left")
    );
    assert_eq!(
        registry
            .output_at_point(2200.0, 400.0)
            .map(|info| info.name.as_str()),
        Some("right")
    );
    assert!(registry.output_at_point(-10.0, 0.0).is_none());
}

#[test]
fn select_for_point_with_fallback_prefers_point_match() {
    let mut registry = OutputRegistry::new();
    registry.upsert(reg("left", 0, 0, 1920, 1080));
    registry.upsert(reg("right", 1920, 0, 2560, 1440));
    assert_eq!(
        registry
            .select_for_point_with_fallback(2200.0, 400.0)
            .map(|info| info.name.as_str()),
        Some("right")
    );
}

#[test]
fn select_for_point_with_fallback_uses_primary_before_first() {
    let infos = [
        OutputInfo {
            id: OutputId(1),
            name: "first".to_string(),
            geometry: OutputGeometry {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
            primary: false,
        },
        OutputInfo {
            id: OutputId(2),
            name: "primary".to_string(),
            geometry: OutputGeometry {
                x: 1920,
                y: 0,
                width: 1920,
                height: 1080,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
            primary: true,
        },
    ];
    let registry = OutputRegistry {
        next_id: 2,
        outputs: infos.into(),
        modes: Default::default(),
    };
    assert_eq!(
        registry
            .select_for_point_with_fallback(-100.0, -100.0)
            .map(|info| info.name.as_str()),
        Some("primary")
    );
}

#[test]
fn select_for_point_with_fallback_uses_first_when_no_primary_exists() {
    let infos = [
        OutputInfo {
            id: OutputId(1),
            name: "first".to_string(),
            geometry: OutputGeometry {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
            primary: false,
        },
        OutputInfo {
            id: OutputId(2),
            name: "second".to_string(),
            geometry: OutputGeometry {
                x: 1920,
                y: 0,
                width: 1920,
                height: 1080,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
            primary: false,
        },
    ];
    let registry = OutputRegistry {
        next_id: 2,
        outputs: infos.into(),
        modes: Default::default(),
    };
    assert_eq!(
        registry
            .select_for_point_with_fallback(-100.0, -100.0)
            .map(|info| info.name.as_str()),
        Some("first")
    );
}

#[test]
fn select_for_point_with_fallback_is_none_when_empty() {
    let registry = OutputRegistry::new();
    assert!(registry
        .select_for_point_with_fallback(100.0, 100.0)
        .is_none());
}

#[test]
fn empty_registry_is_safe() {
    let registry = OutputRegistry::new();
    assert!(registry.list().is_empty());
    assert!(registry.first().is_none());
    assert!(registry.primary().is_none());
    assert!(registry.by_id(OutputId(1)).is_none());
    assert!(registry.output_at_point(0.0, 0.0).is_none());
}

#[test]
fn remove_by_id_removes_output() {
    let mut registry = OutputRegistry::new();
    let id = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
    assert!(registry.contains_id(id));
    let removed = registry.remove_by_id(id);
    assert!(removed.is_some());
    assert!(!registry.contains_id(id));
    assert!(registry.list().is_empty());
}

#[test]
fn remove_unknown_output_is_safe_noop() {
    let mut registry = OutputRegistry::new();
    registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
    assert!(registry.remove_by_id(OutputId(9999)).is_none());
    assert!(registry.remove_by_name("UNKNOWN").is_none());
    assert_eq!(registry.list().len(), 1);
}

#[test]
fn reconfigure_keeps_output_id() {
    let mut registry = OutputRegistry::new();
    let id = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
    let changed = registry.reconfigure_by_name("eDP-1", reconfigure(10, 20, 1600, 900, 1.5));
    assert_eq!(changed, Some(id));
    assert_eq!(registry.by_id(id).map(|info| info.id), Some(id));
}

#[test]
fn reconfigure_updates_geometry_and_scale() {
    let mut registry = OutputRegistry::new();
    let id = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
    assert!(registry.reconfigure_by_id(id, reconfigure(42, 7, 1280, 720, 2.0)));
    let info = registry.by_id(id).expect("output must exist");
    assert_eq!(info.geometry.x, 42);
    assert_eq!(info.geometry.y, 7);
    assert_eq!(info.geometry.width, 1280);
    assert_eq!(info.geometry.height, 720);
    assert_eq!(info.scale, 2.0);
    assert_eq!(info.transform, Transform::Flipped180);
    assert_eq!(info.refresh_millihz, Some(75_000));
}

#[test]
fn primary_fallback_works_after_primary_remove() {
    let mut registry = OutputRegistry::new();
    let primary = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
    let second = registry.upsert(reg("HDMI-A-1", 1920, 0, 1920, 1080));
    assert_eq!(registry.primary().map(|info| info.id), Some(primary));
    registry.remove_by_id(primary);
    assert_eq!(registry.first().map(|info| info.id), Some(second));
    assert_eq!(registry.primary().map(|info| info.id), Some(second));
}

#[test]
fn output_id_is_not_reused_after_remove_and_add() {
    let mut registry = OutputRegistry::new();
    let first = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
    registry.remove_by_id(first);
    let second = registry.upsert(reg("HDMI-A-1", 1920, 0, 1920, 1080));
    assert!(second.0 > first.0);
}

#[test]
fn remove_primary_promotes_first_remaining_to_primary() {
    let mut registry = OutputRegistry::new();
    let first = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
    registry.upsert(reg("HDMI-A-1", 1920, 0, 1920, 1080));
    registry.upsert(reg("DP-1", 3840, 0, 1920, 1080));

    let _ = registry.remove_by_id(first);

    let first_remaining = registry.list().first().expect("remaining output");
    assert_eq!(first_remaining.name, "HDMI-A-1");
    assert!(first_remaining.primary);
}

#[test]
fn remove_non_primary_does_not_change_primary_flag() {
    let mut registry = OutputRegistry::new();
    let primary = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
    let secondary = registry.upsert(reg("HDMI-A-1", 1920, 0, 1920, 1080));

    let _ = registry.remove_by_id(secondary);

    let primary_info = registry.by_id(primary).expect("primary output");
    assert!(primary_info.primary);
}

#[test]
fn remove_only_output_leaves_empty_registry_without_panic() {
    let mut registry = OutputRegistry::new();
    let only = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));

    let _ = registry.remove_by_id(only);

    assert!(registry.list().is_empty());
    assert!(registry.primary().is_none());
}

#[test]
fn remove_by_name_promotes_first_remaining_to_primary() {
    let mut registry = OutputRegistry::new();
    registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
    registry.upsert(reg("HDMI-A-1", 1920, 0, 1920, 1080));

    let removed = registry.remove_by_name("eDP-1");
    assert!(removed.is_some());

    let remaining = registry.by_name("HDMI-A-1").expect("remaining output");
    assert!(remaining.primary);
}

#[test]
fn remove_when_no_primary_was_set_still_promotes_first() {
    let mut registry = OutputRegistry::new();
    let first = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
    let second = registry.upsert(reg("HDMI-A-1", 1920, 0, 1920, 1080));

    assert!(registry.reconfigure_by_id(
        first,
        OutputReconfigure {
            geometry: OutputGeometry {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
            primary: Some(false),
        }
    ));
    assert!(registry.reconfigure_by_id(
        second,
        OutputReconfigure {
            geometry: OutputGeometry {
                x: 1920,
                y: 0,
                width: 1920,
                height: 1080,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
            primary: Some(false),
        }
    ));

    let _ = registry.remove_by_id(first);

    let remaining = registry.by_id(second).expect("remaining output");
    assert!(remaining.primary);
}
