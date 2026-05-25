use smithay_client_toolkit::shell::{
    wlr_layer::{Anchor, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    WaylandSurface,
};
use tracing::{debug, warn};
use wayland_client::{Connection, QueueHandle};

use crate::wayland::{MeridianShell, RepaintReason};
use crate::{
    AUDIO_POPUP_HEIGHT, AUDIO_POPUP_RIGHT_MARGIN, AUDIO_POPUP_WIDTH, CALENDAR_POPUP_HEIGHT,
    CALENDAR_POPUP_WIDTH, LAUNCHER_HEIGHT, LAUNCHER_WIDTH, NETWORK_POPUP_HEIGHT,
    NETWORK_POPUP_RIGHT_MARGIN, NETWORK_POPUP_WIDTH, SNI_MENU_RIGHT_MARGIN, WORKSPACE_POPUP_HEIGHT,
    WORKSPACE_POPUP_WIDTH,
};

impl LayerShellHandler for MeridianShell {
    fn closed(&mut self, _conn: &Connection, qh: &QueueHandle<Self>, layer: &LayerSurface) {
        if self.panel == *layer {
            warn!("Panel layer surface closed by compositor; terminating shell");
            self.exit = true;
            return;
        }

        if self.desktop_layer == *layer {
            warn!("Desktop background layer surface closed by compositor; disabling desktop menu input");
            self.desktop_configured = false;
            self.desktop_context_menu = None;
            self.desktop_menu_open = false;
            return;
        }

        if self.desktop_menu_layer == *layer {
            warn!("Desktop menu layer surface closed by compositor; recovering menu state");
            self.desktop_menu_configured = false;
            self.desktop_context_menu = None;
            self.desktop_menu_open = false;
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
            self.audio_popup_open = false;
            self.status_notifier_menu_open = false;
            self.status_notifier_menu = None;
            self.status_notifier_menu_entries.clear();
            self.network_configured = false;
            self.network_dirty = false;
            self.audio_dirty = false;
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

        if self.thumbnail_layer == *layer {
            warn!("Thumbnail popup layer surface closed by compositor; recovering popup state");
            self.thumbnail_popup_open = false;
            self.thumbnail_configured = false;
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
        if self.desktop_layer == *layer {
            let width = if configure.new_size.0 > 0 {
                configure.new_size.0
            } else {
                self.panel_output_width_fallback().unwrap_or(1) as u32
            };
            let height = if configure.new_size.1 > 0 {
                configure.new_size.1
            } else {
                self.output_height_fallback()
                    .unwrap_or(crate::PANEL_HEIGHT as i32 + 1) as u32
            };
            tracing::debug!("desktop configure: size={}x{}", width, height);
            self.desktop_configured = true;
            self.desktop_width = width.max(1);
            self.desktop_height = height.max(1);
            self.desktop_layer.wl_surface().attach(None, 0, 0);
            self.desktop_layer.commit();
        } else if self.desktop_menu_layer == *layer {
            let desired_h =
                crate::context_menu::menu_height(crate::context_menu::desktop_item_list().len())
                    as u32;
            let width = if configure.new_size.0 > 0 {
                configure.new_size.0
            } else {
                crate::context_menu::MENU_WIDTH as u32
            };
            let height = if configure.new_size.1 > 0 {
                configure.new_size.1
            } else {
                desired_h
            };
            tracing::debug!("desktop menu configure: size={}x{}", width, height);
            self.desktop_menu_configured = true;
            self.desktop_menu_width = width.max(1);
            self.desktop_menu_height = height.max(1);
            if self.desktop_menu_open {
                self.draw_desktop_menu(qh, RepaintReason::LayerConfigure);
            }
        } else if self.panel == *layer {
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
            if self.launcher_is_fullscreen {
                // Full-screen mode: accept compositor-provided size as-is.
                let w = configure.new_size.0.max(1);
                let h = configure.new_size.1.max(1);
                self.launcher_configured = true;
                self.launcher_width = w;
                self.launcher_height = h;
                self.launcher_visual_x = 8;
                self.launcher_visual_y = h as i32
                    - crate::LAUNCHER_HEIGHT as i32
                    - crate::PANEL_HEIGHT as i32
                    - crate::SHELL_POPUP_BOTTOM_MARGIN;
                tracing::debug!(
                    "launcher fullscreen: {}x{} visual@({},{})",
                    w,
                    h,
                    self.launcher_visual_x,
                    self.launcher_visual_y
                );
            } else {
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
            }
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
            if self.status_notifier_menu_open {
                tracing::debug!(
                    "status-notifier menu configure: requested={}x{} desired={}x{}",
                    configure.new_size.0,
                    configure.new_size.1,
                    self.status_notifier_menu_width,
                    self.status_notifier_menu_height
                );
                self.network_layer
                    .set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
                self.network_layer.set_margin(
                    0,
                    SNI_MENU_RIGHT_MARGIN,
                    crate::SHELL_POPUP_BOTTOM_MARGIN,
                    0,
                );
                self.network_layer.set_exclusive_zone(0);
                self.network_layer.set_size(
                    self.status_notifier_menu_width,
                    self.status_notifier_menu_height,
                );
                self.network_configured = true;
                self.draw_status_notifier_menu(qh, RepaintReason::LayerConfigure);
                return;
            }
            if self.audio_popup_open {
                let requested_w = if configure.new_size.0 > 0 {
                    configure.new_size.0
                } else {
                    AUDIO_POPUP_WIDTH
                };
                let requested_h = if configure.new_size.1 > 0 {
                    configure.new_size.1
                } else {
                    AUDIO_POPUP_HEIGHT
                };
                tracing::debug!(
                    "audio popup configure: requested={}x{} desired={}x{}",
                    requested_w,
                    requested_h,
                    AUDIO_POPUP_WIDTH,
                    AUDIO_POPUP_HEIGHT
                );
                self.network_layer
                    .set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
                self.network_layer.set_margin(
                    0,
                    AUDIO_POPUP_RIGHT_MARGIN,
                    crate::SHELL_POPUP_BOTTOM_MARGIN,
                    0,
                );
                self.network_layer.set_exclusive_zone(0);
                self.network_layer
                    .set_size(AUDIO_POPUP_WIDTH, AUDIO_POPUP_HEIGHT);
                self.network_configured = true;
                self.audio_width = AUDIO_POPUP_WIDTH;
                self.audio_height = AUDIO_POPUP_HEIGHT;
                self.draw_audio_popup(qh, RepaintReason::LayerConfigure);
                return;
            }
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
        } else if self.thumbnail_layer == *layer {
            // Adopt the compositor's chosen size as our truth — drawing with a
            // different (stale) width yields a clipped or stretched popup.
            // Anchor/margin/set_size are still set by open_thumbnail_popup or
            // refresh_thumbnail_popup; we only take the resulting configured
            // dimensions here.
            if configure.new_size.0 > 0 {
                self.thumbnail_width = configure.new_size.0;
            }
            if configure.new_size.1 > 0 {
                self.thumbnail_height = configure.new_size.1;
            }
            self.thumbnail_configured = true;
            if self.thumbnail_popup_open {
                self.draw_thumbnail_popup(qh, RepaintReason::LayerConfigure);
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

    fn output_height_fallback(&self) -> Option<i32> {
        self.output_state
            .outputs()
            .filter_map(|output| self.output_state.info(&output))
            .filter_map(|info| {
                info.logical_size
                    .map(|(_, h)| h)
                    .or_else(|| {
                        info.modes
                            .iter()
                            .find(|mode| mode.current)
                            .map(|mode| mode.dimensions.1)
                    })
                    .filter(|height| *height > 0)
            })
            .max()
    }
}
