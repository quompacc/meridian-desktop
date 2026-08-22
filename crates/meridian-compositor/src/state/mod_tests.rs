use smithay::utils::{Logical, Point, Rectangle, Size};

use super::{
    clear_tiled_toplevel_states, half_snap_client_placement_from_output,
    maximized_client_loc_from_output, maximized_client_rect_from_frame,
    normal_window_workarea_from_output_geometry, normal_window_workarea_from_rect,
    remember_maximize_restore_geometry, resolve_unmaximize_restore_client_loc,
    restore_client_loc_or_fallback, HalfSnapDirection, MaximizeRestoreGeometry, OutputGeometry,
    NORMAL_WINDOW_BOTTOM_RESERVED_PX,
};

#[test]
fn maximized_client_rect_subtracts_all_frame_insets() {
    let frame = Rectangle::new((10, 20).into(), (1600, 850).into());
    let client = maximized_client_rect_from_frame(frame, (0, 32, 0, 0));
    assert_eq!(client.loc, (10, 52).into());
    assert_eq!(client.size, (1600, 818).into());
}

#[test]
fn capture_known_loc_and_size_preserves_client_size() {
    let mut map = std::collections::HashMap::new();
    let loc: Point<i32, Logical> = (10, 20).into();
    let size: Size<i32, Logical> = (800, 600).into();
    let geometry = MaximizeRestoreGeometry::new(loc, Some(size));
    remember_maximize_restore_geometry(&mut map, "window-a".to_string(), geometry);

    let stored = map.get("window-a").expect("stored geometry");
    assert_eq!(stored.client_loc, loc);
    assert_eq!(stored.client_size, Some(size));
}

#[test]
fn premaximized_entry_allows_missing_client_size() {
    let mut map = std::collections::HashMap::new();
    let loc: Point<i32, Logical> = (2, 34).into();
    let geometry = MaximizeRestoreGeometry::new(loc, None);
    remember_maximize_restore_geometry(&mut map, "window-b".to_string(), geometry);

    let stored = map.get("window-b").expect("stored geometry");
    assert_eq!(stored.client_loc, loc);
    assert_eq!(stored.client_size, None);
}

#[test]
fn existing_entry_is_not_overwritten() {
    let mut map = std::collections::HashMap::new();
    let first = MaximizeRestoreGeometry::new((10, 20).into(), Some((800, 600).into()));
    let second = MaximizeRestoreGeometry::new((30, 40).into(), Some((1024, 768).into()));
    remember_maximize_restore_geometry(&mut map, "window-c".to_string(), first);
    remember_maximize_restore_geometry(&mut map, "window-c".to_string(), second);

    assert_eq!(map.get("window-c"), Some(&first));
}

#[test]
fn missing_restore_entry_uses_fallback_location() {
    let fallback: Point<i32, Logical> = (12, 34).into();
    let resolved = restore_client_loc_or_fallback(None, fallback);
    assert_eq!(resolved, fallback);
}

#[test]
fn maximize_mapping_adds_decoration_offset_to_output_origin() {
    let output_loc: Point<i32, Logical> = (100, 200).into();
    let mapped = maximized_client_loc_from_output(output_loc, (2, 34));
    assert_eq!(mapped, Point::from((102, 234)));
}

#[test]
fn normal_window_workarea_subtracts_bottom_panel_reservation() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let workarea = normal_window_workarea_from_output_geometry(output);
    assert_eq!(workarea.x, output.x);
    assert_eq!(workarea.y, output.y);
    assert_eq!(workarea.width, output.width);
    assert_eq!(
        workarea.height,
        output.height - NORMAL_WINDOW_BOTTOM_RESERVED_PX
    );
}

#[test]
fn normal_window_workarea_rect_preserves_origin_and_width() {
    let rect = Rectangle::new((50, 20).into(), (1600, 900).into());
    let workarea = normal_window_workarea_from_rect(rect);
    assert_eq!(workarea.loc, rect.loc);
    assert_eq!(workarea.size.w, rect.size.w);
    assert_eq!(
        workarea.size.h,
        rect.size.h - NORMAL_WINDOW_BOTTOM_RESERVED_PX
    );
}

#[test]
fn unmaximize_restore_uses_stored_geometry_without_fallback() {
    let geometry = Some(MaximizeRestoreGeometry::new(
        (40, 50).into(),
        Some((800, 600).into()),
    ));
    let (resolved, used_fallback) = resolve_unmaximize_restore_client_loc(geometry, (2, 34));
    assert_eq!(resolved, Point::from((40, 50)));
    assert!(!used_fallback);
}

#[test]
fn unmaximize_restore_uses_decoration_offset_when_missing() {
    let (resolved, used_fallback) = resolve_unmaximize_restore_client_loc(None, (2, 34));
    assert_eq!(resolved, Point::from((2, 34)));
    assert!(used_fallback);
}

#[test]
fn clear_tiled_toplevel_states_unsets_only_tiled_bits() {
    use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;

    let mut state = smithay::wayland::shell::xdg::ToplevelState::default();
    state.states.set(xdg_toplevel::State::TiledLeft);
    state.states.set(xdg_toplevel::State::TiledRight);
    state.states.set(xdg_toplevel::State::TiledTop);
    state.states.set(xdg_toplevel::State::TiledBottom);
    state.states.set(xdg_toplevel::State::Maximized);

    clear_tiled_toplevel_states(&mut state);

    assert!(!state.states.contains(xdg_toplevel::State::TiledLeft));
    assert!(!state.states.contains(xdg_toplevel::State::TiledRight));
    assert!(!state.states.contains(xdg_toplevel::State::TiledTop));
    assert!(!state.states.contains(xdg_toplevel::State::TiledBottom));
    assert!(state.states.contains(xdg_toplevel::State::Maximized));
}

#[test]
fn half_snap_left_placement_uses_left_output_half() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let placement = half_snap_client_placement_from_output(
        output,
        HalfSnapDirection::Left,
        (0, 0),
        (0, 0, 0, 0),
    );
    assert_eq!(placement.client_loc, Point::from((0, 0)));
    assert_eq!(placement.client_size, Size::from((960, 1080)));
}

#[test]
fn half_snap_right_placement_uses_right_output_half() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let placement = half_snap_client_placement_from_output(
        output,
        HalfSnapDirection::Right,
        (0, 0),
        (0, 0, 0, 0),
    );
    assert_eq!(placement.client_loc, Point::from((960, 0)));
    assert_eq!(placement.client_size, Size::from((960, 1080)));
}

#[test]
fn half_snap_nonzero_output_origin_is_preserved() {
    let output = OutputGeometry {
        x: 100,
        y: 50,
        width: 1600,
        height: 900,
    };
    let placement = half_snap_client_placement_from_output(
        output,
        HalfSnapDirection::Left,
        (0, 0),
        (0, 0, 0, 0),
    );
    assert_eq!(placement.client_loc, Point::from((100, 50)));
    assert_eq!(placement.client_size, Size::from((800, 900)));
}

#[test]
fn half_snap_placement_applies_ssd_offset_and_inset() {
    let output = OutputGeometry {
        x: 100,
        y: 50,
        width: 1600,
        height: 900,
    };
    let left = half_snap_client_placement_from_output(
        output,
        HalfSnapDirection::Left,
        (2, 34),
        (2, 34, 2, 2),
    );
    assert_eq!(left.client_loc, Point::from((102, 84)));
    assert_eq!(left.client_size, Size::from((796, 864)));

    let right = half_snap_client_placement_from_output(
        output,
        HalfSnapDirection::Right,
        (2, 34),
        (2, 34, 2, 2),
    );
    assert_eq!(right.client_loc, Point::from((902, 84)));
    assert_eq!(right.client_size, Size::from((796, 864)));
}

#[test]
fn half_snap_odd_output_width_assigns_extra_pixel_to_right_half() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1919,
        height: 1080,
    };
    let left = half_snap_client_placement_from_output(
        output,
        HalfSnapDirection::Left,
        (0, 0),
        (0, 0, 0, 0),
    );
    let right = half_snap_client_placement_from_output(
        output,
        HalfSnapDirection::Right,
        (0, 0),
        (0, 0, 0, 0),
    );

    assert_eq!(left.client_size.w, output.width / 2);
    assert_eq!(right.client_size.w, output.width - (output.width / 2));
    assert_eq!(left.client_size.w, 959);
    assert_eq!(right.client_size.w, 960);
    assert_eq!(right.client_size.w, left.client_size.w + 1);
}
