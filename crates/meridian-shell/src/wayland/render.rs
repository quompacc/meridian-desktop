use std::hash::{Hash, Hasher};

use smithay_client_toolkit::shell::WaylandSurface;
use tracing::{debug, info, warn};
use wayland_client::QueueHandle;

use crate::{
    buffer, launcher, network_popup, panel, workspaces, Painter, Rect, CALENDAR_POPUP_HEIGHT,
    CALENDAR_POPUP_WIDTH, LAUNCHER_HEIGHT, LAUNCHER_WIDTH, NETWORK_POPUP_HEIGHT,
    NETWORK_POPUP_WIDTH, PANEL_HEIGHT, WORKSPACE_POPUP_HEIGHT, WORKSPACE_POPUP_WIDTH,
};

use super::{
    calendar::{weekday_labels, CalendarMonthModel},
    shell::{LauncherRenderSignature, PanelRenderSignature, ThemeRenderSignature},
    time, CommitReason, CommitSurfaceKind, MeridianShell, RepaintReason, SurfaceKind,
};

const CANVAS_RETRY_ATTEMPTS: usize = 2;

impl MeridianShell {
    fn signature_hash<T: Hash>(value: &T) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    fn theme_render_signature(&self) -> ThemeRenderSignature {
        ThemeRenderSignature {
            font_ui: self.theme.fonts.ui.clone(),
            colors: [
                self.theme.colors.background.r,
                self.theme.colors.background.g,
                self.theme.colors.background.b,
                self.theme.colors.background.a,
                self.theme.colors.surface.r,
                self.theme.colors.surface.g,
                self.theme.colors.surface.b,
                self.theme.colors.surface.a,
                self.theme.colors.accent.r,
                self.theme.colors.accent.g,
                self.theme.colors.accent.b,
                self.theme.colors.accent.a,
                self.theme.colors.text.r,
                self.theme.colors.text.g,
                self.theme.colors.text.b,
                self.theme.colors.text.a,
                self.theme.colors.border.r,
                self.theme.colors.border.g,
                self.theme.colors.border.b,
                self.theme.colors.border.a,
            ],
        }
    }

    fn panel_window_entries(&self, active_workspace: u8) -> Vec<panel::PanelWindowEntry> {
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
            network_popup_open: self.network_popup_open,
            theme: self.theme_render_signature(),
        }
    }

    fn launcher_apps_hash(apps: &[crate::launcher::DesktopApp]) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for app in apps {
            app.name.hash(&mut hasher);
            app.program.hash(&mut hasher);
            app.args.hash(&mut hasher);
            app.terminal.hash(&mut hasher);
            app.icon_name.hash(&mut hasher);
        }
        hasher.finish()
    }

    fn launcher_render_signature(
        &self,
        width: u32,
        height: u32,
        visible_apps: &[crate::launcher::DesktopApp],
    ) -> LauncherRenderSignature {
        LauncherRenderSignature {
            open: self.launcher_state.open,
            width,
            height,
            query: self.launcher_state.query.clone(),
            mode: self.launcher_state.current_mode(),
            sidebar_category: self.launcher_state.sidebar_category,
            pending_action_confirmation: self.launcher_state.pending_action_confirmation(),
            selected_index: self.launcher_state.selected_index,
            visible_apps_len: visible_apps.len(),
            visible_apps_hash: Self::launcher_apps_hash(visible_apps),
            theme: self.theme_render_signature(),
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
            CommitReason::FrameCallback => "frame_callback",
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

        let panel_active_workspace = self.panel_active_workspace();
        let panel_window_entries = self.panel_window_entries(panel_active_workspace);
        let width = self.width;
        let height = PANEL_HEIGHT;
        let clock = if self.last_clock.is_empty() {
            time::formatted_time()
        } else {
            self.last_clock.clone()
        };
        let signature = self.panel_render_signature(width, height, panel_active_workspace, &clock);
        if self.panel_last_signature.as_ref() == Some(&signature) {
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

            let mut painter = Painter::new(canvas, width as i32, height as i32);
            panel::draw_panel(
                &mut self.panel_state,
                &mut painter,
                panel::PanelDrawInput {
                    font: &self.font,
                    theme: &self.theme,
                    icon_cache: &self.icon_cache,
                    active_workspace: panel_active_workspace,
                    total_workspaces: 9,
                    pinned_apps: &self.pinned_apps,
                    window_entries: &panel_window_entries,
                    clock: &clock,
                    network_state: self.network_controller.state(),
                    network_popup_open: self.network_popup_open,
                    width,
                    hover_pos: (self.pointer_surface == SurfaceKind::Panel)
                        .then_some(self.pointer_position),
                },
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
            self.panel_last_signature = Some(signature);
            self.panel_dirty = false;
            return;
        }
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

        let width = LAUNCHER_WIDTH;
        let height = LAUNCHER_HEIGHT;
        debug!(
            "draw_launcher size: configured={}x{} effective={}x{} desired={}x{}",
            self.launcher_width,
            self.launcher_height,
            width,
            height,
            LAUNCHER_WIDTH,
            LAUNCHER_HEIGHT
        );
        let visible_apps = self.launcher_state.filtered_apps();
        let signature = self.launcher_render_signature(width, height, &visible_apps);
        if self.launcher_last_signature.as_ref() == Some(&signature) {
            self.render_stats.launcher.skips += 1;
            debug!(
                "draw_launcher skipped: reason={:?} commit=no signature_unchanged=true",
                reason
            );
            if self.render_stats_enabled {
                debug!("shell render skip: surface=launcher reason=signature-unchanged");
            }
            self.launcher_dirty = false;
            tracing::trace!("draw_launcher skipped: unchanged render signature");
            return;
        }
        self.render_stats.launcher.renders += 1;
        if self.render_stats_enabled {
            let old_sig = self
                .launcher_last_signature
                .as_ref()
                .map(Self::signature_hash)
                .unwrap_or(0);
            let new_sig = Self::signature_hash(&signature);
            debug!(
                "shell render commit: surface=launcher reason={:?} old_sig={} new_sig={}",
                reason, old_sig, new_sig
            );
        }

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

            let mut painter = Painter::new(canvas, width as i32, height as i32);
            launcher::draw_launcher(
                &mut self.launcher_state,
                &mut painter,
                &self.font,
                &self.theme,
                &self.icon_cache,
                width,
                height,
            );

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
            self.launcher_last_signature = Some(signature);
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
        self.launcher_last_signature = None;
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
            painter.clear(self.theme.colors.surface_alt);
            let card = Rect {
                x: 4,
                y: 4,
                w: width as i32 - 8,
                h: height as i32 - 8,
            };
            painter.rect(card, self.theme.colors.surface);
            painter.stroke_rect(card, self.theme.colors.border);

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

                let content = Rect {
                    x: card.x + 12,
                    y: card.y + 8,
                    w: card.w - 24,
                    h: card.h - 16,
                };
                let header_rect = Rect {
                    x: content.x,
                    y: content.y,
                    w: content.w,
                    h: 24,
                };
                let header_text = format!("{:02} / {}", model.month, model.year);
                painter.text_centered(
                    &self.font,
                    &header_text,
                    header_rect,
                    self.theme.colors.text,
                );

                let weekday_y = header_rect.y + header_rect.h + 8;
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
                        self.theme.colors.text,
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
                    hover_pos: (self.pointer_surface == SurfaceKind::WorkspacePopup)
                        .then_some(self.pointer_position),
                },
                &mut self.workspace_state,
            );

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
    }
}
