use std::{os::unix::io::OwnedFd, process::Stdio};

use smithay::{
    desktop::Window,
    input::pointer::Focus,
    utils::SERIAL_COUNTER,
    utils::{Logical, Rectangle},
    wayland::{
        selection::SelectionTarget,
        xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    },
    xwayland::{
        xwm::{Reorder, ResizeEdge as X11ResizeEdge, WmWindowProperty, XwmId},
        X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler,
    },
};
use tracing::{debug, error, info, warn};

use crate::grabs::{
    move_grab::MoveSurfaceGrab,
    resize_grab::{ResizeEdge, ResizeSurfaceGrab},
};
use crate::state::{normal_window_workarea_from_output_geometry, window_list_entry, MeridianState};

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

fn find_active_x11_window(state: &MeridianState, surface: &X11Surface) -> Option<Window> {
    let active = state.workspaces.active;
    state
        .workspaces
        .space_at(active)
        .elements()
        .find(|window| matches!(window.x11_surface(), Some(x11) if x11 == surface))
        .cloned()
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

pub fn start_xwayland(state: &mut MeridianState) {
    let (xwayland, client) = match XWayland::spawn(
        &state.display_handle,
        None,
        std::iter::empty::<(String, String)>(),
        true,
        Stdio::null(),
        Stdio::null(),
        |_| (),
    ) {
        Ok(x) => x,
        Err(e) => {
            error!("Failed to spawn XWayland: {}", e);
            return;
        }
    };

    let display_handle = state.display_handle.clone();
    let handle = state.loop_handle.clone();

    if let Err(e) = state
        .loop_handle
        .insert_source(xwayland, move |event, _, state| match event {
            XWaylandEvent::Ready {
                x11_socket,
                display_number,
            } => {
                match X11Wm::start_wm(handle.clone(), &display_handle, x11_socket, client.clone()) {
                    Ok(wm) => {
                        // SAFETY: this updates process env once XWayland reports its assigned display number.
                        unsafe {
                            std::env::set_var("DISPLAY", format!(":{}", display_number));
                        }
                        info!("XWayland ready on DISPLAY=:{}", display_number);
                        state.xwm = Some(wm);
                    }
                    Err(e) => error!("Failed to start X11 WM: {}", e),
                }
            }
            XWaylandEvent::Error => warn!("XWayland crashed on startup"),
        })
    {
        error!("Failed to insert XWayland event source: {}", e);
    }
}

impl XWaylandShellHandler for MeridianState {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.xwayland_shell_state
    }
}

impl XwmHandler for MeridianState {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwm
            .as_mut()
            .expect("xwm_state called but X11Wm is not initialised")
    }

    fn new_window(&mut self, _xwm: XwmId, window: X11Surface) {
        debug!(
            event = "xwayland.new_window",
            window_id = window.window_id(),
            override_redirect = window.is_override_redirect(),
            geometry = ?window.geometry(),
            "xwayland window announced"
        );
    }

    fn new_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        debug!(
            event = "xwayland.new_override_redirect_window",
            window_id = window.window_id(),
            override_redirect = window.is_override_redirect(),
            geometry = ?window.geometry(),
            "xwayland override-redirect window announced"
        );
    }

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let window_id = window.window_id();
        let is_override_redirect = window.is_override_redirect();
        let initial_geometry = window.geometry();
        debug!(
            event = "xwayland.map_window_request.start",
            window_id,
            override_redirect = is_override_redirect,
            geometry = ?initial_geometry,
            "handling xwayland map window request"
        );
        if let Err(e) = window.set_mapped(true) {
            error!("map_window_request: set_mapped failed: {}", e);
            return;
        }
        let geo = window.geometry();
        // Place at a sensible default if the window hasn't reported a size yet.
        let loc = if geo.size.w > 0 && geo.size.h > 0 {
            geo.loc
        } else {
            (100, 100).into()
        };
        let requested_rect = Rectangle::new(loc, geo.size);
        let output_geometry = select_output_geometry_for_rect(self, requested_rect);
        let clamped_loc = output_geometry
            .map(|geometry| panel_safe_normal_xwayland_rect(requested_rect, geometry))
            .map(|rect| rect.loc)
            .unwrap_or(loc);
        debug!(
            event = "xwayland.map_window_request.geometry",
            window_id,
            requested_rect = ?requested_rect,
            output_geometry = ?output_geometry,
            clamp_applied = output_geometry.is_some(),
            final_loc = ?clamped_loc,
            map_path = "managed",
            "resolved xwayland managed map geometry"
        );
        let win = Window::new_x11_window(window);
        let active = self.workspaces.active;
        let opened = window_list_entry(&win);
        self.workspaces
            .space_at_mut(active)
            .map_element(win, clamped_loc, true);
        if let Some((id, title)) = opened {
            self.broadcast_window_opened(id, title);
        }
        self.mark_all_outputs_dirty("xwayland-map-window");
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let window_id = window.window_id();
        let is_override_redirect = window.is_override_redirect();
        let geo = window.geometry();
        let loc = if geo.size.w > 0 && geo.size.h > 0 {
            geo.loc
        } else {
            (0, 0).into()
        };
        let win = Window::new_x11_window(window);
        let active = self.workspaces.active;
        self.workspaces
            .space_at_mut(active)
            .map_element(win, loc, true);
        debug!(
            event = "xwayland.mapped_override_redirect_window",
            window_id,
            override_redirect = is_override_redirect,
            geometry = ?geo,
            final_loc = ?loc,
            map_path = "override_redirect",
            "mapped xwayland override-redirect window"
        );
        self.mark_all_outputs_dirty("xwayland-map-override");
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let window_id = window.window_id();
        let is_override_redirect = window.is_override_redirect();
        debug!(
            event = "xwayland.unmapped_window.start",
            window_id,
            override_redirect = is_override_redirect,
            geometry = ?window.geometry(),
            "handling xwayland unmap"
        );
        let active = self.workspaces.active;
        let maybe = self
            .workspaces
            .space_at(active)
            .elements()
            .find(|w| matches!(w.x11_surface(), Some(x) if x == &window))
            .cloned();
        if let Some(win) = maybe {
            if let Some((id, _)) = window_list_entry(&win) {
                self.broadcast_window_closed(id.clone());
                self.clear_window_runtime_state(&id);
                debug!(
                    event = "xwayland.unmapped_window.closed",
                    window_id,
                    published_id = id,
                    "broadcasted window closed for xwayland unmap"
                );
            }
            self.workspaces.space_at_mut(active).unmap_elem(&win);
            self.mark_all_outputs_dirty("xwayland-unmap-window");
        }
        if !is_override_redirect {
            let _ = window.set_mapped(false);
        }
        debug!(
            event = "xwayland.unmapped_window.done",
            window_id,
            override_redirect = is_override_redirect,
            mapped_flag_cleared = !is_override_redirect,
            "completed xwayland unmap handling"
        );
    }

    fn destroyed_window(&mut self, _xwm: XwmId, window: X11Surface) {
        debug!(
            event = "xwayland.destroyed_window",
            window_id = window.window_id(),
            override_redirect = window.is_override_redirect(),
            geometry = ?window.geometry(),
            "xwayland window destroyed"
        );
    }

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        x: Option<i32>,
        y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        reorder: Option<Reorder>,
    ) {
        let window_id = window.window_id();
        let is_override_redirect = window.is_override_redirect();
        let base_rect = window.geometry();
        let requested_rect = configure_request_rect(base_rect, x, y, w, h);
        let output_geometry = select_output_geometry_for_rect(self, requested_rect);
        let adjusted_rect =
            adjusted_configure_request_rect(requested_rect, output_geometry, is_override_redirect);
        let adjusted_geo = Rectangle::new(adjusted_rect.loc, adjusted_rect.size);
        let clamp_applied = !is_override_redirect && output_geometry.is_some();
        debug!(
            event = "xwayland.configure_request",
            window_id,
            override_redirect = is_override_redirect,
            base_rect = ?base_rect,
            requested_x = ?x,
            requested_y = ?y,
            requested_w = ?w,
            requested_h = ?h,
            requested_rect = ?requested_rect,
            adjusted_rect = ?adjusted_rect,
            output_geometry = ?output_geometry,
            reorder = ?reorder,
            clamp_applied,
            clamp_skipped_override_redirect = is_override_redirect,
            "handling xwayland configure request"
        );
        if let Err(e) = window.configure(adjusted_geo) {
            error!("configure_request: configure failed: {}", e);
        } else {
            self.mark_all_outputs_dirty("xwayland-configure-request");
        }
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        geometry: Rectangle<i32, Logical>,
        above: Option<u32>,
    ) {
        let window_id = window.window_id();
        let is_override_redirect = window.is_override_redirect();
        let active = self.workspaces.active;
        let maybe = self
            .workspaces
            .space_at(active)
            .elements()
            .find(|w| matches!(w.x11_surface(), Some(x) if x == &window))
            .cloned();
        if let Some(win) = maybe {
            let output_geometry = if is_override_redirect {
                None
            } else {
                select_output_geometry_for_rect(self, geometry)
            };
            let loc = if window.is_override_redirect() {
                geometry.loc
            } else {
                output_geometry
                    .map(|geometry_for_output| {
                        panel_safe_normal_xwayland_rect(geometry, geometry_for_output)
                    })
                    .map(|rect| rect.loc)
                    .unwrap_or(geometry.loc)
            };
            debug!(
                event = "xwayland.configure_notify",
                window_id,
                override_redirect = is_override_redirect,
                geometry = ?geometry,
                mapped_loc = ?loc,
                output_geometry = ?output_geometry,
                above = ?above,
                clamp_applied = !is_override_redirect && output_geometry.is_some(),
                "handling xwayland configure notify"
            );
            self.workspaces
                .space_at_mut(active)
                .map_element(win, loc, false);
            self.mark_all_outputs_dirty("xwayland-configure-notify");
        }
    }

    fn property_notify(&mut self, _xwm: XwmId, _window: X11Surface, property: WmWindowProperty) {
        if matches!(property, WmWindowProperty::Title | WmWindowProperty::Class) {
            self.broadcast_window_snapshot();
        }
    }

    fn resize_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        button: u32,
        edges: X11ResizeEdge,
    ) {
        let window_id = window.window_id();
        if window.is_override_redirect() {
            debug!(
                event = "xwayland.resize_request.ignored",
                window_id,
                override_redirect = true,
                button,
                edges = ?edges,
                reason = "override_redirect",
                "ignoring xwayland resize request"
            );
            return;
        }
        let Some(mapped_window) = find_active_x11_window(self, &window) else {
            debug!(
                event = "xwayland.resize_request.ignored",
                window_id,
                override_redirect = false,
                button,
                edges = ?edges,
                reason = "window_not_mapped",
                "ignoring xwayland resize request"
            );
            return;
        };
        let fullscreen_shaped = window_is_output_fullscreen_shape(self, &mapped_window);
        if fullscreen_shaped {
            debug!(
                event = "xwayland.resize_request.ignored",
                window_id,
                button,
                edges = ?edges,
                fullscreen_shaped,
                reason = "fullscreen_shaped",
                "ignoring xwayland resize request"
            );
            return;
        }
        debug!(
            event = "xwayland.resize_request.start",
            window_id,
            button,
            edges = ?edges,
            fullscreen_shaped,
            geometry = ?window.geometry(),
            "handling xwayland resize request"
        );
        let Some(pointer) = self.seat.get_pointer() else {
            tracing::debug!("ignoring xwayland resize request: seat has no pointer");
            return;
        };
        let Some(start_data) = pointer.grab_start_data() else {
            tracing::debug!(
                "ignoring xwayland resize request: pointer grab start data unavailable"
            );
            return;
        };
        let resize_edges = x11_resize_edge_to_resize_edge(edges);
        if resize_edges.is_empty() {
            tracing::debug!("ignoring xwayland resize request: empty resize edges");
            return;
        }
        let Some(initial_window_location) = self
            .workspaces
            .space_at(self.workspaces.active)
            .element_location(&mapped_window)
        else {
            tracing::debug!("ignoring xwayland resize request: window location unavailable");
            return;
        };
        let initial_window_size = mapped_window.geometry().size;
        debug!(
            event = "xwayland.resize_request.grab",
            window_id,
            initial_window_location = ?initial_window_location,
            initial_window_size = ?initial_window_size,
            "starting xwayland resize grab"
        );
        let grab = ResizeSurfaceGrab::start(
            start_data,
            mapped_window,
            resize_edges,
            Rectangle::new(initial_window_location, initial_window_size),
        );
        let serial = SERIAL_COUNTER.next_serial();
        pointer.set_grab(self, grab, serial, Focus::Clear);
    }

    fn move_request(&mut self, _xwm: XwmId, window: X11Surface, button: u32) {
        let window_id = window.window_id();
        if window.is_override_redirect() {
            debug!(
                event = "xwayland.move_request.ignored",
                window_id,
                override_redirect = true,
                button,
                reason = "override_redirect",
                "ignoring xwayland move request"
            );
            return;
        }
        let Some(mapped_window) = find_active_x11_window(self, &window) else {
            debug!(
                event = "xwayland.move_request.ignored",
                window_id,
                override_redirect = false,
                button,
                reason = "window_not_mapped",
                "ignoring xwayland move request"
            );
            return;
        };
        let fullscreen_shaped = window_is_output_fullscreen_shape(self, &mapped_window);
        if fullscreen_shaped {
            debug!(
                event = "xwayland.move_request.ignored",
                window_id,
                button,
                fullscreen_shaped,
                reason = "fullscreen_shaped",
                "ignoring xwayland move request"
            );
            return;
        }
        debug!(
            event = "xwayland.move_request.start",
            window_id,
            button,
            fullscreen_shaped,
            geometry = ?window.geometry(),
            "handling xwayland move request"
        );
        let Some(pointer) = self.seat.get_pointer() else {
            tracing::debug!("ignoring xwayland move request: seat has no pointer");
            return;
        };
        let Some(start_data) = pointer.grab_start_data() else {
            tracing::debug!("ignoring xwayland move request: pointer grab start data unavailable");
            return;
        };
        let Some(initial_window_location) = self
            .workspaces
            .space_at(self.workspaces.active)
            .element_location(&mapped_window)
        else {
            tracing::debug!("ignoring xwayland move request: window location unavailable");
            return;
        };
        debug!(
            event = "xwayland.move_request.grab",
            window_id,
            initial_window_location = ?initial_window_location,
            "starting xwayland move grab"
        );
        let grab = MoveSurfaceGrab {
            start_data,
            window: mapped_window,
            initial_window_location,
            latest_pointer_location: None,
            started_maximized: false,
            started_fullscreen: false,
            drag_restore_done: false,
        };
        let serial = SERIAL_COUNTER.next_serial();
        pointer.set_grab(self, grab, serial, Focus::Clear);
    }

    fn send_selection(
        &mut self,
        _xwm: XwmId,
        _selection: SelectionTarget,
        _mime_type: String,
        _fd: OwnedFd,
    ) {
    }

    fn disconnected(&mut self, _xwm: XwmId) {
        debug!(event = "xwayland.disconnected", "xwayland wm disconnected");
        self.xwm = None;
    }
}

#[cfg(test)]
mod tests {
    use smithay::utils::Rectangle;

    use crate::state::{OutputGeometry, NORMAL_WINDOW_BOTTOM_RESERVED_PX};

    use super::{
        adjusted_configure_request_rect, configure_request_rect, panel_safe_normal_xwayland_rect,
    };

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
        assert_eq!(adjusted.loc.y, 744);
        assert_eq!(adjusted.size.h, 300);
        assert_eq!(adjusted.loc.x, 100);
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
            864,
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
        assert_eq!(adjusted.loc.y, 844);
        assert_eq!(adjusted.size.h, 200);
    }
}
