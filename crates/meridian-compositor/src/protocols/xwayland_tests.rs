use smithay::utils::{Rectangle, Size};

use crate::state::{OutputGeometry, NORMAL_WINDOW_BOTTOM_RESERVED_PX};

use super::{
    adjusted_configure_request_rect, apply_managed_map_ssd, apply_override_redirect_ssd,
    centered_normal_xwayland_rect_with_insets, classify_managed_configure_request,
    configure_request_rect, maximized_x11_content_size, panel_safe_normal_xwayland_rect,
    panel_safe_normal_xwayland_rect_with_insets, DecorationSyncTarget,
    ManagedConfigureRequestAction,
};

#[test]
fn normal_xwayland_window_centers_its_complete_frame() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let requested = Rectangle::new((4000, -200).into(), (800, 600).into());

    let centered = centered_normal_xwayland_rect_with_insets(requested, output, (2, 34, 2, 2));

    assert_eq!(centered.loc.x, 560);
    assert_eq!(
        centered.loc.y,
        (1080 - NORMAL_WINDOW_BOTTOM_RESERVED_PX - 636) / 2 + 34
    );
    assert_eq!(centered.size, Size::from((800, 600)));
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct MockDecorationSyncTarget {
    ssd: Option<bool>,
    focused: Option<bool>,
    maximized: Option<bool>,
    fullscreen: Option<bool>,
}

impl DecorationSyncTarget for MockDecorationSyncTarget {
    fn set_ssd(&mut self, ssd: bool) {
        self.ssd = Some(ssd);
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = Some(focused);
    }

    fn set_maximized(&mut self, maximized: bool) {
        self.maximized = Some(maximized);
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = Some(fullscreen);
    }
}

#[test]
fn normal_xwayland_rect_is_clamped_to_panel_safe_bottom() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let requested = Rectangle::new((100, 900).into(), (800, 300).into());
    let adjusted = panel_safe_normal_xwayland_rect(requested, output);
    assert_eq!(
        adjusted.loc.y,
        1080 - NORMAL_WINDOW_BOTTOM_RESERVED_PX - 300
    );
    assert_eq!(adjusted.size.h, 300);
    assert_eq!(adjusted.loc.x, 100);
}

#[test]
fn decorated_xwayland_rect_keeps_the_complete_frame_on_screen() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let requested = Rectangle::new((0, 0).into(), (800, 600).into());
    let adjusted = panel_safe_normal_xwayland_rect_with_insets(requested, output, (2, 34, 2, 2));

    assert_eq!(adjusted.loc.x, 2);
    assert_eq!(adjusted.loc.y, 34);
    assert_eq!(adjusted.loc.x - 2, output.x);
    assert_eq!(adjusted.loc.y - 34, output.y);
}

#[test]
fn output_sized_rect_is_treated_as_fullscreen_and_left_unchanged() {
    let output = OutputGeometry {
        x: 42,
        y: 7,
        width: 1600,
        height: 900,
    };
    let requested = Rectangle::new((42, 7).into(), (1600, 900).into());
    let adjusted = panel_safe_normal_xwayland_rect(requested, output);
    assert_eq!(adjusted, requested);
    assert_eq!(
        output.height - NORMAL_WINDOW_BOTTOM_RESERVED_PX,
        816,
        "sanity check: panel-safe height differs from fullscreen height"
    );
}

#[test]
fn configure_request_rect_uses_requested_x_y_when_present() {
    let base = Rectangle::new((100, 200).into(), (800, 600).into());
    let configured = configure_request_rect(base, Some(320), Some(480), None, None);
    assert_eq!(configured.loc.x, 320);
    assert_eq!(configured.loc.y, 480);
    assert_eq!(configured.size.w, 800);
    assert_eq!(configured.size.h, 600);
}

#[test]
fn override_redirect_configure_bypasses_panel_clamp() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let requested = Rectangle::new((500, 980).into(), (400, 200).into());
    let adjusted = adjusted_configure_request_rect(requested, Some(output), true);
    assert_eq!(adjusted, requested);
}

#[test]
fn managed_configure_still_clamps_to_panel_safe_workarea() {
    let output = OutputGeometry {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let requested = Rectangle::new((500, 980).into(), (400, 200).into());
    let adjusted = adjusted_configure_request_rect(requested, Some(output), false);
    assert_eq!(
        adjusted.loc.y,
        1080 - NORMAL_WINDOW_BOTTOM_RESERVED_PX - 200
    );
    assert_eq!(adjusted.size.h, 200);
}

#[test]
fn override_redirect_always_passes_through() {
    let r = Rectangle::new((0, 0).into(), (100, 100).into());
    let workarea = Rectangle::new((0, 0).into(), (1920, 1044).into());
    let output = Rectangle::new((0, 0).into(), (1920, 1080).into());
    for max in [false, true] {
        for fs in [false, true] {
            assert_eq!(
                classify_managed_configure_request(true, max, fs, r, workarea, output),
                ManagedConfigureRequestAction::PassThrough
            );
        }
    }
}

#[test]
fn managed_normal_window_passes_through() {
    let r = Rectangle::new((0, 0).into(), (100, 100).into());
    let workarea = Rectangle::new((0, 0).into(), (1920, 1044).into());
    let output = Rectangle::new((0, 0).into(), (1920, 1080).into());
    assert_eq!(
        classify_managed_configure_request(false, false, false, r, workarea, output),
        ManagedConfigureRequestAction::PassThrough
    );
}

#[test]
fn managed_maximized_request_matching_workarea_is_deny_noop() {
    let workarea = Rectangle::new((0, 0).into(), (1920, 1044).into());
    let output = Rectangle::new((0, 0).into(), (1920, 1080).into());
    assert_eq!(
        classify_managed_configure_request(false, true, false, workarea, workarea, output),
        ManagedConfigureRequestAction::DenyNoOp
    );
}

#[test]
fn managed_maximized_request_with_other_rect_is_implicit_unmaximize() {
    let workarea = Rectangle::new((0, 0).into(), (1920, 1044).into());
    let output = Rectangle::new((0, 0).into(), (1920, 1080).into());
    let other = Rectangle::new((100, 100).into(), (800, 600).into());
    assert_eq!(
        classify_managed_configure_request(false, true, false, other, workarea, output),
        ManagedConfigureRequestAction::ImplicitUnmaximize
    );
}

#[test]
fn managed_maximized_request_with_same_size_but_other_loc_is_implicit_unmaximize() {
    let workarea = Rectangle::new((0, 0).into(), (1920, 1044).into());
    let output = Rectangle::new((0, 0).into(), (1920, 1080).into());
    let same_size_shifted = Rectangle::new((100, 50).into(), (1920, 1044).into());
    assert_eq!(
        classify_managed_configure_request(false, true, false, same_size_shifted, workarea, output),
        ManagedConfigureRequestAction::ImplicitUnmaximize
    );
}

#[test]
fn managed_fullscreen_request_matching_output_is_deny_noop() {
    let workarea = Rectangle::new((0, 0).into(), (1920, 1044).into());
    let output = Rectangle::new((0, 0).into(), (1920, 1080).into());
    assert_eq!(
        classify_managed_configure_request(false, false, true, output, workarea, output),
        ManagedConfigureRequestAction::DenyNoOp
    );
}

#[test]
fn managed_fullscreen_request_with_other_rect_is_implicit_unfullscreen() {
    let workarea = Rectangle::new((0, 0).into(), (1920, 1044).into());
    let output = Rectangle::new((0, 0).into(), (1920, 1080).into());
    let other = Rectangle::new((100, 100).into(), (800, 600).into());
    assert_eq!(
        classify_managed_configure_request(false, false, true, other, workarea, output),
        ManagedConfigureRequestAction::ImplicitUnfullscreen
    );
}

#[test]
fn managed_fullscreen_takes_priority_over_maximized() {
    let workarea = Rectangle::new((0, 0).into(), (1920, 1044).into());
    let output = Rectangle::new((0, 0).into(), (1920, 1080).into());
    let other = Rectangle::new((100, 100).into(), (800, 600).into());
    assert_eq!(
        classify_managed_configure_request(false, true, true, other, workarea, output),
        ManagedConfigureRequestAction::ImplicitUnfullscreen
    );
}

#[test]
fn decoration_state_sync_after_map_managed() {
    let mut target = MockDecorationSyncTarget::default();
    apply_managed_map_ssd(&mut target, true, false);
    assert_eq!(target.ssd, Some(true));
    assert_eq!(target.focused, Some(false));
    assert_eq!(target.maximized, Some(true));
    assert_eq!(target.fullscreen, Some(false));
}

#[test]
fn decoration_state_sync_for_override_redirect_is_no_ssd() {
    let mut target = MockDecorationSyncTarget {
        ssd: Some(true),
        ..Default::default()
    };
    apply_override_redirect_ssd(&mut target);
    assert_eq!(target.ssd, Some(false));
    assert_eq!(target.focused, None);
    assert_eq!(target.maximized, None);
    assert_eq!(target.fullscreen, None);
}

#[test]
fn apply_managed_map_ssd_is_idempotent_when_called_twice() {
    let mut target = MockDecorationSyncTarget::default();
    apply_managed_map_ssd(&mut target, true, false);
    let after_once = target;
    apply_managed_map_ssd(&mut target, true, false);
    assert_eq!(target, after_once);
    assert_eq!(target.ssd, Some(true));
    assert_eq!(target.maximized, Some(true));
}

#[test]
fn apply_override_redirect_ssd_overrides_prior_managed_state() {
    let mut target = MockDecorationSyncTarget::default();
    apply_managed_map_ssd(&mut target, true, true);
    apply_override_redirect_ssd(&mut target);
    assert_eq!(target.ssd, Some(false));
}

#[test]
fn maximized_x11_content_size_subtracts_frame_insets_in_both_axes() {
    let content_size = maximized_x11_content_size(Size::from((1920, 1080)), (2, 34));
    assert_eq!(content_size, Size::from((1916, 1044)));
}

#[test]
fn maximized_x11_content_size_clamps_minimum_dimension_to_one() {
    let content_size = maximized_x11_content_size(Size::from((2, 30)), (2, 34));
    assert_eq!(content_size, Size::from((1, 1)));
}

#[test]
fn maximized_x11_content_size_with_zero_decoration_offset_equals_workarea() {
    let content_size = maximized_x11_content_size(Size::from((1920, 1080)), (0, 0));
    assert_eq!(content_size, Size::from((1920, 1080)));
}
