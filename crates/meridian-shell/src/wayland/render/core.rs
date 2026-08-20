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
    pub(crate) fn draw_panel(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        if self.web_panel_enabled {
            self.panel_dirty = false;
            return;
        }
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
        let height = crate::PANEL_SURFACE_HEIGHT;
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
                &self.battery_snapshot,
                self.power_profile,
                panel_active_w,
                9,
                &clock,
                &self.icon_cache,
                screenshot_icon,
                &self.theme,
                &state_fn,
                &mut self.panel_state.clicks,
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

    pub(crate) fn activate_native_panel_fallback(&mut self) {
        if !self.web_panel_enabled {
            return;
        }
        self.web_panel_enabled = false;
        self.panel.set_size(0, crate::PANEL_SURFACE_HEIGHT);
        self.panel
            .set_exclusive_zone(crate::PANEL_SURFACE_HEIGHT as i32);
        self.panel_dirty = true;
        self.panel_last_signature = None;
        self.commit_surface(CommitSurfaceKind::Panel, CommitReason::InitialCreate);
        warn!("WebKit panel unavailable; native panel fallback committed");
    }
}
