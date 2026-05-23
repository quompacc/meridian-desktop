use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
};
use tracing::{debug, warn};
use wayland_client::{Connection, QueueHandle};

use crate::wayland::{MeridianShell, RepaintReason};
use crate::{
    CALENDAR_POPUP_HEIGHT, CALENDAR_POPUP_WIDTH, LAUNCHER_HEIGHT, LAUNCHER_WIDTH,
    NETWORK_POPUP_HEIGHT, NETWORK_POPUP_RIGHT_MARGIN, NETWORK_POPUP_WIDTH, WORKSPACE_POPUP_HEIGHT,
    WORKSPACE_POPUP_WIDTH,
};

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

        if self.workspace_layer == *layer {
            warn!("Workspace popup layer surface closed by compositor; recovering popup state");
            self.workspace_popup_open = false;
            self.workspace_configured = false;
            self.workspace_dirty = false;
            self.draw_panel(qh, RepaintReason::LayerConfigure);
            return;
        }

        if self.network_layer == *layer {
            warn!("Network popup layer surface closed by compositor; recovering popup state");
            self.network_popup_open = false;
            self.network_configured = false;
            self.network_dirty = false;
            self.draw_panel(qh, RepaintReason::LayerConfigure);
            return;
        }

        if self.notification_layer == *layer {
            warn!("Notification layer surface closed by compositor; clearing queue");
            self.notifications.clear();
            self.notification_configured = false;
            self.notification_dirty = false;
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
        } else if self.workspace_layer == *layer {
            let requested_w = if configure.new_size.0 > 0 {
                configure.new_size.0
            } else {
                WORKSPACE_POPUP_WIDTH
            };
            let requested_h = if configure.new_size.1 > 0 {
                configure.new_size.1
            } else {
                WORKSPACE_POPUP_HEIGHT
            };
            let clamped_w = requested_w.min(WORKSPACE_POPUP_WIDTH);
            let clamped_h = requested_h.min(WORKSPACE_POPUP_HEIGHT);
            tracing::debug!(
                "workspace popup configure: requested={}x{} clamped={}x{} desired={}x{}",
                requested_w,
                requested_h,
                clamped_w,
                clamped_h,
                WORKSPACE_POPUP_WIDTH,
                WORKSPACE_POPUP_HEIGHT
            );
            self.workspace_layer
                .set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
            self.workspace_layer
                .set_margin(0, 160, crate::SHELL_POPUP_BOTTOM_MARGIN, 0);
            self.workspace_layer.set_exclusive_zone(0);
            self.workspace_layer
                .set_size(WORKSPACE_POPUP_WIDTH, WORKSPACE_POPUP_HEIGHT);
            self.workspace_configured = true;
            self.workspace_width = WORKSPACE_POPUP_WIDTH;
            self.workspace_height = WORKSPACE_POPUP_HEIGHT;
            if self.workspace_popup_open {
                self.draw_workspace_popup(qh, RepaintReason::LayerConfigure);
            }
        } else if self.network_layer == *layer {
            let requested_w = if configure.new_size.0 > 0 {
                configure.new_size.0
            } else {
                NETWORK_POPUP_WIDTH
            };
            let requested_h = if configure.new_size.1 > 0 {
                configure.new_size.1
            } else {
                NETWORK_POPUP_HEIGHT
            };
            let clamped_w = requested_w.min(NETWORK_POPUP_WIDTH);
            let clamped_h = requested_h.min(NETWORK_POPUP_HEIGHT);
            tracing::debug!(
                "network popup configure: requested={}x{} clamped={}x{} desired={}x{}",
                requested_w,
                requested_h,
                clamped_w,
                clamped_h,
                NETWORK_POPUP_WIDTH,
                NETWORK_POPUP_HEIGHT
            );
            self.network_layer
                .set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
            self.network_layer.set_margin(
                0,
                NETWORK_POPUP_RIGHT_MARGIN,
                crate::SHELL_POPUP_BOTTOM_MARGIN,
                0,
            );
            self.network_layer.set_exclusive_zone(0);
            self.network_layer
                .set_size(NETWORK_POPUP_WIDTH, NETWORK_POPUP_HEIGHT);
            self.network_configured = true;
            self.network_width = NETWORK_POPUP_WIDTH;
            self.network_height = NETWORK_POPUP_HEIGHT;
            if self.network_popup_open {
                self.draw_network_popup(qh, RepaintReason::LayerConfigure);
            }
        } else if self.notification_layer == *layer {
            tracing::debug!(
                "notification configure: requested={}x{} desired={}x{}",
                configure.new_size.0,
                configure.new_size.1,
                crate::NOTIFICATION_WIDTH,
                crate::NOTIFICATION_HEIGHT
            );
            self.notification_layer
                .set_anchor(Anchor::TOP | Anchor::RIGHT);
            self.notification_layer.set_margin(
                crate::NOTIFICATION_TOP_MARGIN,
                crate::NOTIFICATION_RIGHT_MARGIN,
                0,
                0,
            );
            self.notification_layer.set_exclusive_zone(0);
            self.notification_layer
                .set_size(crate::NOTIFICATION_WIDTH, crate::NOTIFICATION_HEIGHT);
            self.notification_configured = true;
            self.notification_width = crate::NOTIFICATION_WIDTH;
            self.notification_height = crate::NOTIFICATION_HEIGHT;
            if !self.notifications.is_empty() {
                self.draw_notification_popup(qh, RepaintReason::LayerConfigure);
            } else {
                self.unmap_notification_popup(crate::wayland::CommitReason::UnknownOther);
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
