use smithay_client_toolkit::shell::wlr_layer::{
    LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
};
use tracing::warn;
use wayland_client::{Connection, QueueHandle};

use crate::wayland::{MeridianShell, RepaintReason};

impl LayerShellHandler for MeridianShell {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        warn!("Layer surface closed by compositor");
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if self.panel == *layer {
            tracing::info!(
                "Panel configure received: {}x{}",
                configure.new_size.0,
                configure.new_size.1
            );
            self.panel_configured = true;
            if configure.new_size.0 > 0 {
                self.width = configure.new_size.0;
            }
            self.draw_panel(qh, RepaintReason::LayerConfigure);
        } else if self.launcher_layer == *layer {
            tracing::info!(
                "Launcher configure received: {}x{}",
                configure.new_size.0,
                configure.new_size.1
            );
            self.launcher_configured = true;
            if configure.new_size.0 > 0 {
                self.launcher_width = configure.new_size.0;
            }
            if configure.new_size.1 > 0 {
                self.launcher_height = configure.new_size.1;
            }
            if self.launcher_state.open {
                self.draw_launcher(qh, RepaintReason::LayerConfigure);
            }
        }
    }
}
