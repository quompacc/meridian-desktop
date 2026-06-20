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

        if self.consent_layer == *layer {
            warn!("Screenshot consent layer surface closed by compositor; clearing modal state");
            self.consent_configured = false;
            self.consent_open = false;
            self.consent_request_id = None;
            self.consent_app_id.clear();
            self.consent_hover = None;
            return;
        }

        if self.wifi_modal_layer == *layer {
            warn!("Wi-Fi password modal layer surface closed by compositor; clearing modal state");
            self.wifi_modal_configured = false;
            self.wifi_modal_open = false;
            self.wifi_password_prompt = None;
            self.wifi_password_input.clear();
            self.wifi_modal_hover = None;
            return;
        }

        if self.region_picker_layer == *layer {
            warn!("Screenshot region picker layer surface closed by compositor; clearing picker state");
            self.region_picker_configured = false;
            self.region_picker_open = false;
            self.region_picker_request_id = None;
            self.region_picker_app_id.clear();
            self.region_picker_local = false;
            self.region_picker_drag_start = None;
            self.region_picker_drag_current = None;
            self.region_picker_pending = None;
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
            // Re-assert the menu's current desired geometry. desktop_menu_width /
            // desktop_menu_height are maintained by open_desktop_context_menu and
            // the pointer handler (they widen when the settings flyout opens).
            // Never recompute from the item list here — that would ignore the
            // expanded width and cause the same protocol error we fixed before.
            let desired_w = self
                .desktop_menu_width
                .max(crate::context_menu::MENU_WIDTH as u32);
            let desired_h = self.desktop_menu_height.max(1);
            self.desktop_menu_layer
                .set_anchor(Anchor::TOP | Anchor::LEFT);
            if let Some(ref menu) = self.desktop_context_menu {
                let mx = (menu.x.max(0) - crate::POPUP_SHADOW_PAD).max(0);
                let my = (menu.y.max(0) - crate::POPUP_SHADOW_PAD).max(0);
                self.desktop_menu_layer.set_margin(my, 0, 0, mx);
            }
            self.desktop_menu_layer.set_size(desired_w, desired_h);
            tracing::debug!(
                "desktop menu configure: forcing {}x{} (compositor suggested {}x{})",
                desired_w,
                desired_h,
                configure.new_size.0,
                configure.new_size.1
            );
            self.desktop_menu_configured = true;
            self.desktop_menu_width = desired_w;
            self.desktop_menu_height = desired_h;
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
                // Fullscreen mode: reassert all-edge anchors and zero size on
                // each configure so reopen cycles keep a valid stretched layer.
                self.launcher_layer
                    .set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
                self.launcher_layer.set_margin(0, 0, 0, 0);
                self.launcher_layer.set_exclusive_zone(0);
                self.launcher_layer.set_size(0, 0);
                let w = configure.new_size.0.max(1);
                let h = configure.new_size.1.max(1);
                self.launcher_configured = true;
                self.launcher_width = w;
                self.launcher_height = h;
                self.launcher_visual_x = crate::PANEL_SIDE_MARGIN as i32;
                self.launcher_visual_y =
                    h as i32 - crate::LAUNCHER_HEIGHT as i32 - crate::SHELL_POPUP_BOTTOM_MARGIN;
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
            let surface_w = crate::popup_surface_w(CALENDAR_POPUP_WIDTH);
            let surface_h = crate::popup_surface_h(CALENDAR_POPUP_HEIGHT);
            tracing::debug!(
                "calendar popup configure: requested={}x{} surface={}x{}",
                configure.new_size.0,
                configure.new_size.1,
                surface_w,
                surface_h
            );
            self.calendar_layer
                .set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
            self.calendar_layer
                .set_margin(0, 12, crate::SHELL_POPUP_BOTTOM_MARGIN, 0);
            self.calendar_layer.set_exclusive_zone(0);
            self.calendar_layer.set_size(surface_w, surface_h);
            self.calendar_configured = true;
            self.calendar_width = surface_w;
            self.calendar_height = surface_h;
            if self.calendar_popup_open {
                self.draw_calendar_popup(qh, RepaintReason::LayerConfigure);
            }
        } else if self.workspace_layer == *layer {
            let surface_w = crate::popup_surface_w(WORKSPACE_POPUP_WIDTH);
            let surface_h = crate::popup_surface_h(WORKSPACE_POPUP_HEIGHT);
            tracing::debug!(
                "workspace popup configure: requested={}x{} surface={}x{}",
                configure.new_size.0,
                configure.new_size.1,
                surface_w,
                surface_h
            );
            self.workspace_layer
                .set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
            self.workspace_layer
                .set_margin(0, 160, crate::SHELL_POPUP_BOTTOM_MARGIN, 0);
            self.workspace_layer.set_exclusive_zone(0);
            self.workspace_layer.set_size(surface_w, surface_h);
            self.workspace_configured = true;
            self.workspace_width = surface_w;
            self.workspace_height = surface_h;
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
                    crate::popup_surface_w(self.status_notifier_menu_width),
                    crate::popup_surface_h(self.status_notifier_menu_height),
                );
                self.network_configured = true;
                self.draw_status_notifier_menu(qh, RepaintReason::LayerConfigure);
                return;
            }
            if self.volume_osd_open {
                self.network_layer.set_anchor(Anchor::BOTTOM);
                self.network_layer
                    .set_margin(0, 0, crate::VOLUME_OSD_BOTTOM_MARGIN, 0);
                self.network_layer.set_exclusive_zone(0);
                self.network_layer.set_size(
                    crate::popup_surface_w(crate::VOLUME_OSD_WIDTH),
                    crate::popup_surface_h(crate::VOLUME_OSD_HEIGHT),
                );
                self.network_configured = true;
                self.volume_osd_width = crate::popup_surface_w(crate::VOLUME_OSD_WIDTH);
                self.volume_osd_height = crate::popup_surface_h(crate::VOLUME_OSD_HEIGHT);
                self.draw_volume_osd(qh, RepaintReason::LayerConfigure);
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
                self.network_layer.set_size(
                    crate::popup_surface_w(AUDIO_POPUP_WIDTH),
                    crate::popup_surface_h(AUDIO_POPUP_HEIGHT),
                );
                self.network_configured = true;
                self.audio_width = crate::popup_surface_w(AUDIO_POPUP_WIDTH);
                self.audio_height = crate::popup_surface_h(AUDIO_POPUP_HEIGHT);
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
            self.network_layer.set_size(
                crate::popup_surface_w(NETWORK_POPUP_WIDTH),
                crate::popup_surface_h(NETWORK_POPUP_HEIGHT),
            );
            self.network_configured = true;
            self.network_width = crate::popup_surface_w(NETWORK_POPUP_WIDTH);
            self.network_height = crate::popup_surface_h(NETWORK_POPUP_HEIGHT);
            if self.network_popup_open {
                self.draw_network_popup(qh, RepaintReason::LayerConfigure);
            }
        } else if self.notification_layer == *layer {
            let surface_w = crate::popup_surface_w(crate::NOTIFICATION_WIDTH);
            let surface_h = crate::popup_surface_h(crate::NOTIFICATION_HEIGHT);
            tracing::debug!(
                "notification configure: requested={}x{} surface={}x{}",
                configure.new_size.0,
                configure.new_size.1,
                surface_w,
                surface_h
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
            self.notification_layer.set_size(surface_w, surface_h);
            self.notification_configured = true;
            self.notification_width = surface_w;
            self.notification_height = surface_h;
            if !self.notifications.is_empty() {
                self.draw_notification_popup(qh, RepaintReason::LayerConfigure);
            } else {
                self.unmap_notification_popup(crate::wayland::CommitReason::UnknownOther);
            }
        } else if self.thumbnail_layer == *layer {
            // Thumbnail popup has dynamic width (set by open_thumbnail_popup /
            // refresh_thumbnail_popup); adopt whatever surface size the
            // compositor sent back. self.thumbnail_width / _height already
            // include the shadow pad.
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
        } else if self.consent_layer == *layer {
            let desired_w = crate::screenshot_consent::MODAL_WIDTH as u32;
            let desired_h = crate::screenshot_consent::MODAL_HEIGHT as u32;
            tracing::debug!(
                "consent configure: requested={}x{} desired={}x{}",
                configure.new_size.0,
                configure.new_size.1,
                desired_w,
                desired_h
            );
            // Re-assert the modal's intended size: a layer surface with anchor=None
            // is centered by the compositor, but it MUST have nonzero width AND
            // height on every commit, otherwise wlr-layer-shell error 1 fires.
            // sctk loses our init-time set_size after the first unmap, so we have
            // to re-set it on every configure cycle (mirrors the desktop_menu path).
            self.consent_layer.set_size(desired_w, desired_h);
            self.consent_configured = true;
            if self.consent_open {
                self.draw_consent_modal(qh, RepaintReason::LayerConfigure);
            }
        } else if self.wifi_modal_layer == *layer {
            // Same centering contract as the consent modal: anchor=None, so the
            // compositor centers it, but it must carry a nonzero size on every
            // commit. Re-assert on each configure (sctk drops it after unmap).
            let desired_w = crate::wifi_password_modal::MODAL_WIDTH as u32;
            let desired_h = crate::wifi_password_modal::MODAL_HEIGHT as u32;
            self.wifi_modal_layer.set_size(desired_w, desired_h);
            self.wifi_modal_configured = true;
            if self.wifi_modal_open {
                self.draw_wifi_modal(qh, RepaintReason::LayerConfigure);
            }
        } else if self.region_picker_layer == *layer {
            // Anchored to all four edges; size is dictated by the compositor.
            // Re-assert the anchor + (zero) size on every configure cycle:
            // sctk drops the pending state after unmap, so without this the
            // second open + commit fires wlr-layer-shell error 1 (width 0
            // without left + right anchors).
            use smithay_client_toolkit::shell::wlr_layer::Anchor;
            self.region_picker_layer
                .set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
            self.region_picker_layer.set_exclusive_zone(-1);
            self.region_picker_layer.set_size(0, 0);
            let w = configure.new_size.0.max(1);
            let h = configure.new_size.1.max(1);
            tracing::debug!("region picker configure: {}x{}", w, h);
            self.region_picker_width = w;
            self.region_picker_height = h;
            self.region_picker_configured = true;
            if self.region_picker_open {
                self.draw_region_picker(qh, RepaintReason::LayerConfigure);
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
