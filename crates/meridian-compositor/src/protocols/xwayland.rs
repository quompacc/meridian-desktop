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

use crate::state::MeridianState;

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
        let win = Window::new_x11_window(window);
        let active = self.workspaces.active;
        self.workspaces
            .space_at_mut(active)
            .map_element(win, loc, true);
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
        if let Err(e) = window.configure(geo) {
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
            self.workspaces
                .space_at_mut(active)
                .map_element(win, geometry.loc, false);
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
