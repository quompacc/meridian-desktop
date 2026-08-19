use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Point, Size};

use crate::state::{
    HalfSnapRestoreGeometry, MaximizeRestoreGeometry, OutputGeometry,
    NORMAL_WINDOW_BOTTOM_RESERVED_PX,
};

use super::{
    anchored_client_location_from_pointer, apply_half_snap_drag_restore_states,
    apply_half_snap_tiled_states, consume_half_snap_restore_geometry,
    drag_restore_anchor_from_start_pointer, half_snap_restore_geometry_source,
    is_pointer_near_output_top_edge, move_release_workarea_geometry,
    movement_crosses_restore_threshold, release_edge_action_for_output, HalfSnapDirection,
    MoveReleaseEdgeAction,
};

fn point(x: f64, y: f64) -> Point<f64, Logical> {
    Point::from((x, y))
}

#[test]
fn top_edge_threshold_detects_near_top() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };

    assert!(is_pointer_near_output_top_edge(output, point(100.0, 6.0)));
}

#[test]
fn top_edge_threshold_rejects_deeper_positions() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };

    assert!(!is_pointer_near_output_top_edge(output, point(100.0, 25.0)));
}

#[test]
fn top_edge_threshold_requires_pointer_inside_output() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };

    assert!(!is_pointer_near_output_top_edge(output, point(-1.0, 4.0)));
}

#[test]
fn drag_restore_threshold_requires_real_drag_distance() {
    let start = point(100.0, 100.0);
    let below = point(105.0, 105.0);
    let beyond = point(110.0, 106.0);

    assert!(!movement_crosses_restore_threshold(start, below));
    assert!(movement_crosses_restore_threshold(start, beyond));
}

#[test]
fn anchored_restore_location_preserves_pointer_horizontal_ratio() {
    let pointer = point(960.0, 120.0);
    let pointer_frame_offset_y = 10.0;
    let frame_ratio_x = 0.5;
    let client_size: Size<i32, Logical> = (800, 600).into();
    let insets = (2, 34, 2, 2);

    let client_loc = anchored_client_location_from_pointer(
        pointer,
        pointer_frame_offset_y,
        frame_ratio_x,
        client_size,
        insets,
    );
    assert_eq!(client_loc.x, 560);
    assert_eq!(client_loc.y, 144);
}

#[test]
fn drag_restore_anchor_clamps_pointer_ratio_near_left_edge() {
    let drag_start_pointer = point(-20.0, 100.0);
    let maximized_client_loc: Point<i32, Logical> = (0, 32).into();
    let maximized_client_size: Size<i32, Logical> = (1920, 1048).into();
    let maximized_insets = (0, 32, 0, 0);

    let anchor = drag_restore_anchor_from_start_pointer(
        drag_start_pointer,
        maximized_client_loc,
        maximized_client_size,
        maximized_insets,
    );
    assert_eq!(anchor.pointer_frame_ratio_x, 0.0);
    assert_eq!(anchor.pointer_frame_offset_y, 100.0);
}

#[test]
fn drag_restore_anchor_clamps_pointer_ratio_near_right_edge() {
    let drag_start_pointer = point(2500.0, 100.0);
    let maximized_client_loc: Point<i32, Logical> = (0, 32).into();
    let maximized_client_size: Size<i32, Logical> = (1920, 1048).into();
    let maximized_insets = (0, 32, 0, 0);

    let anchor = drag_restore_anchor_from_start_pointer(
        drag_start_pointer,
        maximized_client_loc,
        maximized_client_size,
        maximized_insets,
    );
    assert_eq!(anchor.pointer_frame_ratio_x, 1.0);
    assert_eq!(anchor.pointer_frame_offset_y, 100.0);
}

#[test]
fn anchored_restore_location_applies_floating_insets_after_frame_anchor() {
    let pointer = point(960.0, 200.0);
    let anchor = drag_restore_anchor_from_start_pointer(
        point(960.0, 100.0),
        (0, 32).into(),
        (1920, 1048).into(),
        (0, 32, 0, 0),
    );

    let restored_client_loc = anchored_client_location_from_pointer(
        pointer,
        anchor.pointer_frame_offset_y,
        anchor.pointer_frame_ratio_x,
        (800, 600).into(),
        (2, 34, 2, 2),
    );

    assert_eq!(restored_client_loc, Point::from((560, 134)));
}

#[test]
fn left_edge_release_triggers_left_half_snap() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };

    let action = release_edge_action_for_output(output, point(2.0, 400.0));
    assert_eq!(
        action,
        Some(MoveReleaseEdgeAction::HalfSnap(HalfSnapDirection::Left))
    );
}

#[test]
fn right_edge_release_triggers_right_half_snap() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };

    let action = release_edge_action_for_output(output, point(1919.0, 400.0));
    assert_eq!(
        action,
        Some(MoveReleaseEdgeAction::HalfSnap(HalfSnapDirection::Right))
    );
}

#[test]
fn top_edge_maximize_precedes_side_snap() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };

    let action = release_edge_action_for_output(output, point(2.0, 2.0));
    assert_eq!(action, Some(MoveReleaseEdgeAction::Maximize));
}

#[test]
fn release_away_from_edges_does_not_snap() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };

    assert_eq!(
        release_edge_action_for_output(output, point(800.0, 400.0)),
        None
    );
}

#[test]
fn move_release_workarea_subtracts_panel_reservation() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let workarea = move_release_workarea_geometry(output);
    assert_eq!(workarea.x, 0);
    assert_eq!(workarea.y, 0);
    assert_eq!(workarea.width, 1920);
    assert_eq!(workarea.height, 1080 - NORMAL_WINDOW_BOTTOM_RESERVED_PX);
}

#[test]
fn half_snap_tiled_states_are_set_for_left_and_right() {
    let mut left = smithay::wayland::shell::xdg::ToplevelState::default();
    apply_half_snap_tiled_states(&mut left, HalfSnapDirection::Left);
    assert!(left.states.contains(xdg_toplevel::State::TiledLeft));
    assert!(!left.states.contains(xdg_toplevel::State::TiledRight));
    assert!(left.states.contains(xdg_toplevel::State::TiledTop));
    assert!(left.states.contains(xdg_toplevel::State::TiledBottom));

    let mut right = smithay::wayland::shell::xdg::ToplevelState::default();
    apply_half_snap_tiled_states(&mut right, HalfSnapDirection::Right);
    assert!(!right.states.contains(xdg_toplevel::State::TiledLeft));
    assert!(right.states.contains(xdg_toplevel::State::TiledRight));
    assert!(right.states.contains(xdg_toplevel::State::TiledTop));
    assert!(right.states.contains(xdg_toplevel::State::TiledBottom));
}

#[test]
fn half_snap_restore_prefers_maximize_restore_geometry() {
    let maximize_restore = Some(MaximizeRestoreGeometry::new(
        (40, 50).into(),
        Some((800, 600).into()),
    ));
    let fallback_current_loc: Option<Point<i32, Logical>> = Some((700, 200).into());
    let fallback_current_size: Size<i32, Logical> = (1200, 900).into();

    let selected = half_snap_restore_geometry_source(
        maximize_restore,
        fallback_current_loc,
        fallback_current_size,
    )
    .expect("restore geometry");

    assert_eq!(selected.client_loc, Point::from((40, 50)));
    assert_eq!(selected.client_size, Some(Size::from((800, 600))));
}

#[test]
fn consume_half_snap_restore_geometry_prefers_and_consumes_stored_entry() {
    let mut restore_map = std::collections::HashMap::new();
    restore_map.insert(
        "window-a".to_string(),
        HalfSnapRestoreGeometry::new((40, 50).into(), Some((900, 700).into())),
    );

    let selected = consume_half_snap_restore_geometry(
        &mut restore_map,
        "window-a",
        (10, 20).into(),
        (800, 600).into(),
    );

    assert_eq!(selected.client_loc, Point::from((40, 50)));
    assert_eq!(selected.client_size, Some(Size::from((900, 700))));
    assert!(!restore_map.contains_key("window-a"));
}

#[test]
fn consume_half_snap_restore_geometry_falls_back_to_current_geometry() {
    let mut restore_map = std::collections::HashMap::new();
    let selected = consume_half_snap_restore_geometry(
        &mut restore_map,
        "window-b",
        (120, 140).into(),
        (700, 500).into(),
    );

    assert_eq!(selected.client_loc, Point::from((120, 140)));
    assert_eq!(selected.client_size, Some(Size::from((700, 500))));
}

#[test]
fn half_snap_drag_restore_clears_tiled_bits_and_preserves_other_states() {
    let mut state = smithay::wayland::shell::xdg::ToplevelState::default();
    state.states.set(xdg_toplevel::State::Maximized);
    state.states.set(xdg_toplevel::State::TiledLeft);
    state.states.set(xdg_toplevel::State::TiledTop);
    state.states.set(xdg_toplevel::State::TiledBottom);
    state.states.set(xdg_toplevel::State::Activated);

    apply_half_snap_drag_restore_states(&mut state, (800, 600).into());

    assert!(!state.states.contains(xdg_toplevel::State::Maximized));
    assert!(!state.states.contains(xdg_toplevel::State::TiledLeft));
    assert!(!state.states.contains(xdg_toplevel::State::TiledRight));
    assert!(!state.states.contains(xdg_toplevel::State::TiledTop));
    assert!(!state.states.contains(xdg_toplevel::State::TiledBottom));
    assert!(state.states.contains(xdg_toplevel::State::Activated));
    assert_eq!(state.size, Some(Size::from((800, 600))));
}

#[test]
fn xwayland_snap_rect_for_maximize_uses_workarea_geometry() {
    let output = OutputGeometry {
        x: 10,
        y: 20,
        width: 1600,
        height: 900,
    };
    let rect = super::xwayland_snap_rect_for_action(output, MoveReleaseEdgeAction::Maximize);
    assert_eq!(rect.loc, Point::from((10, 20)));
    assert_eq!(rect.size, Size::from((1600, 900)));
}

#[test]
fn xwayland_snap_rect_for_half_snap_splits_width() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1601,
        height: 900,
    };
    let left = super::xwayland_snap_rect_for_action(
        output,
        MoveReleaseEdgeAction::HalfSnap(HalfSnapDirection::Left),
    );
    let right = super::xwayland_snap_rect_for_action(
        output,
        MoveReleaseEdgeAction::HalfSnap(HalfSnapDirection::Right),
    );
    assert_eq!(left.loc, Point::from((0, 0)));
    assert_eq!(left.size, Size::from((800, 900)));
    assert_eq!(right.loc, Point::from((800, 0)));
    assert_eq!(right.size, Size::from((801, 900)));
}
