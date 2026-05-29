use std::hash::{Hash, Hasher};

use smithay_client_toolkit::shell::WaylandSurface;
use tracing::{debug, info, warn};
use wayland_client::QueueHandle;

use crate::{
    audio_popup, buffer, network_popup, notification_popup, panel, status_notifier_popup,
    thumbnail_popup, workspaces, Painter, Rect, AUDIO_POPUP_HEIGHT, AUDIO_POPUP_WIDTH,
    CALENDAR_POPUP_HEIGHT, CALENDAR_POPUP_WIDTH, LAUNCHER_HEIGHT, LAUNCHER_WIDTH,
    NETWORK_POPUP_HEIGHT, NETWORK_POPUP_WIDTH, NOTIFICATION_HEIGHT, NOTIFICATION_WIDTH,
    THUMBNAIL_POPUP_HEIGHT, THUMBNAIL_POPUP_MAX_WIDTH, WORKSPACE_POPUP_HEIGHT,
    WORKSPACE_POPUP_WIDTH,
};

use super::{
    calendar::{weekday_labels, CalendarMonthModel},
    shell::{PanelRenderSignature, ThemeRenderSignature},
    time, CommitReason, CommitSurfaceKind, MeridianShell, RepaintReason,
};

const CANVAS_RETRY_ATTEMPTS: usize = 2;

impl MeridianShell {
    fn signature_hash<T: Hash>(value: &T) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    fn theme_render_signature(&self) -> ThemeRenderSignature {
        let colors = [
            self.theme.colors.background,
            self.theme.colors.surface,
            self.theme.colors.surface_alt,
            self.theme.colors.accent,
            self.theme.colors.accent_alt,
            self.theme.colors.text,
            self.theme.colors.text_dim,
            self.theme.colors.border,
            self.theme.colors.error,
            self.theme.colors.warning,
            self.theme.colors.success,
        ];
        let mut bytes = [0; 44];
        for (idx, color) in colors.iter().enumerate() {
            let offset = idx * 4;
            bytes[offset] = color.r;
            bytes[offset + 1] = color.g;
            bytes[offset + 2] = color.b;
            bytes[offset + 3] = color.a;
        }
        ThemeRenderSignature {
            font_ui: self.theme.fonts.ui.clone(),
            colors: bytes,
        }
    }

    pub(crate) fn panel_window_entries(
        &self,
        active_workspace: u8,
    ) -> Vec<panel::PanelWindowEntry> {
        let focused_window_id = self.focused_window_id.as_deref();
        self.windows
            .iter()
            .filter(|window| window.workspace == active_workspace)
            .map(|window| panel::PanelWindowEntry {
                id: window.id.clone(),
                title: if window.title.trim().is_empty() {
                    "Window".to_string()
                } else {
                    window.title.clone()
                },
                focused: focused_window_id.is_some_and(|id| id == window.id),
                minimized: window.minimized,
                app_id: window.app_id.clone(),
            })
            .collect()
    }

    fn panel_render_signature(
        &self,
        width: u32,
        height: u32,
        active_workspace: u8,
        clock: &str,
    ) -> PanelRenderSignature {
        let window_entries = self.panel_window_entries(active_workspace);
        PanelRenderSignature {
            width,
            height,
            active_workspace,
            occupied_state_available: self.occupied_state_available,
            occupied_workspaces: self.occupied_workspaces,
            focused_title: self.focused_title.clone(),
            window_entries,
            clock: clock.to_string(),
            network_icon: self.network_controller.state().icon_name(),
            audio_label: self.audio_snapshot.panel_label(),
            audio_icon: self.audio_snapshot.icon_name(),
            status_notifier_items: self
                .status_notifier_items
                .iter()
                .map(|item| {
                    format!(
                        "{}|{}|{}|{}",
                        item.service,
                        item.title.as_deref().unwrap_or(""),
                        item.icon_name.as_deref().unwrap_or(""),
                        item.menu_path.as_deref().unwrap_or("")
                    )
                })
                .collect(),
            network_popup_open: self.network_popup_open,
            audio_popup_open: self.audio_popup_open,
            hover_widget_path: self
                .panel_widget_state
                .as_ref()
                .map(|(path, _)| path.as_slice().to_vec()),
            theme: self.theme_render_signature(),
            pinned_apps: self.pinned_apps.iter().map(|p| p.program.clone()).collect(),
        }
    }

    fn commit_surface_label(surface_kind: CommitSurfaceKind) -> &'static str {
        match surface_kind {
            CommitSurfaceKind::Panel => "panel",
            CommitSurfaceKind::Launcher => "launcher",
        }
    }

    fn commit_reason_label(reason: CommitReason) -> &'static str {
        match reason {
            CommitReason::InitialCreate => "initial_create",
            CommitReason::ConfigureAck => "configure_ack",
            CommitReason::DrawPanel => "draw_panel",
            CommitReason::DrawLauncher => "draw_launcher",
            CommitReason::EventLoopTick => "event_loop_tick",
            CommitReason::Input => "input",
            CommitReason::UnknownOther => "unknown_other",
        }
    }

    fn commit_reason_from_repaint(reason: RepaintReason, is_panel: bool) -> CommitReason {
        match reason {
            RepaintReason::LayerConfigure => CommitReason::ConfigureAck,
            RepaintReason::Pointer | RepaintReason::Keyboard => CommitReason::Input,
            RepaintReason::Ipc | RepaintReason::Clock => {
                if is_panel {
                    CommitReason::DrawPanel
                } else {
                    CommitReason::DrawLauncher
                }
            }
        }
    }

    pub(crate) fn commit_surface(&mut self, surface_kind: CommitSurfaceKind, reason: CommitReason) {
        self.commit_stats.record(surface_kind, reason);
        match surface_kind {
            CommitSurfaceKind::Panel => self.render_stats.panel.commits += 1,
            CommitSurfaceKind::Launcher => self.render_stats.launcher.commits += 1,
        }
        if self.commit_stats_enabled && std::time::Instant::now() <= self.commit_info_until {
            info!(
                "shell commit: surface={} reason={}",
                Self::commit_surface_label(surface_kind),
                Self::commit_reason_label(reason)
            );
        }
        tracing::trace!(
            "shell surface commit: surface={:?} reason={:?}",
            surface_kind,
            reason
        );
        match surface_kind {
            CommitSurfaceKind::Panel => self.panel.commit(),
            CommitSurfaceKind::Launcher => self.launcher_layer.commit(),
        }
    }

    /// Eased progress (0..1) of the panel entrance; 1.0 once finished.
    /// Starts the clock on the first call and latches done at the end.
    fn panel_intro_progress(&mut self) -> f32 {
        if self.panel_intro_done {
            return 1.0;
        }
        let start = *self
            .panel_intro_start
            .get_or_insert_with(std::time::Instant::now);
        let p = (start.elapsed().as_secs_f32() / crate::PANEL_INTRO_SECS).clamp(0.0, 1.0);
        if p >= 1.0 {
            self.panel_intro_start = None;
            self.panel_intro_done = true;
            return 1.0;
        }
        1.0 - (1.0 - p).powi(3)
    }

    pub(crate) fn draw_panel(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        debug!(
            "draw_panel: reason={:?} configured={} width={} panel_dirty={} launcher_open={} commit_expected={}",
            reason,
            self.panel_configured,
            self.width,
            self.panel_dirty,
            self.launcher_state.open,
            self.panel_configured && self.width > 0
        );

        if !self.panel_configured || self.width == 0 {
            debug!(
                "draw_panel skipped: reason={:?} configured={} width={}",
                reason, self.panel_configured, self.width
            );
            return;
        }
        self.repaint_stats.record_panel(reason);

        let intro_progress = self.panel_intro_progress();
        let intro_active = intro_progress < 1.0;

        let panel_active_workspace = self.panel_active_workspace();
        let panel_window_entries = self.panel_window_entries(panel_active_workspace);
        let width = self.width;
        let height = crate::PANEL_SURFACE_HEIGHT;
        let clock = if self.last_clock.is_empty() {
            time::formatted_time()
        } else {
            self.last_clock.clone()
        };
        let signature = self.panel_render_signature(width, height, panel_active_workspace, &clock);
        if !intro_active && self.panel_last_signature.as_ref() == Some(&signature) {
            self.render_stats.panel.skips += 1;
            debug!(
                "draw_panel skipped: reason={:?} commit=no signature_unchanged=true",
                reason
            );
            if self.render_stats_enabled {
                debug!("shell render skip: surface=panel reason=signature-unchanged");
            }
            self.panel_dirty = false;
            tracing::trace!("draw_panel skipped: unchanged render signature");
            return;
        }
        self.render_stats.panel.renders += 1;
        if self.render_stats_enabled {
            let old_sig = self
                .panel_last_signature
                .as_ref()
                .map(Self::signature_hash)
                .unwrap_or(0);
            let new_sig = Self::signature_hash(&signature);
            debug!(
                "shell render commit: surface=panel reason={:?} old_sig={} new_sig={}",
                reason, old_sig, new_sig
            );
        }

        let stride = buffer::shm_buffer_stride(width);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.panel_buffer,
                width,
                height,
                stride,
            );
            let Some(buf) = buf else {
                warn!(
                    "panel buffer unavailable: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.panel_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!(
                    "panel canvas unavailable after retry: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };

            let panel_active_w = panel_active_workspace;
            let state_fn = |path: &[usize]| -> meridian_ui::WidgetState {
                match self.panel_widget_state.as_ref() {
                    Some((p, s)) if p.as_slice() == path => *s,
                    _ => meridian_ui::WidgetState::Idle,
                }
            };
            let screenshot_icon = self
                .icon_cache
                .lookup("camera-photo-symbolic", 22)
                .and_then(crate::icons::icon_image_to_pixmap);

            crate::panel_view::draw_panel_ui(
                canvas,
                width,
                height,
                &self.pinned_apps,
                &panel_window_entries,
                self.network_controller.state(),
                &self.audio_snapshot,
                &self.status_notifier_items,
                self.network_popup_open,
                self.audio_popup_open,
                panel_active_w,
                9,
                &clock,
                &self.icon_cache,
                screenshot_icon,
                &self.theme,
                &state_fn,
                &mut self.panel_state.clicks,
                intro_progress,
            );
            if self.workspace_indicator_dirty {
                tracing::debug!(
                    "panel workspace indicator updated: active_workspace={} legacy_active_workspace={}",
                    panel_active_workspace,
                    self.active_workspace
                );
                self.workspace_indicator_dirty = false;
            }

            if let Err(err) = buf.attach_to(self.panel.wl_surface()) {
                warn!(
                    "panel buffer attach failed: reason={:?} width={} height={} error={}",
                    reason, width, height, err
                );
                return;
            }
            self.panel
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);
            self.commit_surface(
                CommitSurfaceKind::Panel,
                Self::commit_reason_from_repaint(reason, true),
            );
            debug!(
                "draw_panel committed: reason={:?} width={} height={}",
                reason, width, height
            );
            self.write_panel_click_zones_snapshot(width, height);
            self.panel_last_signature = Some(signature);
            self.panel_dirty = false;
            return;
        }
    }

    #[allow(dead_code)]
    pub(crate) fn draw_desktop(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        debug!(
            "draw_desktop: reason={:?} configured={} size={}x{}",
            reason, self.desktop_configured, self.desktop_width, self.desktop_height
        );
        if !self.desktop_configured || self.desktop_width == 0 || self.desktop_height == 0 {
            return;
        }

        let width = self.desktop_width;
        let height = self.desktop_height;
        let stride = buffer::shm_buffer_stride(width);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.desktop_buffer,
                width,
                height,
                stride,
            );
            let Some(buf) = buf else {
                warn!(
                    "desktop buffer unavailable: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.desktop_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!(
                    "desktop canvas unavailable after retry: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };

            canvas.fill(0);
            if let Err(err) = buf.attach_to(self.desktop_layer.wl_surface()) {
                warn!(
                    "desktop buffer attach failed: reason={:?} width={} height={} error={}",
                    reason, width, height, err
                );
                return;
            }
            self.desktop_layer
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);
            self.desktop_layer.commit();
            return;
        }
    }

    /// Resize the desktop-menu surface to match whether the settings flyout is open.
    /// Must be called whenever `desktop_context_menu.submenu_open` changes.
    pub(crate) fn resize_desktop_menu_surface(&mut self, submenu_open: bool) {
        use smithay_client_toolkit::shell::wlr_layer::Anchor;
        let n = crate::context_menu::desktop_item_list().len();
        let new_w = crate::context_menu::total_menu_width(submenu_open) as u32;
        let new_h = crate::context_menu::surface_height(n, submenu_open).max(1) as u32;
        self.desktop_menu_width = new_w;
        self.desktop_menu_height = new_h;
        self.desktop_menu_buffer = None;
        self.desktop_menu_layer.set_anchor(Anchor::TOP | Anchor::LEFT);
        self.desktop_menu_layer.set_size(new_w, new_h);
    }

    pub(crate) fn draw_desktop_menu(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        debug!(
            "draw_desktop_menu: reason={:?} open={} configured={} size={}x{}",
            reason,
            self.desktop_menu_open,
            self.desktop_menu_configured,
            self.desktop_menu_width,
            self.desktop_menu_height
        );
        if !self.desktop_menu_open || !self.desktop_menu_configured {
            return;
        }
        let Some(menu) = self.desktop_context_menu.as_ref() else {
            return;
        };

        let width = self.desktop_menu_width.max(1);
        let height = self.desktop_menu_height.max(1);
        let stride = buffer::shm_buffer_stride(width);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.desktop_menu_buffer,
                width,
                height,
                stride,
            );
            let Some(buf) = buf else {
                warn!(
                    "desktop menu buffer unavailable: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.desktop_menu_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!(
                    "desktop menu canvas unavailable after retry: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };

            canvas.fill(0);
            let local = crate::context_menu::DesktopContextMenuState {
                x: 0,
                y: 0,
                hover_idx: menu.hover_idx,
                submenu_open: menu.submenu_open,
                submenu_hover_idx: menu.submenu_hover_idx,
            };
            let items = crate::context_menu::desktop_item_list();
            crate::context_menu::draw_desktop_overlay(
                canvas,
                width,
                height,
                &local,
                &items,
                &self.theme,
            );
            if let Err(err) = buf.attach_to(self.desktop_menu_layer.wl_surface()) {
                warn!(
                    "desktop menu buffer attach failed: reason={:?} width={} height={} error={}",
                    reason, width, height, err
                );
                return;
            }
            self.desktop_menu_layer
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);
            self.desktop_menu_layer.commit();
            return;
        }
    }

    pub(crate) fn unmap_desktop_menu(&mut self, _reason: CommitReason) {
        // Re-assert a valid, anchored, non-zero size *before* committing the
        // null buffer. Hiding the menu often coincides with a re-arrange (e.g.
        // the "Launcher öffnen" item opens the fullscreen launcher), and during
        // that arrange smithay can see this single-anchored (TOP|LEFT) surface
        // with a 0 width and post a protocol error that tears down the whole
        // shell. Keeping anchor+size valid on the unmap commit prevents it.
        // Also reset to base dimensions so any spurious configure that fires
        // after unmap does not re-assert an old expanded (submenu-open) width.
        use smithay_client_toolkit::shell::wlr_layer::Anchor;
        let base_w = crate::context_menu::MENU_WIDTH as u32;
        let base_h =
            crate::context_menu::menu_height(crate::context_menu::desktop_item_list().len())
                .max(1) as u32;
        self.desktop_menu_width = base_w;
        self.desktop_menu_height = base_h;
        self.desktop_menu_layer
            .set_anchor(Anchor::TOP | Anchor::LEFT);
        self.desktop_menu_layer.set_size(base_w, base_h);
        self.desktop_menu_layer.wl_surface().attach(None, 0, 0);
        self.desktop_menu_layer.commit();
    }

    pub(crate) fn draw_launcher(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        debug!(
            "draw_launcher: reason={:?} open={} configured={} launcher_dirty={} commit_expected={}",
            reason,
            self.launcher_state.open,
            self.launcher_configured,
            self.launcher_dirty,
            self.launcher_configured && self.launcher_state.open
        );
        if !self.launcher_configured || !self.launcher_state.open {
            debug!(
                "draw_launcher skipped: reason={:?} open={} configured={}",
                reason, self.launcher_state.open, self.launcher_configured
            );
            return;
        }
        self.repaint_stats.record_launcher(reason);

        let width = if self.launcher_is_fullscreen {
            self.launcher_width
        } else {
            LAUNCHER_WIDTH
        };
        let height = if self.launcher_is_fullscreen {
            self.launcher_height
        } else {
            LAUNCHER_HEIGHT
        };
        debug!(
            "draw_launcher size: configured={}x{} effective={}x{} desired={}x{}",
            self.launcher_width,
            self.launcher_height,
            width,
            height,
            LAUNCHER_WIDTH,
            LAUNCHER_HEIGHT
        );
        self.render_stats.launcher.renders += 1;

        let stride = buffer::shm_buffer_stride(width);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.launcher_buffer,
                width,
                height,
                stride,
            );
            let Some(buf) = buf else {
                warn!(
                    "launcher buffer unavailable: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.launcher_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!(
                    "launcher canvas unavailable after retry: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };

            let active = self.ui_preview_widget_state.as_ref();
            let state_fn = |path: &[usize]| -> meridian_ui::WidgetState {
                match active {
                    Some((p, s)) if p.as_slice() == path => *s,
                    _ => meridian_ui::WidgetState::Idle,
                }
            };

            // Render launcher content to a fixed-size buffer.
            let lw = LAUNCHER_WIDTH as usize;
            let lh = LAUNCHER_HEIGHT as usize;
            let mut content = vec![0u8; lw * lh * 4];
            let armed_power: Option<(&str, f32)> = self.armed_power.as_ref().map(|(id, at)| {
                let p = (at.elapsed().as_millis() as f32 / crate::POWER_ARM_TIMEOUT_MS as f32)
                    .clamp(0.0, 1.0);
                (id.as_str(), p)
            });
            if self.launcher_settings_open {
                crate::settings_view::draw_settings_launcher(
                    &mut content,
                    LAUNCHER_WIDTH,
                    LAUNCHER_HEIGHT,
                    self.settings_category,
                    &self.settings_search,
                    &self.available_themes,
                    &self.theme_name,
                    &self.available_wallpapers,
                    &self.wallpaper_thumbnails,
                    self.wallpaper_path.as_deref(),
                    self.wallpaper_mode,
                    &self.pinned_apps,
                    &self.output_workspaces,
                    self.display_mode_dropdown_open,
                    &self.printer_snapshot,
                    &self.audio_snapshot,
                    self.settings_pinned_adding,
                    &self.launcher_state.apps,
                    &self.icon_cache,
                    armed_power,
                    &self.theme,
                    &state_fn,
                );
            } else {
                crate::app_view::draw_command_palette(
                    &mut content,
                    LAUNCHER_WIDTH,
                    LAUNCHER_HEIGHT,
                    &self.pinned_apps,
                    &self.launcher_state.apps,
                    &self.search_query,
                    self.app_view_scroll_y,
                    self.launcher_selected_idx,
                    armed_power,
                    &self.icon_cache,
                    &self.hidden_execs,
                    self.hovered_app_card_idx,
                    self.hovered_bento_idx,
                    self.settings_hovered,
                    self.hovered_power_btn,
                    &self.theme,
                );
            }
            if let Some(ref cm) = self.context_menu {
                let items = crate::context_menu::item_list(
                    cm.is_terminal,
                    cm.is_pinned,
                    cm.running_window_id.is_some(),
                );
                crate::context_menu::draw_overlay(
                    &mut content,
                    LAUNCHER_WIDTH,
                    LAUNCHER_HEIGHT,
                    cm,
                    &items,
                    &[],
                    &[],
                    &self.theme,
                );
            }

            // Round the launcher's outer corners so it matches the panel island.
            round_buffer_corners(&mut content, lw, lh, 12);

            if self.launcher_is_fullscreen {
                // Blit LAUNCHER_WxH content into the full-screen canvas at visual offset.
                let fw = width as usize;
                let vx = self.launcher_visual_x.max(0) as usize;
                let vy = self.launcher_visual_y.max(0) as usize;
                canvas.fill(0);
                // Soft drop shadow around the rounded launcher, painted before
                // the content so the rounded corners keep their shadow.
                crate::soft_shadow::draw_soft_shadow(
                    canvas,
                    width as i32,
                    height as i32,
                    vx as i32,
                    vy as i32,
                    lw as i32,
                    lh as i32,
                    12.0,
                    18.0,
                    0.16,
                    4,
                    false,
                );
                // Composite the (premultiplied) content over the shadow so the
                // transparent rounded corners reveal the shadow underneath.
                for y in 0..lh {
                    let src = &content[y * lw * 4..(y + 1) * lw * 4];
                    let dst_off = (vy + y) * fw * 4 + vx * 4;
                    if dst_off + lw * 4 > canvas.len() {
                        continue;
                    }
                    let dst = &mut canvas[dst_off..dst_off + lw * 4];
                    for x in 0..lw {
                        let s = &src[x * 4..x * 4 + 4];
                        let sa = s[3] as u32;
                        if sa == 255 {
                            dst[x * 4..x * 4 + 4].copy_from_slice(s);
                        } else if sa != 0 {
                            let inv = 255 - sa;
                            for k in 0..4 {
                                dst[x * 4 + k] =
                                    (s[k] as u32 + dst[x * 4 + k] as u32 * inv / 255) as u8;
                            }
                        }
                    }
                }
            } else {
                canvas[..lw * lh * 4].copy_from_slice(&content);
            }

            self.launcher_layer
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);
            if let Err(err) = buf.attach_to(self.launcher_layer.wl_surface()) {
                warn!(
                    "launcher buffer attach failed: reason={:?} width={} height={} error={}",
                    reason, width, height, err
                );
                return;
            }
            self.commit_surface(
                CommitSurfaceKind::Launcher,
                Self::commit_reason_from_repaint(reason, false),
            );
            debug!(
                "draw_launcher committed: reason={:?} width={} height={}",
                reason, width, height
            );
            self.launcher_dirty = false;
            return;
        }
    }

    pub(crate) fn unmap_launcher(&mut self, reason: CommitReason) {
        debug!(
            "unmap_launcher: reason={:?} open={} configured={} surface=launcher attach_none=true commit=true",
            reason,
            self.launcher_state.open,
            self.launcher_configured
        );
        self.launcher_layer.wl_surface().attach(None, 0, 0);
        self.commit_surface(CommitSurfaceKind::Launcher, reason);
        self.launcher_dirty = false;
    }

    pub(crate) fn draw_calendar_popup(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        debug!(
            "draw_calendar_popup: reason={:?} open={} configured={} calendar_dirty={} commit_expected={}",
            reason,
            self.calendar_popup_open,
            self.calendar_configured,
            self.calendar_dirty,
            self.calendar_popup_open && self.calendar_configured
        );
        if !self.calendar_popup_open || !self.calendar_configured {
            debug!(
                "draw_calendar_popup skipped: reason={:?} open={} configured={}",
                reason, self.calendar_popup_open, self.calendar_configured
            );
            return;
        }

        let width = self.calendar_width.min(CALENDAR_POPUP_WIDTH);
        let height = self.calendar_height.min(CALENDAR_POPUP_HEIGHT);
        let stride = buffer::shm_buffer_stride(width);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.calendar_buffer,
                width,
                height,
                stride,
            );
            let Some(buf) = buf else {
                warn!(
                    "calendar popup buffer unavailable: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.calendar_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!(
                    "calendar popup canvas unavailable after retry: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };

            let mut painter = Painter::new(canvas, width as i32, height as i32);
            crate::popup_card::draw_card_body(&mut painter, &self.theme);
            let card = Rect {
                x: 0,
                y: 0,
                w: width as i32,
                h: height as i32,
            };

            let maybe_model = time::local_date().and_then(|date| {
                CalendarMonthModel::for_month(
                    date.year,
                    date.month,
                    Some(date.day),
                    self.calendar_display_policy.week_start,
                )
            });

            if let Some(model) = maybe_model {
                let labels = weekday_labels(self.calendar_display_policy.week_start);
                debug_assert_eq!(
                    model
                        .cells
                        .iter()
                        .position(|cell| cell.is_some())
                        .unwrap_or(0),
                    usize::from(model.first_weekday_col0)
                );

                let header_text = format!(
                    "{} {}",
                    german_month_name(model.month),
                    model.year
                );
                crate::popup_card::draw_card_title(
                    &mut painter,
                    &self.font,
                    &self.theme,
                    &header_text,
                );

                let content = Rect {
                    x: card.x + crate::popup_card::PAD_X,
                    y: crate::popup_card::BODY_TOP,
                    w: card.w - 2 * crate::popup_card::PAD_X,
                    h: (card.h - crate::popup_card::BODY_TOP - crate::popup_card::PAD_BOTTOM).max(1),
                };

                let weekday_y = content.y;
                let weekday_h = 18;
                for (col, label) in labels.iter().enumerate() {
                    let x0 = content.x + (col as i32 * content.w) / 7;
                    let x1 = content.x + (((col + 1) as i32 * content.w) / 7);
                    painter.text_centered(
                        &self.font,
                        label,
                        Rect {
                            x: x0,
                            y: weekday_y,
                            w: x1 - x0,
                            h: weekday_h,
                        },
                        self.theme.colors.text_dim,
                    );
                }

                let grid_y = weekday_y + weekday_h + 6;
                let grid_h = (content.y + content.h) - grid_y;
                for row in 0_usize..6 {
                    let row_i32 = row as i32;
                    let y0 = grid_y + (row_i32 * grid_h) / 6;
                    let y1 = grid_y + (((row_i32 + 1) * grid_h) / 6);
                    for col in 0_usize..7 {
                        let idx = row * 7 + col;
                        let Some(day) = model.cells[idx] else {
                            continue;
                        };

                        let col_i32 = col as i32;
                        let x0 = content.x + (col_i32 * content.w) / 7;
                        let x1 = content.x + (((col_i32 + 1) * content.w) / 7);
                        let cell_rect = Rect {
                            x: x0,
                            y: y0,
                            w: x1 - x0,
                            h: y1 - y0,
                        };
                        let is_today = model.today_day == Some(day);
                        let day_text = day.to_string();
                        if is_today {
                            let highlight = Rect {
                                x: cell_rect.x + 3,
                                y: cell_rect.y + 2,
                                w: (cell_rect.w - 6).max(0),
                                h: (cell_rect.h - 4).max(0),
                            };
                            if highlight.w > 0 && highlight.h > 0 {
                                painter.rect(highlight, self.theme.colors.accent);
                            }
                            painter.text_centered(
                                &self.font,
                                &day_text,
                                cell_rect,
                                crate::ui::tokens::ACCENT_FOREGROUND,
                            );
                        } else {
                            painter.text_centered(
                                &self.font,
                                &day_text,
                                cell_rect,
                                self.theme.colors.text,
                            );
                        }
                    }
                }
            } else {
                let time_text = if self.last_clock.is_empty() {
                    time::formatted_time()
                } else {
                    self.last_clock.clone()
                };
                let text_rect = Rect {
                    x: card.x + 12,
                    y: card.y + 16,
                    w: card.w - 24,
                    h: 28,
                };
                painter.text_centered(&self.font, &time_text, text_rect, self.theme.colors.text);
            }

            round_buffer_corners(canvas, width as usize, height as usize, crate::popup_card::CARD_RADIUS);
            if let Err(err) = buf.attach_to(self.calendar_layer.wl_surface()) {
                warn!(
                    "calendar popup buffer attach failed: reason={:?} width={} height={} error={}",
                    reason, width, height, err
                );
                return;
            }
            self.calendar_layer
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);
            self.calendar_layer.commit();
            debug!(
                "draw_calendar_popup committed: reason={:?} width={} height={}",
                reason, width, height
            );
            self.calendar_dirty = false;
            return;
        }
    }

    pub(crate) fn unmap_calendar_popup(&mut self, reason: CommitReason) {
        debug!(
            "unmap_calendar_popup: reason={:?} open={} configured={} surface=calendar attach_none=true commit=true",
            reason, self.calendar_popup_open, self.calendar_configured
        );
        self.calendar_layer.wl_surface().attach(None, 0, 0);
        self.calendar_layer.commit();
        self.calendar_dirty = false;
    }

    pub(crate) fn draw_workspace_popup(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        debug!(
            "draw_workspace_popup: reason={:?} open={} configured={} workspace_dirty={} commit_expected={}",
            reason,
            self.workspace_popup_open,
            self.workspace_configured,
            self.workspace_dirty,
            self.workspace_popup_open && self.workspace_configured
        );
        if !self.workspace_popup_open || !self.workspace_configured {
            debug!(
                "draw_workspace_popup skipped: reason={:?} open={} configured={}",
                reason, self.workspace_popup_open, self.workspace_configured
            );
            return;
        }

        let width = self.workspace_width.min(WORKSPACE_POPUP_WIDTH);
        let height = self.workspace_height.min(WORKSPACE_POPUP_HEIGHT);
        let active_workspace = self.panel_active_workspace() as u32;
        let stride = buffer::shm_buffer_stride(width);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.workspace_buffer,
                width,
                height,
                stride,
            );
            let Some(buf) = buf else {
                warn!(
                    "workspace popup buffer unavailable: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.workspace_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!(
                    "workspace popup canvas unavailable after retry: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };

            let mut painter = Painter::new(canvas, width as i32, height as i32);
            workspaces::draw_workspace_popup(
                &mut painter,
                &self.font,
                &self.theme,
                workspaces::WorkspacePopupInput {
                    active_workspace,
                    total_workspaces: 9,
                    occupied: self.occupied_workspaces,
                    hovered_idx: self.workspace_hover_idx,
                },
                &mut self.workspace_state,
            );
            round_buffer_corners(canvas, width as usize, height as usize, crate::popup_card::CARD_RADIUS);

            if let Err(err) = buf.attach_to(self.workspace_layer.wl_surface()) {
                warn!(
                    "workspace popup buffer attach failed: reason={:?} width={} height={} error={}",
                    reason, width, height, err
                );
                return;
            }
            self.workspace_layer
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);
            self.workspace_layer.commit();
            debug!(
                "draw_workspace_popup committed: reason={:?} width={} height={}",
                reason, width, height
            );
            self.workspace_dirty = false;
            return;
        }
    }

    pub(crate) fn unmap_workspace_popup(&mut self, reason: CommitReason) {
        debug!(
            "unmap_workspace_popup: reason={:?} open={} configured={} surface=workspace attach_none=true commit=true",
            reason, self.workspace_popup_open, self.workspace_configured
        );
        self.workspace_layer.wl_surface().attach(None, 0, 0);
        self.workspace_layer.commit();
        self.workspace_dirty = false;
    }

    pub(crate) fn draw_network_popup(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        debug!(
            "draw_network_popup: reason={:?} open={} configured={} network_dirty={} commit_expected={}",
            reason,
            self.network_popup_open,
            self.network_configured,
            self.network_dirty,
            self.network_popup_open && self.network_configured
        );
        if !self.network_popup_open || !self.network_configured {
            debug!(
                "draw_network_popup skipped: reason={:?} open={} configured={}",
                reason, self.network_popup_open, self.network_configured
            );
            return;
        }

        let width = self.network_width.min(NETWORK_POPUP_WIDTH);
        let height = self.network_height.min(NETWORK_POPUP_HEIGHT);
        let stride = buffer::shm_buffer_stride(width);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.network_buffer,
                width,
                height,
                stride,
            );
            let Some(buf) = buf else {
                warn!(
                    "network popup buffer unavailable: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.network_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!(
                    "network popup canvas unavailable after retry: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };

            let mut painter = Painter::new(canvas, width as i32, height as i32);
            network_popup::draw_network_popup(
                &mut painter,
                &self.font,
                &self.theme,
                self.network_controller.state(),
            );
            round_buffer_corners(canvas, width as usize, height as usize, crate::popup_card::CARD_RADIUS);

            if let Err(err) = buf.attach_to(self.network_layer.wl_surface()) {
                warn!(
                    "network popup buffer attach failed: reason={:?} width={} height={} error={}",
                    reason, width, height, err
                );
                return;
            }
            self.network_layer
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);
            self.network_layer.commit();
            debug!(
                "draw_network_popup committed: reason={:?} width={} height={}",
                reason, width, height
            );
            self.network_dirty = false;
            return;
        }
    }

    pub(crate) fn unmap_network_popup(&mut self, reason: CommitReason) {
        debug!(
            "unmap_network_popup: reason={:?} open={} configured={} surface=network attach_none=true commit=true",
            reason, self.network_popup_open, self.network_configured
        );
        self.network_layer.wl_surface().attach(None, 0, 0);
        self.network_layer.commit();
        self.network_dirty = false;
        // network_layer is shared with audio and SNI popups. After unmap we
        // must wait for a fresh configure before committing another buffer,
        // otherwise the next popup re-attaches into a half-mapped surface and
        // the compositor disconnects us with a layer-shell protocol error.
        self.network_configured = false;
    }

    pub(crate) fn draw_audio_popup(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        debug!(
            "draw_audio_popup: reason={:?} open={} configured={} audio_dirty={} commit_expected={}",
            reason,
            self.audio_popup_open,
            self.network_configured,
            self.audio_dirty,
            self.audio_popup_open && self.network_configured
        );
        if !self.audio_popup_open || !self.network_configured {
            return;
        }

        self.audio_snapshot = crate::audio::AudioSnapshot::poll();
        let width = self.audio_width.min(AUDIO_POPUP_WIDTH);
        let height = self.audio_height.min(AUDIO_POPUP_HEIGHT);
        let stride = buffer::shm_buffer_stride(width);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.network_buffer,
                width,
                height,
                stride,
            );
            let Some(buf) = buf else {
                warn!(
                    "audio popup buffer unavailable: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.network_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                return;
            };

            let mut painter = Painter::new(canvas, width as i32, height as i32);
            audio_popup::draw_audio_popup(
                &mut painter,
                &self.font,
                &self.theme,
                &self.audio_snapshot,
            );
            round_buffer_corners(canvas, width as usize, height as usize, crate::popup_card::CARD_RADIUS);

            if let Err(err) = buf.attach_to(self.network_layer.wl_surface()) {
                warn!(
                    "audio popup buffer attach failed: reason={:?} width={} height={} error={}",
                    reason, width, height, err
                );
                return;
            }
            self.network_layer
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);
            self.network_layer.commit();
            self.audio_dirty = false;
            return;
        }
    }

    pub(crate) fn unmap_audio_popup(&mut self, reason: CommitReason) {
        debug!(
            "unmap_audio_popup: reason={:?} open={} configured={} surface=network attach_none=true commit=true",
            reason, self.audio_popup_open, self.network_configured
        );
        self.network_layer.wl_surface().attach(None, 0, 0);
        self.network_layer.commit();
        self.audio_dirty = false;
        self.network_configured = false;
    }

    pub(crate) fn draw_status_notifier_menu(
        &mut self,
        _qh: &QueueHandle<Self>,
        reason: RepaintReason,
    ) {
        if !self.status_notifier_menu_open || !self.network_configured {
            return;
        }
        let width = self
            .status_notifier_menu_width
            .min(status_notifier_popup::SNI_MENU_WIDTH);
        let height = self
            .status_notifier_menu_height
            .min(status_notifier_popup::SNI_MENU_MAX_HEIGHT);
        let Some(menu_state) = self.status_notifier_menu.as_ref() else {
            self.close_status_notifier_menu(CommitReason::UnknownOther);
            return;
        };
        let title = menu_state
            .service
            .rsplit('.')
            .next()
            .unwrap_or(menu_state.service.as_str());
        let stride = buffer::shm_buffer_stride(width);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.network_buffer,
                width,
                height,
                stride,
            );
            let Some(buf) = buf else {
                warn!(
                    "status-notifier menu buffer unavailable: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.network_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                return;
            };

            let mut painter = Painter::new(canvas, width as i32, height as i32);
            status_notifier_popup::draw_status_notifier_menu(
                &mut painter,
                &self.font,
                &self.theme,
                title,
                &self.status_notifier_menu_entries,
                height,
            );

            if let Err(err) = buf.attach_to(self.network_layer.wl_surface()) {
                warn!(
                    "status-notifier menu buffer attach failed: reason={:?} width={} height={} error={}",
                    reason, width, height, err
                );
                return;
            }
            self.network_layer
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);
            self.network_layer.commit();
            self.network_dirty = false;
            return;
        }
    }

    pub(crate) fn unmap_status_notifier_menu(&mut self, reason: CommitReason) {
        debug!(
            "unmap_status_notifier_menu: reason={:?} open={} configured={} surface=network attach_none=true commit=true",
            reason, self.status_notifier_menu_open, self.network_configured
        );
        self.network_layer.wl_surface().attach(None, 0, 0);
        self.network_layer.commit();
        self.network_dirty = false;
        self.network_configured = false;
    }

    /// Phase A1.3: paint the front notification onto the dedicated
    /// top-right layer-surface. If the queue is empty the caller should
    /// invoke [`Self::unmap_notification_popup`] instead.
    pub(crate) fn draw_notification_popup(
        &mut self,
        _qh: &QueueHandle<Self>,
        reason: RepaintReason,
    ) {
        // Newest notification gets the spotlight; older ones stay queued
        // and become visible again when newer entries expire or are
        // closed. Stacking multiple at once is A1.3+ polish.
        let Some(notif) = self.notifications.back().cloned() else {
            self.unmap_notification_popup(CommitReason::UnknownOther);
            return;
        };
        if !self.notification_configured {
            debug!(
                "draw_notification_popup deferred: reason={:?} configured=false",
                reason
            );
            return;
        }

        let width = self.notification_width.min(NOTIFICATION_WIDTH);
        let height = self.notification_height.min(NOTIFICATION_HEIGHT);
        let stride = buffer::shm_buffer_stride(width);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.notification_buffer,
                width,
                height,
                stride,
            );
            let Some(buf) = buf else {
                warn!(
                    "notification buffer unavailable: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.notification_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!(
                    "notification canvas unavailable after retry: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };

            let mut painter = Painter::new(canvas, width as i32, height as i32);
            notification_popup::draw_notification(&mut painter, &self.font, &self.theme, &notif);

            if let Err(err) = buf.attach_to(self.notification_layer.wl_surface()) {
                warn!(
                    "notification buffer attach failed: reason={:?} width={} height={} error={}",
                    reason, width, height, err
                );
                return;
            }
            self.notification_layer
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);
            self.notification_layer.commit();
            debug!(
                "draw_notification_popup committed: reason={:?} id={} width={} height={}",
                reason, notif.id, width, height
            );
            self.notification_dirty = false;
            return;
        }
    }

    pub(crate) fn unmap_notification_popup(&mut self, reason: CommitReason) {
        debug!(
            "unmap_notification_popup: reason={:?} configured={} surface=notification attach_none=true commit=true",
            reason, self.notification_configured
        );
        self.notification_layer.wl_surface().attach(None, 0, 0);
        self.notification_layer.commit();
        self.notification_dirty = false;
    }

    pub(crate) fn draw_thumbnail_popup(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        if !self.thumbnail_popup_open || !self.thumbnail_configured {
            return;
        }

        let width = self.thumbnail_width.min(THUMBNAIL_POPUP_MAX_WIDTH);
        let height = self.thumbnail_height.min(THUMBNAIL_POPUP_HEIGHT);
        let stride = buffer::shm_buffer_stride(width);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.thumbnail_buffer,
                width,
                height,
                stride,
            );
            let Some(buf) = buf else {
                warn!(
                    "thumbnail: buffer unavailable reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.thumbnail_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                return;
            };

            let mut painter = Painter::new(canvas, width as i32, height as i32);
            thumbnail_popup::draw_thumbnail_popup(
                &mut painter,
                &self.theme,
                &self.thumbnail_cache,
                &self.thumbnail_popup_window_ids,
                width,
                height,
            );

            if let Err(err) = buf.attach_to(self.thumbnail_layer.wl_surface()) {
                warn!("thumbnail: buffer attach failed: {}", err);
                return;
            }
            self.thumbnail_layer
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);
            self.thumbnail_layer.commit();
            self.thumbnail_dirty = false;
            return;
        }
    }

    pub(crate) fn unmap_thumbnail_popup(&mut self, _reason: CommitReason) {
        self.thumbnail_layer.wl_surface().attach(None, 0, 0);
        self.thumbnail_layer.commit();
        self.thumbnail_dirty = false;
    }

    fn write_panel_click_zones_snapshot(&mut self, width: u32, height: u32) {
        let zones: Vec<_> = self
            .panel_state
            .clicks
            .iter()
            .map(|zone| {
                serde_json::json!({
                    "id": zone.id.as_deref(),
                    "action": zone.action.test_name(),
                    "x": zone.rect.x,
                    "y": zone.rect.y,
                    "w": zone.rect.w,
                    "h": zone.rect.h,
                    "center_x": zone.rect.x + zone.rect.w / 2,
                    "center_y": zone.rect.y + zone.rect.h / 2,
                })
            })
            .collect();
        let payload = serde_json::json!({
            "surface": "panel",
            "width": width,
            "height": height,
            "zones": zones,
        });
        let Ok(snapshot) = serde_json::to_string_pretty(&payload) else {
            return;
        };
        if self.panel_click_zones_snapshot.as_deref() == Some(snapshot.as_str()) {
            return;
        }
        self.panel_click_zones_snapshot = Some(snapshot.clone());

        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
        let path = std::path::Path::new(&runtime_dir).join("meridian-panel-click-zones.json");
        let tmp_path = path.with_extension("json.tmp");
        if std::fs::write(&tmp_path, snapshot).is_ok() {
            let _ = std::fs::rename(tmp_path, path);
        }
    }
}

/// German month name from a 1-based month number.
fn german_month_name(month: u8) -> &'static str {
    match month {
        1 => "Januar",
        2 => "Februar",
        3 => "März",
        4 => "April",
        5 => "Mai",
        6 => "Juni",
        7 => "Juli",
        8 => "August",
        9 => "September",
        10 => "Oktober",
        11 => "November",
        12 => "Dezember",
        _ => "",
    }
}

/// Make the four corners of a packed ARGB8888 buffer transparent so a
/// rectangular surface renders with rounded corners. Anti-aliased via a 1px
/// coverage falloff; channels are scaled together (premultiplied-safe).
fn round_buffer_corners(buf: &mut [u8], w: usize, h: usize, radius: i32) {
    if radius <= 0 || w == 0 || h == 0 {
        return;
    }
    let rad = (radius as usize).min(w / 2).min(h / 2);
    let r = rad as f32;
    let corners = [
        (r, r, 0usize, 0usize),
        ((w - rad) as f32, r, w - rad, 0),
        (r, (h - rad) as f32, 0, h - rad),
        ((w - rad) as f32, (h - rad) as f32, w - rad, h - rad),
    ];
    for (cx, cy, x0, y0) in corners {
        for yy in 0..rad {
            for xx in 0..rad {
                let px = x0 + xx;
                let py = y0 + yy;
                let dx = (px as f32 + 0.5) - cx;
                let dy = (py as f32 + 0.5) - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                let cov = (r - dist + 0.5).clamp(0.0, 1.0);
                if cov < 1.0 {
                    let idx = (py * w + px) * 4;
                    if idx + 4 <= buf.len() {
                        for k in 0..4 {
                            buf[idx + k] = (buf[idx + k] as f32 * cov).round() as u8;
                        }
                    }
                }
            }
        }
    }
}
