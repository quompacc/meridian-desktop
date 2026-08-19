impl MeridianShell {
    pub(crate) fn handle_panel_click(&mut self, qh: &QueueHandle<Self>, action: ClickAction) {
        if self.calendar_popup_open && !matches!(action, ClickAction::Clock) {
            self.close_calendar_popup(CommitReason::Input);
        }
        if self.workspace_popup_open && !matches!(action, ClickAction::ToggleWorkspacePopup) {
            self.close_workspace_popup(CommitReason::Input);
        }
        if self.network_popup_open && !matches!(action, ClickAction::ToggleNetworkPopup) {
            self.close_network_popup(CommitReason::Input);
        }
        if self.audio_popup_open && !matches!(action, ClickAction::ToggleAudioPopup) {
            self.close_audio_popup(CommitReason::Input);
        }

        match action {
            ClickAction::SwitchWorkspace(workspace) => {
                if self.active_workspace != workspace {
                    debug!(
                        "active workspace changed: old={} new={} (panel click)",
                        self.active_workspace, workspace
                    );
                    self.workspace_indicator_dirty = true;
                }
                self.active_workspace = workspace;
                self.ipc.send(&ShellCommand::SwitchWorkspace { workspace });
                self.draw_panel(qh, RepaintReason::Pointer);
            }
            ClickAction::FocusWindow(id) => {
                self.ipc.send(&ShellCommand::FocusWindow { id });
            }
            ClickAction::LaunchPinnedApp(idx) => {
                if let Some(app) = self.pinned_apps.get(idx).cloned() {
                    let ws = self.panel_active_workspace();
                    if let Some(id) = first_minimized_pinned_app_window_id(&app, &self.windows, ws)
                    {
                        self.ipc.send(&ShellCommand::FocusWindow { id });
                        return;
                    }

                    let command = ShellCommand::LaunchApp {
                        program: app.program,
                        args: app.args,
                        terminal: app.terminal,
                    };
                    if !self.ipc.send(&command) {
                        tracing::warn!("IPC unavailable, pinned app launch skipped: {}", idx);
                    }
                }
            }
            ClickAction::ToggleLauncher => {
                tracing::info!("panel click: ToggleLauncher (registered)");
                self.toggle_launcher();
                self.draw_panel(qh, RepaintReason::Pointer);
                if self.launcher_state.open {
                    self.draw_launcher(qh, RepaintReason::Pointer);
                } else {
                    self.unmap_launcher(CommitReason::Input);
                }
            }
            ClickAction::ToggleWorkspacePopup => {
                self.toggle_workspace_popup(CommitReason::Input);
                self.draw_panel(qh, RepaintReason::Pointer);
                if self.workspace_popup_open {
                    self.draw_workspace_popup(qh, RepaintReason::Pointer);
                }
            }
            ClickAction::ToggleNetworkPopup => {
                self.toggle_network_popup(CommitReason::Input);
                self.draw_panel(qh, RepaintReason::Pointer);
                if self.network_popup_open {
                    self.draw_network_popup(qh, RepaintReason::Pointer);
                }
            }
            ClickAction::ToggleAudioPopup => {
                self.toggle_audio_popup(CommitReason::Input);
                self.draw_panel(qh, RepaintReason::Pointer);
                if self.audio_popup_open {
                    self.draw_audio_popup(qh, RepaintReason::Pointer);
                }
            }
            ClickAction::CyclePowerProfile => {
                use crate::power_profile::{self, PowerProfile};
                let order = PowerProfile::ALL;
                let current = power_profile::current().unwrap_or(PowerProfile::Standard);
                let idx = order.iter().position(|&p| p == current).unwrap_or(0);
                let next = order[(idx + 1) % order.len()];
                let applied = if power_profile::set(next) {
                    next
                } else {
                    current
                };
                self.power_profile = Some(applied);
                self.show_power_profile_osd(qh, applied.label().to_string());
            }
            ClickAction::OpenSoundSettings => {
                self.open_sound_settings_from_tray(CommitReason::Input);
                self.draw_panel(qh, RepaintReason::Pointer);
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            ClickAction::OpenNetworkSettings => {
                self.open_network_settings_from_tray(CommitReason::Input);
                self.draw_panel(qh, RepaintReason::Pointer);
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            ClickAction::CloseStatusNotifierMenu => {
                self.close_status_notifier_menu(CommitReason::Input);
            }
            ClickAction::ActivateStatusNotifierItem(idx) => {
                if let Some(item) = self.status_notifier_items.get(idx) {
                    let point = panel_global_activation_point(
                        self.pointer_position,
                        self.panel_output_height_fallback(),
                    );
                    tracing::info!(
                        idx,
                        service = %item.service,
                        x = point.x,
                        y = point.y,
                        "status-notifier: panel item clicked"
                    );
                    status_notifier::activate_item(item.clone(), point);
                }
            }
            ClickAction::Clock => {
                self.toggle_calendar_popup(CommitReason::Input);
                self.draw_panel(qh, RepaintReason::Pointer);
                if self.calendar_popup_open {
                    self.draw_calendar_popup(qh, RepaintReason::Pointer);
                }
            }
            ClickAction::TakeScreenshot => {
                // Don't capture immediately: let the user pick a region.
                // The actual screencopy starts in respond_region once the
                // user confirms (Enter); Esc cancels silently.
                if self.screenshot_capture.is_some() || self.region_picker_open {
                    return;
                }
                self.open_region_picker_local(qh);
            }
            ClickAction::ToggleSettings => {
                self.launcher_settings_open = true;
                self.toggle_launcher();
            }
        }
    }

    pub(crate) fn handle_workspace_click(&mut self, qh: &QueueHandle<Self>, action: ClickAction) {
        if let ClickAction::SwitchWorkspace(workspace) = action {
            if self.active_workspace != workspace {
                debug!(
                    "active workspace changed: old={} new={} (workspace popup click)",
                    self.active_workspace, workspace
                );
                self.workspace_indicator_dirty = true;
            }
            self.active_workspace = workspace;
            self.ipc.send(&ShellCommand::SwitchWorkspace { workspace });
            self.close_workspace_popup(CommitReason::Input);
            self.draw_panel(qh, RepaintReason::Pointer);
        }
    }

    pub(crate) fn handle_status_notifier_secondary_activate(&mut self, idx: usize) {
        if let Some(item) = self.status_notifier_items.get(idx) {
            let point = panel_global_activation_point(
                self.pointer_position,
                self.panel_output_height_fallback(),
            );
            tracing::info!(
                idx,
                service = %item.service,
                x = point.x,
                y = point.y,
                "status-notifier: panel item secondary-clicked"
            );
            status_notifier::secondary_activate_item(item.clone(), point);
        }
    }

    pub(crate) fn handle_status_notifier_context_menu(&mut self, idx: usize) {
        if let Some(item) = self.status_notifier_items.get(idx) {
            let point = panel_global_activation_point(
                self.pointer_position,
                self.panel_output_height_fallback(),
            );
            tracing::info!(
                idx,
                service = %item.service,
                x = point.x,
                y = point.y,
                "status-notifier: panel item context-menu requested"
            );
            status_notifier::context_menu_item(
                item.clone(),
                point,
                self.status_notifier_tx.clone(),
            );
        }
    }

    fn panel_output_height_fallback(&self) -> Option<i32> {
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

    pub(crate) fn handle_launcher_click(&mut self, qh: &QueueHandle<Self>, action: ClickAction) {
        match action {
            ClickAction::LaunchPinnedApp(_) => {}
            ClickAction::FocusWindow(_) => {}
            ClickAction::SwitchWorkspace(_) => {}
            ClickAction::ToggleLauncher => {}
            ClickAction::ToggleWorkspacePopup => {}
            ClickAction::ToggleNetworkPopup => {}
            ClickAction::ToggleAudioPopup => {}
            ClickAction::OpenSoundSettings => {}
            ClickAction::OpenNetworkSettings => {}
            ClickAction::ActivateStatusNotifierItem(_) => {}
            ClickAction::CloseStatusNotifierMenu => {}
            ClickAction::Clock => {}
            ClickAction::TakeScreenshot => {}
            ClickAction::CyclePowerProfile => {}
            ClickAction::ToggleSettings => {
                self.launcher_settings_open = true;
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
        }
    }

    fn update_occupied_workspaces(&mut self) {
        let next = compute_occupied_workspaces(&self.workspace_window_counts);
        if next != self.occupied_workspaces {
            self.occupied_workspaces = next;
            self.workspace_indicator_dirty = true;
            if self.workspace_popup_open {
                self.workspace_dirty = true;
            }
            debug!(
                "occupied workspaces recalculated from snapshot: {:?}",
                self.occupied_workspaces
            );
        }
    }

    fn maybe_log_repaint_stats(&mut self, now: Instant) {
        if !self.repaint_stats_enabled {
            return;
        }
        if now.duration_since(self.last_repaint_stats_log) < Duration::from_secs(1) {
            return;
        }
        self.last_repaint_stats_log = now;
        if !self.repaint_stats.has_activity() {
            return;
        }
        info!(
            "shell repaint summary: panel_draws={} launcher_draws={} panel(ipc={} clock={} layer={} pointer={} keyboard={} frame={} other={}) launcher(ipc={} layer={} pointer={} keyboard={} toggle={} frame={} other={})",
            self.repaint_stats.panel_draws,
            self.repaint_stats.launcher_draws,
            self.repaint_stats.panel_ipc,
            self.repaint_stats.panel_clock,
            self.repaint_stats.panel_layer_configure,
            self.repaint_stats.panel_pointer,
            self.repaint_stats.panel_keyboard,
            self.repaint_stats.panel_compositor_frame,
            self.repaint_stats.panel_other,
            self.repaint_stats.launcher_ipc,
            self.repaint_stats.launcher_layer_configure,
            self.repaint_stats.launcher_pointer,
            self.repaint_stats.launcher_keyboard,
            self.repaint_stats.launcher_toggle,
            self.repaint_stats.launcher_compositor_frame,
            self.repaint_stats.launcher_other
        );
        self.repaint_stats.reset();
    }

    fn maybe_log_commit_stats(&mut self, now: Instant) {
        if !self.commit_stats_enabled {
            return;
        }
        if now.duration_since(self.last_commit_stats_log) < Duration::from_secs(1) {
            return;
        }
        self.last_commit_stats_log = now;
        if !self.commit_stats.has_activity() {
            return;
        }
        info!(
            "shell commit summary: total={} panel(initial_create={} configure_ack={} draw_panel={} draw_launcher={} event_loop_tick={} input={} other={}) launcher(initial_create={} configure_ack={} draw_panel={} draw_launcher={} event_loop_tick={} input={} other={})",
            self.commit_stats.total(),
            self.commit_stats.panel.initial_create,
            self.commit_stats.panel.configure_ack,
            self.commit_stats.panel.draw_panel,
            self.commit_stats.panel.draw_launcher,
            self.commit_stats.panel.event_loop_tick,
            self.commit_stats.panel.input,
            self.commit_stats.panel.unknown_other,
            self.commit_stats.launcher.initial_create,
            self.commit_stats.launcher.configure_ack,
            self.commit_stats.launcher.draw_panel,
            self.commit_stats.launcher.draw_launcher,
            self.commit_stats.launcher.event_loop_tick,
            self.commit_stats.launcher.input,
            self.commit_stats.launcher.unknown_other
        );
        self.commit_stats.reset();
    }

    fn maybe_log_render_stats(&mut self, now: Instant) {
        if !self.render_stats_enabled {
            return;
        }
        if now.duration_since(self.last_render_stats_log) < Duration::from_secs(1) {
            return;
        }
        self.last_render_stats_log = now;
        if !self.render_stats.has_activity() {
            return;
        }
        info!(
            "shell render summary: panel(renders={} skips={} commits={}) launcher(renders={} skips={} commits={})",
            self.render_stats.panel.renders,
            self.render_stats.panel.skips,
            self.render_stats.panel.commits,
            self.render_stats.launcher.renders,
            self.render_stats.launcher.skips,
            self.render_stats.launcher.commits
        );
        self.render_stats.reset();
    }
}

pub(crate) fn load_hidden_apps() -> std::collections::HashSet<String> {
    let path = hidden_apps_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string())
            .collect(),
        Err(_) => std::collections::HashSet::new(),
    }
}
