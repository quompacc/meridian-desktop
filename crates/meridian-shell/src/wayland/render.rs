use std::hash::{Hash, Hasher};

use smithay_client_toolkit::shell::WaylandSurface;
use tracing::{debug, info};
use wayland_client::QueueHandle;

use crate::{buffer, launcher, panel, Painter, LAUNCHER_HEIGHT, LAUNCHER_WIDTH, PANEL_HEIGHT};

use super::{
    shell::{LauncherRenderSignature, PanelRenderSignature, ThemeRenderSignature},
    time, CommitReason, CommitSurfaceKind, MeridianShell, RepaintReason,
};

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
            ],
        }
    }

    fn panel_render_signature(
        &self,
        width: u32,
        height: u32,
        active_workspace: u8,
        clock: &str,
    ) -> PanelRenderSignature {
        PanelRenderSignature {
            width,
            height,
            active_workspace,
            occupied_state_available: self.occupied_state_available,
            occupied_workspaces: self.occupied_workspaces,
            focused_title: self.focused_title.clone(),
            clock: clock.to_string(),
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
        static DRAW_PANEL_LOGS: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        let draw_log = DRAW_PANEL_LOGS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if draw_log < 6 {
            debug!(
                "draw_panel: configured={}, width={}",
                self.panel_configured, self.width
            );
        }

        if !self.panel_configured || self.width == 0 {
            return;
        }
        self.repaint_stats.record_panel(reason);

        let panel_active_workspace = self.panel_active_workspace();
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
        let buf = buffer::buffer_for(
            &mut self.pool,
            &mut self.panel_buffer,
            width,
            height,
            stride,
        );
        let Some(canvas) = buf.canvas(&mut self.pool) else {
            self.panel_buffer = None;
            return self.draw_panel(_qh, reason);
        };

        let mut painter = Painter::new(canvas, width as i32, height as i32);
        panel::draw_panel(
            &mut self.panel_state,
            &mut painter,
            &self.font,
            &self.theme,
            panel_active_workspace,
            self.occupied_state_available
                .then_some(&self.occupied_workspaces),
            self.focused_title.as_deref(),
            &clock,
            width,
        );
        if self.workspace_indicator_dirty {
            tracing::debug!(
                "panel workspace indicator updated: active_workspace={} legacy_active_workspace={}",
                panel_active_workspace,
                self.active_workspace
            );
            self.workspace_indicator_dirty = false;
        }

        buf.attach_to(self.panel.wl_surface())
            .expect("panel buffer attach");
        self.panel
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);
        self.commit_surface(
            CommitSurfaceKind::Panel,
            Self::commit_reason_from_repaint(reason, true),
        );
        self.panel_last_signature = Some(signature);
        self.panel_dirty = false;
    }

    pub(crate) fn draw_launcher(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        if !self.launcher_configured || !self.launcher_state.open {
            return;
        }
        self.repaint_stats.record_launcher(reason);

        let width = self.launcher_width.max(LAUNCHER_WIDTH);
        let height = self.launcher_height.max(LAUNCHER_HEIGHT);
        let visible_apps = self.launcher_state.filtered_apps();
        let signature = self.launcher_render_signature(width, height, &visible_apps);
        if self.launcher_last_signature.as_ref() == Some(&signature) {
            self.render_stats.launcher.skips += 1;
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
        let buf = buffer::buffer_for(
            &mut self.pool,
            &mut self.launcher_buffer,
            width,
            height,
            stride,
        );
        let Some(canvas) = buf.canvas(&mut self.pool) else {
            self.launcher_buffer = None;
            return self.draw_launcher(_qh, reason);
        };

        let mut painter = Painter::new(canvas, width as i32, height as i32);
        launcher::draw_launcher(
            &mut self.launcher_state,
            &mut painter,
            &self.font,
            &self.theme,
            width,
            height,
        );

        self.launcher_layer
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);
        buf.attach_to(self.launcher_layer.wl_surface())
            .expect("launcher buffer attach");
        self.commit_surface(
            CommitSurfaceKind::Launcher,
            Self::commit_reason_from_repaint(reason, false),
        );
        self.launcher_last_signature = Some(signature);
        self.launcher_dirty = false;
    }

    pub(crate) fn unmap_launcher(&mut self, reason: CommitReason) {
        self.launcher_layer.wl_surface().attach(None, 0, 0);
        self.commit_surface(CommitSurfaceKind::Launcher, reason);
        self.launcher_last_signature = None;
        self.launcher_dirty = false;
    }
}
