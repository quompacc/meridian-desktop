use std::{os::unix::io::OwnedFd, process::Stdio};

use smithay::{
    desktop::Window,
    utils::{Logical, Rectangle},
    wayland::{
        selection::SelectionTarget,
        xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    },
    xwayland::{
        xwm::{Reorder, ResizeEdge as X11ResizeEdge, XwmId},
        X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler,
    },
};
use tracing::{error, info, warn};

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

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}
    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
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
        let clamped_loc = select_output_geometry_for_rect(self, requested_rect)
            .map(|output_geometry| panel_safe_normal_xwayland_rect(requested_rect, output_geometry))
            .map(|rect| rect.loc)
            .unwrap_or(loc);
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
        self.mark_all_outputs_dirty("xwayland-map-override");
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let active = self.workspaces.active;
        let maybe = self
            .workspaces
            .space_at(active)
            .elements()
            .find(|w| matches!(w.x11_surface(), Some(x) if x == &window))
            .cloned();
        if let Some(win) = maybe {
            if let Some((id, _)) = window_list_entry(&win) {
                self.broadcast_window_closed(id);
            }
            self.workspaces.space_at_mut(active).unmap_elem(&win);
            self.mark_all_outputs_dirty("xwayland-unmap-window");
        }
        if !window.is_override_redirect() {
            let _ = window.set_mapped(false);
        }
    }

    fn destroyed_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        _x: Option<i32>,
        _y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        let mut geo = window.geometry();
        if let Some(w) = w {
            geo.size.w = w.max(1) as i32;
        }
        if let Some(h) = h {
            geo.size.h = h.max(1) as i32;
        }
        let requested_rect = Rectangle::new(geo.loc, geo.size);
        let adjusted_rect = select_output_geometry_for_rect(self, requested_rect)
            .map(|output_geometry| panel_safe_normal_xwayland_rect(requested_rect, output_geometry))
            .unwrap_or(requested_rect);
        let adjusted_geo = Rectangle::new(adjusted_rect.loc, adjusted_rect.size);
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
        _above: Option<u32>,
    ) {
        let active = self.workspaces.active;
        let maybe = self
            .workspaces
            .space_at(active)
            .elements()
            .find(|w| matches!(w.x11_surface(), Some(x) if x == &window))
            .cloned();
        if let Some(win) = maybe {
            let loc = if window.is_override_redirect() {
                geometry.loc
            } else {
                select_output_geometry_for_rect(self, geometry)
                    .map(|output_geometry| {
                        panel_safe_normal_xwayland_rect(geometry, output_geometry)
                    })
                    .map(|rect| rect.loc)
                    .unwrap_or(geometry.loc)
            };
            self.workspaces
                .space_at_mut(active)
                .map_element(win, loc, false);
            self.mark_all_outputs_dirty("xwayland-configure-notify");
        }
    }

    fn resize_request(
        &mut self,
        _xwm: XwmId,
        _window: X11Surface,
        _button: u32,
        _edges: X11ResizeEdge,
    ) {
    }

    fn move_request(&mut self, _xwm: XwmId, _window: X11Surface, _button: u32) {}

    fn send_selection(
        &mut self,
        _xwm: XwmId,
        _selection: SelectionTarget,
        _mime_type: String,
        _fd: OwnedFd,
    ) {
    }

    fn disconnected(&mut self, _xwm: XwmId) {
        self.xwm = None;
    }
}

#[cfg(test)]
mod tests {
    use smithay::utils::Rectangle;

    use crate::state::{OutputGeometry, NORMAL_WINDOW_BOTTOM_RESERVED_PX};

    use super::panel_safe_normal_xwayland_rect;

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
}
