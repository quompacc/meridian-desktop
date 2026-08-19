use smithay::utils::{Logical, Point, Transform};

use crate::state::{OutputGeometry, OutputId, OutputInfo};

#[test]
fn click_point_on_output_one() {
    let infos = vec![
        OutputInfo {
            id: OutputId(1),
            name: "left".to_string(),
            geometry: OutputGeometry {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
            primary: true,
        },
        OutputInfo {
            id: OutputId(2),
            name: "right".to_string(),
            geometry: OutputGeometry {
                x: 1920,
                y: 0,
                width: 2560,
                height: 1440,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
            primary: false,
        },
    ];
    let point: Point<f64, Logical> = (100.0, 200.0).into();
    let (selected, reason) = super::select_pointer_button_output_info(&infos, Some(point));
    assert_eq!(selected.map(|info| info.name.as_str()), Some("left"));
    assert_eq!(reason, "point-match");
}

#[test]
fn click_point_on_output_two() {
    let infos = vec![
        OutputInfo {
            id: OutputId(1),
            name: "left".to_string(),
            geometry: OutputGeometry {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
            primary: true,
        },
        OutputInfo {
            id: OutputId(2),
            name: "right".to_string(),
            geometry: OutputGeometry {
                x: 1920,
                y: 0,
                width: 2560,
                height: 1440,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
            primary: false,
        },
    ];
    let point: Point<f64, Logical> = (2200.0, 400.0).into();
    let (selected, reason) = super::select_pointer_button_output_info(&infos, Some(point));
    assert_eq!(selected.map(|info| info.name.as_str()), Some("right"));
    assert_eq!(reason, "point-match");
}

#[test]
fn outside_point_uses_primary_fallback() {
    let infos = vec![OutputInfo {
        id: OutputId(11),
        name: "primary".to_string(),
        geometry: OutputGeometry {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
        scale: 1.0,
        transform: Transform::Normal,
        refresh_millihz: Some(60_000),
        primary: true,
    }];
    let point: Point<f64, Logical> = (-50.0, -50.0).into();
    let (selected, reason) = super::select_pointer_button_output_info(&infos, Some(point));
    assert_eq!(selected.map(|info| info.name.as_str()), Some("primary"));
    assert_eq!(reason, "fallback-primary");
}

#[test]
fn no_primary_uses_first_fallback() {
    let infos = vec![
        OutputInfo {
            id: OutputId(21),
            name: "first".to_string(),
            geometry: OutputGeometry {
                x: 0,
                y: 0,
                width: 1280,
                height: 720,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
            primary: false,
        },
        OutputInfo {
            id: OutputId(22),
            name: "second".to_string(),
            geometry: OutputGeometry {
                x: 1280,
                y: 0,
                width: 1280,
                height: 720,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
            primary: false,
        },
    ];
    let point: Point<f64, Logical> = (-500.0, -10.0).into();
    let (selected, reason) = super::select_pointer_button_output_info(&infos, Some(point));
    assert_eq!(selected.map(|info| info.name.as_str()), Some("first"));
    assert_eq!(reason, "fallback-first");
}

#[test]
fn empty_registry_is_safe() {
    let (selected, reason) = super::select_pointer_button_output_info(&[], None);
    assert!(selected.is_none());
    assert_eq!(reason, "empty-registry");
}
