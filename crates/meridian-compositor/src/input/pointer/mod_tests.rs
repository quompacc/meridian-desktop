use smithay::utils::{Logical, Point, Rectangle, Transform};

use crate::state::{OutputGeometry, OutputRegistration, OutputRegistry};

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

#[test]
fn absolute_point_selects_output_one() {
    let mut registry = OutputRegistry::new();
    registry.upsert(reg("left", 0, 0, 1920, 1080));
    registry.upsert(reg("right", 1920, 0, 2560, 1440));
    let (output, fallback) = super::select_output_from_registry_for_point(&registry, 100.0, 200.0);
    assert_eq!(output.map(|o| o.name.as_str()), Some("left"));
    assert!(!fallback);
}

#[test]
fn absolute_point_selects_output_two() {
    let mut registry = OutputRegistry::new();
    registry.upsert(reg("left", 0, 0, 1920, 1080));
    registry.upsert(reg("right", 1920, 0, 2560, 1440));
    let (output, fallback) = super::select_output_from_registry_for_point(&registry, 2300.0, 100.0);
    assert_eq!(output.map(|o| o.name.as_str()), Some("right"));
    assert!(!fallback);
}

#[test]
fn resize_hit_maps_to_expected_cursor_icons() {
    assert_eq!(
        super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::Resize(
            crate::decoration::DecorationResizeEdge::Left,
        ))),
        crate::backend::drm::DrmCursorIcon::EwResize
    );
    assert_eq!(
        super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::Resize(
            crate::decoration::DecorationResizeEdge::Right,
        ))),
        crate::backend::drm::DrmCursorIcon::EwResize
    );
    assert_eq!(
        super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::Resize(
            crate::decoration::DecorationResizeEdge::Top,
        ))),
        crate::backend::drm::DrmCursorIcon::NsResize
    );
    assert_eq!(
        super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::Resize(
            crate::decoration::DecorationResizeEdge::Bottom,
        ))),
        crate::backend::drm::DrmCursorIcon::NsResize
    );
    assert_eq!(
        super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::Resize(
            crate::decoration::DecorationResizeEdge::TopLeft,
        ))),
        crate::backend::drm::DrmCursorIcon::NwseResize
    );
    assert_eq!(
        super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::Resize(
            crate::decoration::DecorationResizeEdge::TopRight,
        ))),
        crate::backend::drm::DrmCursorIcon::NeswResize
    );
    assert_eq!(
        super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::Resize(
            crate::decoration::DecorationResizeEdge::BottomLeft,
        ))),
        crate::backend::drm::DrmCursorIcon::NeswResize
    );
    assert_eq!(
        super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::Resize(
            crate::decoration::DecorationResizeEdge::BottomRight,
        ))),
        crate::backend::drm::DrmCursorIcon::NwseResize
    );
}

#[test]
fn non_resize_hit_maps_to_default_cursor_icon() {
    assert_eq!(
        super::cursor_icon_for_decoration_hit(Some(crate::decoration::DecorationHit::TitleBar)),
        crate::backend::drm::DrmCursorIcon::Default
    );
    assert_eq!(
        super::cursor_icon_for_decoration_hit(None),
        crate::backend::drm::DrmCursorIcon::Default
    );
}

#[test]
fn absolute_point_outside_uses_primary_fallback() {
    let mut registry = OutputRegistry::new();
    registry.upsert(reg("primary", 0, 0, 1920, 1080));
    registry.upsert(reg("second", 1920, 0, 2560, 1440));
    let (output, fallback) =
        super::select_output_from_registry_for_point(&registry, -100.0, -100.0);
    assert_eq!(output.map(|o| o.name.as_str()), Some("primary"));
    assert!(fallback);
}

#[test]
fn focus_update_candidate_is_none_outside_outputs() {
    let mut registry = OutputRegistry::new();
    registry.upsert(reg("left", 0, 0, 1920, 1080));
    registry.upsert(reg("right", 1920, 0, 2560, 1440));
    assert!(super::output_id_at_point_for_focus(&registry, -10.0, 0.0).is_none());
}

#[test]
fn relative_clamp_keeps_point_inside_bounds() {
    let bounds = (0, 0, 1920, 1080);
    let point: Point<f64, Logical> = (2500.0, 1500.0).into();
    let (clamped, was_clamped) = super::clamp_point_to_desktop_bounds(point, bounds);
    assert!(was_clamped);
    assert_eq!(clamped.x, 1919.0);
    assert_eq!(clamped.y, 1079.0);
}

#[test]
fn relative_clamp_noop_when_inside_bounds() {
    let bounds = (0, 0, 1920, 1080);
    let point: Point<f64, Logical> = (1200.0, 800.0).into();
    let (clamped, was_clamped) = super::clamp_point_to_desktop_bounds(point, bounds);
    assert!(!was_clamped);
    assert_eq!(clamped, point);
}

#[test]
fn xwayland_edge_hit_detects_corners_and_edges() {
    let rect: Rectangle<i32, Logical> = Rectangle::new((100, 100).into(), (400, 300).into());

    assert_eq!(
        super::xwayland_resize_edge_from_rect(rect, (101.0, 101.0).into()),
        Some(crate::decoration::DecorationResizeEdge::TopLeft)
    );
    assert_eq!(
        super::xwayland_resize_edge_from_rect(rect, (498.0, 101.0).into()),
        Some(crate::decoration::DecorationResizeEdge::TopRight)
    );
    assert_eq!(
        super::xwayland_resize_edge_from_rect(rect, (101.0, 398.0).into()),
        Some(crate::decoration::DecorationResizeEdge::BottomLeft)
    );
    assert_eq!(
        super::xwayland_resize_edge_from_rect(rect, (498.0, 398.0).into()),
        Some(crate::decoration::DecorationResizeEdge::BottomRight)
    );
    assert_eq!(
        super::xwayland_resize_edge_from_rect(rect, (101.0, 250.0).into()),
        Some(crate::decoration::DecorationResizeEdge::Left)
    );
    assert_eq!(
        super::xwayland_resize_edge_from_rect(rect, (498.0, 250.0).into()),
        Some(crate::decoration::DecorationResizeEdge::Right)
    );
    assert_eq!(
        super::xwayland_resize_edge_from_rect(rect, (250.0, 101.0).into()),
        Some(crate::decoration::DecorationResizeEdge::Top)
    );
    assert_eq!(
        super::xwayland_resize_edge_from_rect(rect, (250.0, 398.0).into()),
        Some(crate::decoration::DecorationResizeEdge::Bottom)
    );
}

#[test]
fn xwayland_edge_hit_ignores_interior_and_outside_points() {
    let rect: Rectangle<i32, Logical> = Rectangle::new((50, 50).into(), (300, 200).into());

    assert_eq!(
        super::xwayland_resize_edge_from_rect(rect, (200.0, 150.0).into()),
        None
    );
    assert_eq!(
        super::xwayland_resize_edge_from_rect(rect, (49.0, 150.0).into()),
        None
    );
}
