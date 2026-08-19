fn select_output_geometry_for_rect(
    state: &MeridianState,
    rect: Rectangle<i32, Logical>,
) -> Option<crate::state::OutputGeometry> {
    let center_x = rect.loc.x as f64 + (rect.size.w.max(1) as f64 * 0.5);
    let center_y = rect.loc.y as f64 + (rect.size.h.max(1) as f64 * 0.5);
    state
        .output_registry
        .select_for_point_with_fallback(center_x, center_y)
        .map(|info| info.geometry)
}

fn rect_matches_output_fullscreen_shape(
    rect: Rectangle<i32, Logical>,
    output_geometry: crate::state::OutputGeometry,
) -> bool {
    rect.loc.x == output_geometry.x
        && rect.loc.y == output_geometry.y
        && rect.size.w == output_geometry.width
        && rect.size.h == output_geometry.height
}

fn panel_safe_normal_xwayland_rect(
    rect: Rectangle<i32, Logical>,
    output_geometry: crate::state::OutputGeometry,
) -> Rectangle<i32, Logical> {
    if rect_matches_output_fullscreen_shape(rect, output_geometry) {
        return rect;
    }

    let workarea = normal_window_workarea_from_output_geometry(output_geometry);
    let mut adjusted = rect;
    adjusted.size.h = adjusted.size.h.min(workarea.height.max(1));

    let workarea_top = workarea.y;
    let workarea_bottom = workarea.y.saturating_add(workarea.height);
    let mut y = adjusted.loc.y;
    let bottom = y.saturating_add(adjusted.size.h);
    if bottom > workarea_bottom {
        y = workarea_bottom.saturating_sub(adjusted.size.h);
    }
    if y < workarea_top {
        y = workarea_top;
    }
    adjusted.loc.y = y;
    adjusted
}

fn configure_request_rect(
    base: Rectangle<i32, Logical>,
    x: Option<i32>,
    y: Option<i32>,
    w: Option<u32>,
    h: Option<u32>,
) -> Rectangle<i32, Logical> {
    let mut rect = base;
    if let Some(x) = x {
        rect.loc.x = x;
    }
    if let Some(y) = y {
        rect.loc.y = y;
    }
    if let Some(w) = w {
        rect.size.w = w.max(1) as i32;
    }
    if let Some(h) = h {
        rect.size.h = h.max(1) as i32;
    }
    rect
}

fn adjusted_configure_request_rect(
    requested_rect: Rectangle<i32, Logical>,
    output_geometry: Option<crate::state::OutputGeometry>,
    is_override_redirect: bool,
) -> Rectangle<i32, Logical> {
    if is_override_redirect {
        return requested_rect;
    }
    output_geometry
        .map(|geometry| panel_safe_normal_xwayland_rect(requested_rect, geometry))
        .unwrap_or(requested_rect)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagedConfigureRequestAction {
    PassThrough,
    DenyNoOp,
    ImplicitUnmaximize,
    ImplicitUnfullscreen,
}

pub(crate) fn classify_managed_configure_request(
    is_override_redirect: bool,
    is_maximized: bool,
    is_fullscreen: bool,
    requested_rect: Rectangle<i32, Logical>,
    workarea_rect: Rectangle<i32, Logical>,
    output_rect: Rectangle<i32, Logical>,
) -> ManagedConfigureRequestAction {
    if is_override_redirect {
        return ManagedConfigureRequestAction::PassThrough;
    }
    if is_fullscreen {
        if requested_rect == output_rect {
            return ManagedConfigureRequestAction::DenyNoOp;
        }
        return ManagedConfigureRequestAction::ImplicitUnfullscreen;
    }
    if is_maximized {
        if requested_rect == workarea_rect {
            return ManagedConfigureRequestAction::DenyNoOp;
        }
        return ManagedConfigureRequestAction::ImplicitUnmaximize;
    }
    ManagedConfigureRequestAction::PassThrough
}

fn find_active_x11_window(state: &MeridianState, surface: &X11Surface) -> Option<Window> {
    let active = state.workspaces.active;
    state
        .workspaces
        .space_at(active)
        .elements()
        .find(|window| matches!(window.x11_surface(), Some(x11) if x11 == surface))
        .cloned()
}

fn find_x11_window_with_workspace(
    state: &MeridianState,
    surface: &X11Surface,
) -> Option<(usize, Window)> {
    state
        .workspaces
        .find_element_workspace(
            |window| matches!(window.x11_surface(), Some(x11) if x11 == surface),
        )
        .map(|(workspace, window)| (workspace, window.clone()))
}

fn find_x11_surface_by_window_id(state: &MeridianState, window_id: u32) -> Option<X11Surface> {
    (0..state.workspaces.count()).find_map(|workspace| {
        state
            .workspaces
            .space_at(workspace)
            .elements()
            .find_map(|window| {
                let x11 = window.x11_surface()?;
                (x11.window_id() == window_id).then(|| x11.clone())
            })
    })
}

fn find_x11_window_by_stacking_id(
    state: &MeridianState,
    workspace: usize,
    stacking_id: u32,
) -> Option<Window> {
    state
        .workspaces
        .space_at(workspace)
        .elements()
        .find(|window| {
            window.x11_surface().is_some_and(|x11| {
                x11.window_id() == stacking_id
                    || x11
                        .mapped_window_id()
                        .is_some_and(|mapped_id| mapped_id == stacking_id)
            })
        })
        .cloned()
}

fn restack_override_redirect_above_hint(
    state: &mut MeridianState,
    workspace: usize,
    window: &Window,
    above: u32,
    source: &'static str,
) -> bool {
    let maybe_target =
        find_x11_window_by_stacking_id(state, workspace, above).filter(|target| target != window);
    if let Some(target) = maybe_target {
        state
            .workspaces
            .space_at_mut(workspace)
            .raise_element_above(window, &target, false);
        debug!(
            event = "xwayland.override_redirect.restack_above",
            source,
            above,
            matched = true,
            fallback_topmost = false,
            "restacked override-redirect window above referenced sibling"
        );
    } else {
        state
            .workspaces
            .space_at_mut(workspace)
            .raise_element(window, false);
        debug!(
            event = "xwayland.override_redirect.restack_above",
            source,
            above,
            matched = false,
            fallback_topmost = true,
            "unable to match override-redirect above sibling, raised to topmost"
        );
    }
    true
}

fn reorder_above_hint(reorder: Option<Reorder>) -> Option<u32> {
    match reorder {
        Some(Reorder::Above(above)) => Some(above),
        _ => None,
    }
}

fn update_or_diag_entry<F>(state: &mut MeridianState, window: &X11Surface, updater: F)
where
    F: FnOnce(&mut XwaylandOrDiagEntry),
{
    if let Some(entry) = state.xwayland_or_diag.get_mut(&window.window_id()) {
        updater(entry);
    }
}

fn window_is_output_fullscreen_shape(state: &MeridianState, window: &Window) -> bool {
    let active = state.workspaces.active;
    let Some(loc) = state.workspaces.space_at(active).element_location(window) else {
        return false;
    };
    let rect = Rectangle::new(loc, window.geometry().size);
    select_output_geometry_for_rect(state, rect)
        .is_some_and(|output_geometry| rect_matches_output_fullscreen_shape(rect, output_geometry))
}

fn x11_resize_edge_to_resize_edge(edges: X11ResizeEdge) -> ResizeEdge {
    match edges {
        X11ResizeEdge::Top => ResizeEdge::TOP,
        X11ResizeEdge::Bottom => ResizeEdge::BOTTOM,
        X11ResizeEdge::Left => ResizeEdge::LEFT,
        X11ResizeEdge::Right => ResizeEdge::RIGHT,
        X11ResizeEdge::TopLeft => ResizeEdge::TOP_LEFT,
        X11ResizeEdge::TopRight => ResizeEdge::TOP_RIGHT,
        X11ResizeEdge::BottomLeft => ResizeEdge::BOTTOM_LEFT,
        X11ResizeEdge::BottomRight => ResizeEdge::BOTTOM_RIGHT,
    }
}

pub(crate) fn x11_window_key(window: &X11Surface) -> String {
    crate::state::x11_window_id_key(window.window_id())
}

fn x11_fullscreen_restore_key(window: &X11Surface) -> String {
    format!("x11-fullscreen:{}", window.window_id())
}

fn maximized_x11_content_size(
    workarea_size: Size<i32, Logical>,
    decoration_offset: (i32, i32),
) -> Size<i32, Logical> {
    let width = workarea_size
        .w
        .saturating_sub(decoration_offset.0.saturating_mul(2))
        .max(1);
    let height = workarea_size
        .h
        .saturating_sub(decoration_offset.1.saturating_add(decoration_offset.0))
        .max(1);
    (width, height).into()
}
