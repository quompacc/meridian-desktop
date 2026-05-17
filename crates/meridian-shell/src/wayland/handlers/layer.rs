use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
};
use tracing::{debug, warn};
use wayland_client::{Connection, QueueHandle};

use crate::wayland::{MeridianShell, RepaintReason};
use crate::{CALENDAR_POPUP_HEIGHT, CALENDAR_POPUP_WIDTH, LAUNCHER_HEIGHT, LAUNCHER_WIDTH};

impl LayerShellHandler for MeridianShell {
    fn closed(&mut self, _conn: &Connection, qh: &QueueHandle<Self>, layer: &LayerSurface) {
        if self.panel == *layer {
            warn!("Panel layer surface closed by compositor; terminating shell");
            self.exit = true;
            return;
        }

        if self.launcher_layer == *layer {
            warn!("Launcher layer surface closed by compositor; recovering launcher state");
            self.launcher_state.open = false;
            self.launcher_configured = false;
            self.launcher_last_signature = None;
            self.launcher_dirty = false;
            self.draw_panel(qh, RepaintReason::LayerConfigure);
            return;
        }

        if self.calendar_layer == *layer {
            warn!("Calendar popup layer surface closed by compositor; recovering popup state");
            self.calendar_popup_open = false;
            self.calendar_configured = false;
            self.calendar_dirty = false;
            self.draw_panel(qh, RepaintReason::LayerConfigure);
            return;
        }

        warn!("Unknown layer surface closed by compositor");
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
                "panel configure: size={}x{}",
                configure.new_size.0,
                configure.new_size.1
            );
            self.panel_configured = true;
            if configure.new_size.0 > 0 {
                self.width = configure.new_size.0;
            } else if let Some(output_width) = self.panel_output_width_fallback() {
                self.width = output_width as u32;
                debug!(
                    "panel configure width fallback used: output_width={} (configure width was 0)",
                    output_width
                );
            }
            self.draw_panel(qh, RepaintReason::LayerConfigure);
        } else if self.launcher_layer == *layer {
            let requested_w = if configure.new_size.0 > 0 {
                configure.new_size.0
            } else {
                LAUNCHER_WIDTH
            };
            let requested_h = if configure.new_size.1 > 0 {
                configure.new_size.1
            } else {
                LAUNCHER_HEIGHT
            };
            let clamped_w = requested_w.min(LAUNCHER_WIDTH);
            let clamped_h = requested_h.min(LAUNCHER_HEIGHT);
            tracing::debug!(
                "launcher configure: requested={}x{} clamped={}x{} desired={}x{}",
                requested_w,
                requested_h,
                clamped_w,
                clamped_h,
                LAUNCHER_WIDTH,
                LAUNCHER_HEIGHT
            );
            self.launcher_layer
                .set_anchor(Anchor::BOTTOM | Anchor::LEFT);
            self.launcher_layer
                .set_margin(0, 0, crate::SHELL_POPUP_BOTTOM_MARGIN, 8);
            self.launcher_layer.set_exclusive_zone(0);
            self.launcher_layer
                .set_size(LAUNCHER_WIDTH, LAUNCHER_HEIGHT);
            self.launcher_configured = true;
            self.launcher_width = LAUNCHER_WIDTH;
            self.launcher_height = LAUNCHER_HEIGHT;
            if self.launcher_state.open {
                self.draw_launcher(qh, RepaintReason::LayerConfigure);
            }
        } else if self.calendar_layer == *layer {
            let requested_w = if configure.new_size.0 > 0 {
                configure.new_size.0
            } else {
                CALENDAR_POPUP_WIDTH
            };
            let requested_h = if configure.new_size.1 > 0 {
                configure.new_size.1
            } else {
                CALENDAR_POPUP_HEIGHT
            };
            let clamped_w = requested_w.min(CALENDAR_POPUP_WIDTH);
            let clamped_h = requested_h.min(CALENDAR_POPUP_HEIGHT);
            tracing::debug!(
                "calendar popup configure: requested={}x{} clamped={}x{} desired={}x{}",
                requested_w,
                requested_h,
                clamped_w,
                clamped_h,
                CALENDAR_POPUP_WIDTH,
                CALENDAR_POPUP_HEIGHT
            );
            self.calendar_layer
                .set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
            self.calendar_layer
                .set_margin(0, 12, crate::SHELL_POPUP_BOTTOM_MARGIN, 0);
            self.calendar_layer.set_exclusive_zone(0);
            self.calendar_layer
                .set_size(CALENDAR_POPUP_WIDTH, CALENDAR_POPUP_HEIGHT);
            self.calendar_configured = true;
            self.calendar_width = CALENDAR_POPUP_WIDTH;
            self.calendar_height = CALENDAR_POPUP_HEIGHT;
            if self.calendar_popup_open {
                self.draw_calendar_popup(qh, RepaintReason::LayerConfigure);
            }
        }
    }
}

impl MeridianShell {
    fn panel_output_width_fallback(&self) -> Option<i32> {
        self.output_state
            .outputs()
            .filter_map(|output| self.output_state.info(&output))
            .filter_map(|info| {
                info.logical_size
                    .map(|(w, _)| w)
                    .or_else(|| {
                        info.modes
                            .iter()
                            .find(|mode| mode.current)
                            .map(|mode| mode.dimensions.0)
                    })
                    .filter(|width| *width > 0)
            })
            .max()
    }
}
