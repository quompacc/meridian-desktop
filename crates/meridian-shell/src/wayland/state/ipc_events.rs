impl MeridianShell {
    fn apply_ipc_event(&mut self, event: ShellEvent) {
        match event {
            ShellEvent::WorkspaceChanged { workspace } => {
                let old = self.active_workspace;
                if !self.output_workspace_state_available {
                    debug!("legacy workspace fallback used: workspace={}", workspace);
                }
                apply_workspace_changed(&mut self.active_workspace, workspace);
                let next = self.active_workspace;
                debug!("workspace state received: active_workspace={}", next);
                if old != next {
                    debug!("active workspace changed: old={} new={}", old, next);
                    self.workspace_indicator_dirty = true;
                }
                self.workspace_state_received = true;
                self.workspace_ipc_unavailable_logged = false;
            }
            ShellEvent::WindowSnapshot {
                active_workspace,
                windows,
            } => {
                let old = self.active_workspace;
                debug!(
                    "full window snapshot received: active_workspace={} windows={}",
                    active_workspace,
                    windows.len()
                );
                apply_full_window_snapshot(
                    &mut self.active_workspace,
                    &mut self.windows,
                    &mut self.workspace_window_counts,
                    active_workspace,
                    windows,
                );
                if old != self.active_workspace {
                    debug!(
                        "active workspace changed: old={} new={}",
                        old, self.active_workspace
                    );
                    self.workspace_indicator_dirty = true;
                }
                self.workspace_state_received = true;
                self.workspace_ipc_unavailable_logged = false;
                self.occupied_state_available = true;
                self.occupied_unavailable_logged = false;
                self.update_occupied_workspaces();
                clear_stale_focused_window_id(&mut self.focused_window_id, &self.windows);
                self.update_focused_title();
            }
            ShellEvent::OutputWorkspaceChanged {
                output_id,
                output_name,
                workspace,
                focused,
            } => {
                debug!(
                    "output workspace changed received: output_id={} output_name={:?} workspace={} focused={}",
                    output_id, output_name, workspace, focused
                );
                apply_output_workspace_changed_state(
                    &mut self.focused_output_id,
                    &mut self.output_workspaces,
                    &mut self.output_workspace_state_available,
                    &mut self.workspace_indicator_dirty,
                    OutputWorkspaceChangedInput {
                        output_id,
                        output_name,
                        workspace,
                        focused,
                    },
                );
                debug!(
                    "output workspace state available: focused_output_id={:?} outputs={}",
                    self.focused_output_id,
                    self.output_workspaces.len()
                );
                self.workspace_state_received = true;
                self.workspace_ipc_unavailable_logged = false;
            }
            ShellEvent::OutputWorkspaceSnapshot {
                focused_output_id,
                outputs,
            } => {
                debug!(
                    "output workspace snapshot received: focused_output_id={:?} outputs={}",
                    focused_output_id,
                    outputs.len()
                );
                apply_output_workspace_snapshot_state(
                    &mut self.focused_output_id,
                    &mut self.output_workspaces,
                    &mut self.output_workspace_state_available,
                    &mut self.workspace_indicator_dirty,
                    focused_output_id,
                    outputs,
                );
                debug!(
                    "output workspace state available: focused_output_id={:?} outputs={}",
                    self.focused_output_id,
                    self.output_workspaces.len()
                );
                self.workspace_state_received = true;
                self.workspace_ipc_unavailable_logged = false;
            }
            ShellEvent::WindowOpened { id, title } => {
                apply_window_opened_state(
                    &mut self.windows,
                    id,
                    title,
                    Some(self.active_workspace),
                );
                self.update_focused_title();
            }
            ShellEvent::WindowClosed { id } => {
                apply_window_closed_state(&mut self.windows, &id);
                clear_stale_focused_window_id(&mut self.focused_window_id, &self.windows);
                self.update_focused_title();
            }
            ShellEvent::WindowFocused { id } => {
                if !self.desktop_menu_in_open_debounce() {
                    self.close_desktop_context_menu_from_ipc();
                }
                self.focused_window_id = Some(id);
                self.update_focused_title();
            }
            ShellEvent::WindowFocusCleared => {
                if !self.desktop_menu_in_open_debounce() {
                    self.close_desktop_context_menu_from_ipc();
                }
                self.focused_window_id = None;
                self.update_focused_title();
            }
            ShellEvent::ConfigReloaded { success } => {
                debug!("ConfigReloaded {{ success: {} }}", success);
                self.handle_config_reloaded(success);
            }
            ShellEvent::ToggleLauncher => {
                self.toggle_launcher();
            }
            ShellEvent::ToggleQuickSettings => {
                self.toggle_web_quick_settings();
            }
            ShellEvent::OpenSystemSettings => {
                self.open_web_system_settings();
            }
            ShellEvent::AppearanceRefresh => {
                self.refresh_web_appearance();
            }
            ShellEvent::AppearanceThemeSet { theme } => {
                let name = theme.config_name();
                if self.theme_name != name {
                    meridian_config::MeridianConfig::save_theme(name);
                    self.ipc.send(&meridian_ipc::ShellCommand::ReloadConfig);
                } else {
                    self.refresh_web_appearance();
                }
            }
            ShellEvent::AppearanceWallpaperSet { path } => {
                if std::path::Path::new(&path).is_file() {
                    let mode = self.wallpaper_mode;
                    meridian_config::MeridianConfig::save_wallpaper(&path, mode);
                    self.wallpaper_path = Some(path);
                    self.ipc.send(&meridian_ipc::ShellCommand::ReloadConfig);
                    self.refresh_web_appearance();
                } else {
                    tracing::warn!("appearance rejected missing wallpaper file");
                }
            }
            ShellEvent::AppearanceWallpaperModeSet { mode } => {
                let mode = match mode {
                    meridian_ipc::AppearanceWallpaperMode::Fill => {
                        meridian_config::WallpaperMode::Fill
                    }
                    meridian_ipc::AppearanceWallpaperMode::Fit => {
                        meridian_config::WallpaperMode::Fit
                    }
                    meridian_ipc::AppearanceWallpaperMode::Center => {
                        meridian_config::WallpaperMode::Center
                    }
                    meridian_ipc::AppearanceWallpaperMode::Tile => {
                        meridian_config::WallpaperMode::Tile
                    }
                };
                let effective_path = self.wallpaper_path.clone().or_else(|| {
                    self.theme
                        .wallpaper
                        .as_ref()
                        .map(|wallpaper| wallpaper.path.clone())
                });
                if let Some(path) = effective_path {
                    self.wallpaper_path = Some(path.clone());
                    self.wallpaper_mode = mode;
                    meridian_config::MeridianConfig::save_wallpaper(&path, mode);
                    self.ipc.send(&meridian_ipc::ShellCommand::ReloadConfig);
                    self.refresh_web_appearance();
                }
            }
            ShellEvent::QuickSettingsNetworkRefresh => {
                self.refresh_quick_settings_network();
                self.refresh_web_quick_settings();
            }
            ShellEvent::QuickSettingsNetworkConnect { ssid, password } => {
                let Some(network) = self
                    .wifi_networks
                    .iter()
                    .find(|network| network.ssid == ssid)
                else {
                    tracing::warn!("Quick Settings rejected stale Wi-Fi selection");
                    return;
                };
                let known = self
                    .network_profiles
                    .iter()
                    .any(|profile| profile.name == ssid);
                if network.secured && !known && password.is_none() {
                    tracing::warn!("Quick Settings rejected secured Wi-Fi without credentials");
                    return;
                }
                crate::network::connect_wifi(
                    &ssid,
                    if network.secured && !known {
                        password.as_deref()
                    } else {
                        None
                    },
                );
            }
            ShellEvent::QuickSettingsNetworkDisconnect => {
                if let crate::network::NetworkState::Connected {
                    kind: crate::network::ConnectionKind::Wifi { .. },
                    connection_name,
                } = self.network_controller.state()
                {
                    crate::network::disconnect_connection(connection_name);
                }
            }
            ShellEvent::AudioVolumeStep { delta } => {
                let current = self
                    .audio_snapshot
                    .default_output
                    .as_ref()
                    .and_then(|device| device.volume_percent)
                    .unwrap_or(50) as i16;
                let next = (current + delta as i16).clamp(0, 100) as u8;
                crate::audio::set_default_sink_volume(next);
                self.audio_snapshot = crate::audio::AudioSnapshot::poll();
                self.panel_dirty = true;
                self.volume_osd_pending = true;
                self.refresh_web_quick_settings();
            }
            ShellEvent::AudioVolumeSet { percent } => {
                crate::audio::set_default_sink_volume(percent.min(100));
                self.audio_snapshot = crate::audio::AudioSnapshot::poll();
                self.panel_dirty = true;
                self.refresh_web_quick_settings();
            }
            ShellEvent::AudioMuteToggle => {
                crate::audio::toggle_default_sink_mute();
                self.audio_snapshot = crate::audio::AudioSnapshot::poll();
                self.panel_dirty = true;
                self.volume_osd_pending = true;
                self.refresh_web_quick_settings();
            }
            ShellEvent::QuickSettingsAudioMuteToggle => {
                crate::audio::toggle_default_sink_mute();
                self.audio_snapshot = crate::audio::AudioSnapshot::poll();
                self.panel_dirty = true;
                self.refresh_web_quick_settings();
            }
            ShellEvent::PowerProfileSet { profile } => {
                use crate::power_profile::{self, PowerProfile};
                let profile = match profile {
                    meridian_ipc::QuickSettingsPowerProfile::Eco => PowerProfile::Eco,
                    meridian_ipc::QuickSettingsPowerProfile::Standard => PowerProfile::Standard,
                    meridian_ipc::QuickSettingsPowerProfile::Performance => {
                        PowerProfile::Performance
                    }
                };
                if power_profile::set(profile) {
                    self.power_profile = Some(profile);
                } else {
                    self.power_profile = power_profile::current();
                }
                self.panel_dirty = true;
                self.refresh_web_quick_settings();
            }
            ShellEvent::DesktopContextMenu { x, y } => {
                self.open_desktop_context_menu_from_ipc(x, y);
            }
            ShellEvent::WindowThumbnail {
                id,
                path,
                width,
                height,
            } => match std::fs::read(&path) {
                Ok(data) => {
                    let _ = std::fs::remove_file(&path);
                    tracing::debug!(
                        "thumbnail received: id={} {}x{} bytes={}",
                        id,
                        width,
                        height,
                        data.len()
                    );
                    self.thumbnail_cache.insert(id, (width, height, data));
                    if self.thumbnail_popup_open {
                        self.thumbnail_dirty = true;
                    }
                }
                Err(e) => {
                    tracing::warn!("thumbnail: read failed {}: {}", path, e);
                }
            },
            ShellEvent::ScreenshotConsentRequest { request_id, app_id } => {
                self.open_consent_modal(request_id, app_id);
            }
            ShellEvent::ScreenshotRegionRequest { request_id, app_id } => {
                self.open_region_picker(request_id, app_id);
            }
        }
    }

    fn handle_config_reloaded(&mut self, success: bool) {
        debug!("shell config reload requested");
        if !success {
            tracing::warn!("shell config reload failed; keeping previous config");
            return;
        }

        let mut config = MeridianConfig::default();
        if let Err(err) = config.reload() {
            tracing::warn!(
                "shell config reload failed; keeping previous config: {}",
                err
            );
            return;
        }

        match resolve_shell_theme_from_config(&config) {
            Ok((theme_name, new_theme, available_themes)) => {
                let old_sig = panel_theme_signature(&self.theme);
                let new_sig = panel_theme_signature(&new_theme);
                let theme_changed = old_sig != new_sig || self.theme_name != theme_name;

                if theme_changed {
                    debug!(
                        "shell theme changed: old={} new={}",
                        self.theme_name, theme_name
                    );
                }

                let font_changed = self.theme.fonts.ui != new_theme.fonts.ui;
                self.theme_name = theme_name;
                self.theme = new_theme;
                self.available_themes = available_themes;
                self.wallpaper_path = config.wallpaper.as_ref().map(|wallpaper| wallpaper.path.clone());
                self.wallpaper_mode = config
                    .wallpaper
                    .as_ref()
                    .map(|wallpaper| wallpaper.mode)
                    .or_else(|| self.theme.wallpaper.as_ref().map(|wallpaper| wallpaper.mode))
                    .unwrap_or_default();

                if font_changed {
                    crate::font_resolve::apply_theme_ui_font(&self.theme);
                }

                // LAUNCH-2 regression guard: scanning every applications dir is
                // hundreds of fs reads + TryExec stats; running it synchronously
                // here froze the shell on every theme switch / config reload.
                // Reuse the off-thread rescan — tick() swaps the fresh list in
                // when the worker finishes (P1-2, AUDIT_2026-08-19).
                self.request_launcher_apps_refresh();
                // The icon cache rebuild stays synchronous by design: a theme
                // change must recolour the symbolic panel/tray icons before the
                // next panel frame, and this warm set (~40 icons) is far smaller
                // than the launcher-grid decode that LAUNCH-3 offloaded.
                self.icon_cache =
                    super::init::assets::build_icon_cache(&self.theme, &self.pinned_apps);
                self.launcher_icons_warmed = false;
                self.panel_dirty = true;
                self.launcher_dirty = true;
                self.calendar_dirty = true;
                self.workspace_dirty = true;
                self.network_dirty = true;
                self.audio_dirty = true;
                self.thumbnail_dirty |= self.thumbnail_popup_open;
                if theme_changed {
                    self.sync_web_theme();
                }
                self.refresh_web_appearance();
                debug!("shell config reload succeeded");
            }
            Err(err) => {
                tracing::warn!(
                    "shell config reload failed; keeping previous config: {}",
                    err
                );
            }
        }
    }

    fn update_focused_title(&mut self) {
        self.focused_title = self
            .focused_window_id
            .as_deref()
            .and_then(|id| self.windows.iter().find(|w| w.id == id))
            .map(|w| w.title.clone());
    }

    /// Warm the launcher grid's app icons (deferred from startup so the panel
    /// appears immediately). Runs once per cache build; the first launcher open
    /// pays the decode cost instead of every login.
    /// Kick an OFF-THREAD warm of the launcher grid icons (LAUNCH-3). Decoding
    /// every app icon at two sizes on the event-loop thread froze the launcher
    /// for ~0.5s on each open (and again after every background app refresh),
    /// which showed up as input lag / bursty scrolling. The worker decodes with
    /// a throwaway loader and posts ready buffers to `launcher_icons_rx`, which
    /// `tick()` drains via `poll_launcher_icons_warm`. No-op if already warmed
    /// or a warm is already in flight.
    fn warm_launcher_icons(&mut self) {
        if self.launcher_icons_warmed || self.launcher_icons_rx.is_some() {
            return;
        }
        let names: Vec<String> = self
            .launcher_state
            .apps
            .iter()
            .filter_map(|app| app.icon_name.clone())
            .filter(|name| !name.is_empty())
            .collect();
        if names.is_empty() {
            self.launcher_icons_warmed = true;
            return;
        }
        let (theme_name, symbolic_color) = self.icon_cache.loader_config();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let batch = crate::icons::IconCache::load_batch(
                &theme_name,
                &symbolic_color,
                &names,
                &[22, 24],
            );
            let _ = tx.send(batch);
        });
        self.launcher_icons_rx = Some(rx);
        // Mark warmed now so re-opens before results arrive don't re-spawn; the
        // results are applied when the worker finishes.
        self.launcher_icons_warmed = true;
    }

    /// Apply a finished off-thread icon warm, if one has arrived. Called from
    /// `tick()`; cheap `try_recv`, never blocks. Redraws the launcher so the
    /// freshly-decoded icons appear the moment they land.
    fn poll_launcher_icons_warm(&mut self, qh: &QueueHandle<Self>) {
        let Some(rx) = self.launcher_icons_rx.as_ref() else {
            return;
        };
        match rx.try_recv() {
            Ok(batch) => {
                self.launcher_icons_rx = None;
                for (name, size, image) in batch {
                    self.icon_cache.insert_loaded(name, size, image);
                }
                if self.launcher_state.open {
                    self.draw_launcher(qh, RepaintReason::Ipc);
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.launcher_icons_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }

    /// Kick a background rescan of the desktop-entry app list (LAUNCH-2).
    /// Scanning every applications dir is hundreds of fs reads + TryExec stats;
    /// doing it on the event-loop thread froze the UI on every launcher open.
    /// The worker thread posts the fresh list to `launcher_apps_rx`, which
    /// `tick()` swaps in. No-op if a rescan is already in flight.
    pub(crate) fn request_launcher_apps_refresh(&mut self) {
        if self.launcher_apps_rx.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::launcher::DesktopApp::load_system());
        });
        self.launcher_apps_rx = Some(rx);
    }

    /// Apply a finished background app-list rescan, if one has arrived. Called
    /// from `tick()`; cheap `try_recv`, never blocks.
    fn poll_launcher_apps_refresh(&mut self, qh: &QueueHandle<Self>) {
        let Some(rx) = self.launcher_apps_rx.as_ref() else {
            return;
        };
        match rx.try_recv() {
            Ok(apps) => {
                self.launcher_apps_rx = None;
                self.launcher_state.apps = apps;
                // New app list → the old warm is stale. Cancel any in-flight
                // warm and re-request so the fresh apps get their icons.
                self.launcher_icons_warmed = false;
                self.launcher_icons_rx = None;
                if self.launcher_state.open {
                    self.warm_launcher_icons();
                    self.draw_launcher(qh, RepaintReason::Ipc);
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.launcher_apps_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
}
