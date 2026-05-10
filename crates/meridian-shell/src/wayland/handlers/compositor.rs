use smithay_client_toolkit::compositor::CompositorHandler;
use smithay_client_toolkit::shell::WaylandSurface;
use wayland_client::{
    protocol::{wl_output, wl_surface},
    Connection, QueueHandle,
};

use crate::wayland::MeridianShell;

impl CompositorHandler for MeridianShell {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        if self.panel.wl_surface() == surface {
            self.panel_dirty = true;
            tracing::trace!(
                "panel frame callback received: dirty-flag set only (no immediate draw/commit)"
            );
        } else if self.launcher_state.open && self.launcher_layer.wl_surface() == surface {
            self.launcher_dirty = true;
            tracing::trace!(
                "launcher frame callback received: dirty-flag set only (no immediate draw/commit)"
            );
        }
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}
