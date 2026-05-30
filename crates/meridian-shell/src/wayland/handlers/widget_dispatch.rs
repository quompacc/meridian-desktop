use wayland_client::QueueHandle;

use super::MeridianShell;
use crate::{
    context_menu::{ContextMenuAction, ContextMenuState, DesktopContextMenuAction},
    panel::PinnedApp,
    wayland::{CommitReason, RepaintReason},
    widget_action::WidgetAction,
};

impl MeridianShell {
    pub(super) fn dispatch_widget_action(
        &mut self,
        qh: &QueueHandle<MeridianShell>,
        action: WidgetAction,
    ) {
        // Cancel an armed power button on any non-power action — the user
        // changed their mind by clicking somewhere else. Power actions are
        // skipped here; their own handler arms or consumes the armed state.
        let is_power = matches!(
            action,
            WidgetAction::PowerOff
                | WidgetAction::PowerRestart
                | WidgetAction::PowerSleep
                | WidgetAction::PowerLock
                | WidgetAction::PowerLogout
        );
        if !is_power && self.armed_power.is_some() {
            self.armed_power = None;
            self.draw_launcher(qh, RepaintReason::Pointer);
        }

        match action {
            WidgetAction::LaunchApp { .. } | WidgetAction::LaunchExec(_) => {
                self.dispatch_launch_action(qh, action);
            }
            WidgetAction::ToggleCalendar
            | WidgetAction::ToggleNetworkPopup
            | WidgetAction::ToggleWorkspacePopup => self.dispatch_popup_action(qh, action),
            WidgetAction::ToggleSettings
            | WidgetAction::SetSettingsCategory(_)
            | WidgetAction::ApplyThemeByIndex(_)
            | WidgetAction::ApplyWallpaperByIndex(_)
            | WidgetAction::SetWallpaperMode(_)
            | WidgetAction::SetCursorSize(_)
            | WidgetAction::ApplyCursorThemeByIndex(_)
            | WidgetAction::SetIdleTimeout(_)
            | WidgetAction::SetDefaultSinkVolume(_)
            | WidgetAction::ToggleDefaultSinkMute
            | WidgetAction::SetDefaultAudioOutput(_)
            | WidgetAction::SetDefaultAudioInput(_)
            | WidgetAction::ActivateConnection(_)
            | WidgetAction::WifiConnect(_)
            | WidgetAction::ToggleBluetoothPower
            | WidgetAction::ToggleBluetoothScan
            | WidgetAction::BluetoothDevice(_)
            | WidgetAction::BrowseWallpaper
            | WidgetAction::SetPrimaryOutput(_)
            | WidgetAction::CycleOutputScale(_)
            | WidgetAction::CycleOutputTransform(_)
            | WidgetAction::ToggleOutputModeDropdown(_)
            | WidgetAction::SetOutputMode { .. } => self.dispatch_settings_action(qh, action),
            WidgetAction::PowerOff
            | WidgetAction::PowerRestart
            | WidgetAction::PowerSleep
            | WidgetAction::PowerLock
            | WidgetAction::PowerLogout => self.dispatch_power_action(qh, action),
            WidgetAction::PinnedMoveUp(_)
            | WidgetAction::PinnedMoveDown(_)
            | WidgetAction::PinnedRemove(_)
            | WidgetAction::PinnedOpenAdd
            | WidgetAction::PinnedCloseAdd
            | WidgetAction::PinnedAddApp(_) => self.dispatch_pinned_action(qh, action),
        }
    }

    fn dispatch_launch_action(&mut self, qh: &QueueHandle<MeridianShell>, action: WidgetAction) {
        match action {
            WidgetAction::LaunchApp { program, args } => {
                if let Err(err) = std::process::Command::new(&program).args(&args).spawn() {
                    tracing::warn!("launch failed: {:?}", err);
                }
            }
            WidgetAction::LaunchExec(exec) => {
                if let Err(err) = std::process::Command::new(&exec).spawn() {
                    tracing::warn!("launch failed: {:?}", err);
                }
            }
            _ => unreachable!("non launch action routed to launch dispatcher"),
        }
        // Mirror handle_launcher_click's behavior: any launch via a widget
        // click should dismiss the launcher so the new window is not occluded.
        // No-op when launcher is closed (close_launcher_after_launch checks).
        self.close_launcher_after_launch(qh, RepaintReason::Pointer);
    }

    fn dispatch_popup_action(&mut self, qh: &QueueHandle<MeridianShell>, action: WidgetAction) {
        match action {
            WidgetAction::ToggleCalendar => {
                self.toggle_calendar_popup(CommitReason::Input);
                self.draw_panel(qh, RepaintReason::Pointer);
                if self.calendar_popup_open {
                    self.draw_calendar_popup(qh, RepaintReason::Pointer);
                }
            }
            WidgetAction::ToggleNetworkPopup => {
                self.toggle_network_popup(CommitReason::Input);
                self.draw_panel(qh, RepaintReason::Pointer);
                if self.network_popup_open {
                    self.draw_network_popup(qh, RepaintReason::Pointer);
                }
            }
            WidgetAction::ToggleWorkspacePopup => {
                self.toggle_workspace_popup(CommitReason::Input);
                self.draw_panel(qh, RepaintReason::Pointer);
                if self.workspace_popup_open {
                    self.draw_workspace_popup(qh, RepaintReason::Pointer);
                }
            }
            _ => unreachable!("non popup action routed to popup dispatcher"),
        }
    }

    fn dispatch_settings_action(&mut self, qh: &QueueHandle<MeridianShell>, action: WidgetAction) {
        match action {
            WidgetAction::ToggleSettings => {
                self.launcher_settings_open = !self.launcher_settings_open;
                if self.launcher_settings_open {
                    // entering settings — start with an empty search
                    self.settings_search.clear();
                } else {
                    // returning to the command palette
                    self.ui_preview_widget_state = None;
                }
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::SetSettingsCategory(cat) => {
                self.settings_category = cat;
                self.display_mode_dropdown_open = None;
                if cat == crate::settings_view::SettingsCategory::Wallpaper
                    && self.wallpaper_thumbnails.is_empty()
                {
                    self.load_wallpaper_thumbnails();
                }
                if cat == crate::settings_view::SettingsCategory::Printers {
                    self.printer_snapshot = crate::printers::PrinterSnapshot::poll();
                }
                if cat == crate::settings_view::SettingsCategory::Sound {
                    self.audio_snapshot = crate::audio::AudioSnapshot::poll();
                }
                if cat == crate::settings_view::SettingsCategory::Network {
                    // Read-only listings; safe on the event loop (fast nmcli
                    // queries: saved-connection list + cached wifi scan).
                    self.network_profiles = crate::network::list_saved_connections();
                    self.wifi_networks = crate::network::scan_wifi_networks();
                    self.wifi_password_prompt = None;
                    self.wifi_password_input.clear();
                }
                if cat == crate::settings_view::SettingsCategory::Bluetooth {
                    // Read-only bluetoothctl snapshot; cheap, safe on the loop.
                    self.bluetooth_snapshot = crate::bluetooth::BluetoothSnapshot::poll();
                }
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::ApplyThemeByIndex(idx) => {
                if let Some(name) = self.available_themes.get(idx).cloned() {
                    self.apply_theme(qh, name);
                }
            }
            WidgetAction::ApplyWallpaperByIndex(idx) => {
                if let Some(entry) = self.available_wallpapers.get(idx) {
                    let path = entry.apply_path.clone();
                    let mode = self.wallpaper_mode;
                    self.apply_wallpaper(qh, path, mode);
                }
            }
            WidgetAction::SetWallpaperMode(mode) => {
                self.wallpaper_mode = mode;
                if let Some(path) = self.wallpaper_path.clone() {
                    self.apply_wallpaper(qh, path, mode);
                } else {
                    self.draw_launcher(qh, RepaintReason::Pointer);
                }
            }
            WidgetAction::SetCursorSize(size) => {
                if self.cursor_size != size {
                    self.cursor_size = size;
                    // Persist alongside the current theme, then ask the
                    // compositor to reload so the live cursor updates at once.
                    meridian_config::MeridianConfig::save_cursor(&self.cursor_theme, size);
                    self.ipc.send(&meridian_ipc::ShellCommand::ReloadConfig);
                }
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::ApplyCursorThemeByIndex(idx) => {
                if let Some(name) = self.available_cursor_themes.get(idx).cloned() {
                    if self.cursor_theme != name {
                        self.cursor_theme = name.clone();
                        // Persist alongside the current size, then reload so the
                        // compositor swaps the live cursor theme at once.
                        meridian_config::MeridianConfig::save_cursor(&name, self.cursor_size);
                        self.ipc.send(&meridian_ipc::ShellCommand::ReloadConfig);
                    }
                }
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::SetIdleTimeout(secs) => {
                if self.idle_timeout_secs != secs {
                    self.idle_timeout_secs = secs;
                    // Persist to [general] and reload so the compositor picks up
                    // the new idle blanking timeout (or disables it) at once.
                    meridian_config::MeridianConfig::save_idle_timeout(secs);
                    self.ipc.send(&meridian_ipc::ShellCommand::ReloadConfig);
                }
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::SetDefaultSinkVolume(percent) => {
                // System state, not Meridian config: drive wpctl directly, then
                // re-poll so the page reflects the real new level.
                crate::audio::set_default_sink_volume(percent);
                self.audio_snapshot = crate::audio::AudioSnapshot::poll();
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::ToggleDefaultSinkMute => {
                crate::audio::toggle_default_sink_mute();
                self.audio_snapshot = crate::audio::AudioSnapshot::poll();
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::SetDefaultAudioOutput(idx) => {
                // Look up the wpctl id by position in the current snapshot, make
                // it the default sink, then re-poll so the page reflects it.
                if let Some(device) = self.audio_snapshot.outputs.get(idx) {
                    crate::audio::set_default_device(device.id);
                    self.audio_snapshot = crate::audio::AudioSnapshot::poll();
                }
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::SetDefaultAudioInput(idx) => {
                if let Some(device) = self.audio_snapshot.inputs.get(idx) {
                    crate::audio::set_default_device(device.id);
                    self.audio_snapshot = crate::audio::AudioSnapshot::poll();
                }
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::ActivateConnection(idx) => {
                // Bringing a link up can block for seconds (DHCP/auth), so the
                // activation runs on a background thread inside
                // activate_connection — never on the event loop. Optimistically
                // mark the chosen profile active so the click gives feedback;
                // re-entering the Network page re-lists the real state. We only
                // flip the clicked row (not the others), so a still-active VPN
                // is never falsely hidden if activation is slow or fails.
                if let Some(profile) = self.network_profiles.get_mut(idx) {
                    let name = profile.name.clone();
                    profile.active = true;
                    crate::network::activate_connection(&name);
                }
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::WifiConnect(idx) => {
                if let Some(net) = self.wifi_networks.get(idx) {
                    let ssid = net.ssid.clone();
                    // A secured network with no matching saved profile needs a
                    // password: open the prompt and capture keys there. Open or
                    // already-known networks connect straight away (off-thread).
                    let known = self.network_profiles.iter().any(|p| p.name == ssid);
                    if net.secured && !known {
                        self.wifi_password_prompt = Some(ssid);
                        self.wifi_password_input.clear();
                    } else {
                        crate::network::connect_wifi(&ssid, None);
                    }
                }
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::ToggleBluetoothPower => {
                // Drive bluetoothctl off-thread, then optimistically flip the
                // cached flag so the toggle gives feedback; re-entering the page
                // re-polls the real state.
                let turn_on = !self.bluetooth_snapshot.powered;
                crate::bluetooth::set_power(turn_on);
                self.bluetooth_snapshot.powered = turn_on;
                if !turn_on {
                    self.bluetooth_snapshot.scanning = false;
                }
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::ToggleBluetoothScan => {
                // One-shot timed discovery (off-thread). Optimistically show the
                // scanning state; re-entering the page re-polls real discovery.
                crate::bluetooth::start_scan();
                self.bluetooth_snapshot.scanning = true;
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::BluetoothDevice(idx) => {
                // Already-paired device connects; an unknown device pairs (which
                // also trusts + connects). Both block on a remote handshake, so
                // they run off-thread inside the bluetooth helpers.
                if let Some(dev) = self.bluetooth_snapshot.devices.get(idx) {
                    if dev.paired {
                        crate::bluetooth::connect_device(&dev.address);
                    } else {
                        crate::bluetooth::pair_device(&dev.address);
                    }
                }
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::BrowseWallpaper => {
                // Close launcher so the file dialog opens in the foreground.
                self.close_launcher_after_launch(qh, RepaintReason::Pointer);
                self.spawn_file_picker();
            }
            WidgetAction::SetPrimaryOutput(idx) => {
                if let Some(output) = self.output_workspaces.get(idx).cloned() {
                    let Some(name) = output.output_name else {
                        tracing::warn!(
                            "cannot set primary output without output name: output_id={}",
                            output.output_id
                        );
                        return;
                    };
                    meridian_config::MeridianConfig::save_primary_output(&name);
                    for state in &mut self.output_workspaces {
                        state.primary = state.output_name.as_deref() == Some(name.as_str());
                    }
                    self.ipc.send(&meridian_ipc::ShellCommand::ReloadConfig);
                    self.draw_launcher(qh, RepaintReason::Pointer);
                }
            }
            WidgetAction::CycleOutputScale(idx) => {
                if let Some(output) = self.output_workspaces.get(idx).cloned() {
                    let Some(name) = output.output_name else {
                        return;
                    };
                    // Advance to the next scale in the cycle (wrap around).
                    let cur = output.scale_millis as f64 / 1000.0;
                    let cycle = crate::settings_view::DISPLAY_SCALE_CYCLE;
                    let pos = cycle
                        .iter()
                        .position(|s| (s - cur).abs() < 0.001)
                        .unwrap_or(usize::MAX);
                    let next = cycle[pos.wrapping_add(1) % cycle.len()];
                    meridian_config::MeridianConfig::save_output_scale(&name, next);
                    if let Some(state) = self.output_workspaces.get_mut(idx) {
                        state.scale_millis = (next * 1000.0).round() as u32;
                    }
                    self.ipc.send(&meridian_ipc::ShellCommand::ReloadConfig);
                    self.draw_launcher(qh, RepaintReason::Pointer);
                }
            }
            WidgetAction::CycleOutputTransform(idx) => {
                if let Some(output) = self.output_workspaces.get(idx).cloned() {
                    let Some(name) = output.output_name else {
                        return;
                    };
                    let cur = output.transform.as_deref().unwrap_or("");
                    let cycle = crate::settings_view::DISPLAY_ROTATE_CYCLE;
                    let pos = cycle
                        .iter()
                        .position(|(v, _)| *v == cur)
                        .unwrap_or(usize::MAX);
                    let (next_val, _) = cycle[pos.wrapping_add(1) % cycle.len()];
                    let next = if next_val.is_empty() {
                        None
                    } else {
                        Some(next_val)
                    };
                    meridian_config::MeridianConfig::save_output_transform(&name, next);
                    if let Some(state) = self.output_workspaces.get_mut(idx) {
                        state.transform = next.map(|s| s.to_string());
                    }
                    self.ipc.send(&meridian_ipc::ShellCommand::ReloadConfig);
                    self.draw_launcher(qh, RepaintReason::Pointer);
                }
            }
            WidgetAction::ToggleOutputModeDropdown(idx) => {
                self.display_mode_dropdown_open = if self.display_mode_dropdown_open == Some(idx) {
                    None
                } else {
                    Some(idx)
                };
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::SetOutputMode {
                output_index,
                mode_index,
            } => {
                self.apply_output_mode_selection(qh, output_index, mode_index);
            }
            _ => unreachable!("non settings action routed to settings dispatcher"),
        }
    }

    fn apply_output_mode_selection(
        &mut self,
        qh: &QueueHandle<MeridianShell>,
        output_index: usize,
        mode_index: usize,
    ) {
        let Some(output) = self.output_workspaces.get(output_index).cloned() else {
            return;
        };
        let Some(name) = output.output_name.clone() else {
            tracing::warn!(
                "cannot set output mode without output name: output_id={}",
                output.output_id
            );
            return;
        };
        let modes: Vec<_> = output
            .modes
            .iter()
            .filter(|mode| mode.width > 0 && mode.height > 0)
            .cloned()
            .collect();
        let Some(next_mode) = modes.get(mode_index).cloned() else {
            return;
        };

        meridian_config::MeridianConfig::save_output_mode(
            &name,
            next_mode.width,
            next_mode.height,
            next_mode.refresh_millihz,
        );
        for state in &mut self.output_workspaces {
            if state.output_name.as_deref() == Some(name.as_str()) {
                state.width = next_mode.width;
                state.height = next_mode.height;
                state.refresh_millihz = next_mode.refresh_millihz;
                for mode in &mut state.modes {
                    mode.current = mode.width == next_mode.width
                        && mode.height == next_mode.height
                        && mode.refresh_millihz == next_mode.refresh_millihz;
                }
            }
        }
        self.display_mode_dropdown_open = None;
        self.ipc.send(&meridian_ipc::ShellCommand::ReloadConfig);
        self.draw_launcher(qh, RepaintReason::Pointer);
    }

    fn dispatch_power_action(&mut self, qh: &QueueHandle<MeridianShell>, action: WidgetAction) {
        let (id, command) = match action {
            WidgetAction::PowerOff => ("power-off", Some(("systemctl", "poweroff"))),
            WidgetAction::PowerRestart => ("power-restart", Some(("systemctl", "reboot"))),
            WidgetAction::PowerSleep => ("power-sleep", Some(("systemctl", "suspend"))),
            WidgetAction::PowerLock => ("power-lock", Some(("loginctl", "lock-session"))),
            WidgetAction::PowerLogout => ("power-logout", None),
            _ => unreachable!("non power action routed to power dispatcher"),
        };

        if !self.try_consume_armed_power(id) {
            self.arm_power(qh, id);
            return;
        }

        self.close_launcher_after_launch(qh, RepaintReason::Pointer);
        if let Some((program, arg)) = command {
            std::thread::spawn(move || {
                let _ = std::process::Command::new(program).arg(arg).status();
            });
            return;
        }

        tracing::info!("power: logout requested - requesting compositor quit");
        if !self.ipc.send(&meridian_ipc::ShellCommand::Quit) {
            tracing::warn!("power: logout request failed - compositor IPC unavailable");
            self.arm_power(qh, id);
        }
    }

    fn dispatch_pinned_action(&mut self, qh: &QueueHandle<MeridianShell>, action: WidgetAction) {
        match action {
            WidgetAction::PinnedMoveUp(idx) => {
                if idx > 0 && idx < self.pinned_apps.len() {
                    self.pinned_apps.swap(idx - 1, idx);
                    self.persist_pinned_apps_and_redraw(qh);
                }
            }
            WidgetAction::PinnedMoveDown(idx) => {
                if idx + 1 < self.pinned_apps.len() {
                    self.pinned_apps.swap(idx, idx + 1);
                    self.persist_pinned_apps_and_redraw(qh);
                }
            }
            WidgetAction::PinnedRemove(idx) => {
                if idx < self.pinned_apps.len() {
                    self.pinned_apps.remove(idx);
                    self.persist_pinned_apps_and_redraw(qh);
                }
            }
            WidgetAction::PinnedOpenAdd => {
                self.settings_pinned_adding = true;
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::PinnedCloseAdd => {
                self.settings_pinned_adding = false;
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            WidgetAction::PinnedAddApp(idx) => {
                self.add_pinned_app_by_addable_index(qh, idx);
            }
            _ => unreachable!("non pinned action routed to pinned dispatcher"),
        }
    }

    fn persist_pinned_apps_and_redraw(&mut self, qh: &QueueHandle<MeridianShell>) {
        self.save_pinned_apps();
        self.draw_panel(qh, RepaintReason::Pointer);
        self.draw_launcher(qh, RepaintReason::Pointer);
    }

    fn add_pinned_app_by_addable_index(&mut self, qh: &QueueHandle<MeridianShell>, idx: usize) {
        let pinned_programs: std::collections::HashSet<&str> = self
            .pinned_apps
            .iter()
            .map(|p| p.program.as_str())
            .collect();
        let mut addable: Vec<&crate::launcher::DesktopApp> = self
            .launcher_state
            .apps
            .iter()
            .filter(|a| !pinned_programs.contains(a.program.as_str()))
            .collect();
        addable.sort_by(|a, b| a.name.cmp(&b.name));

        if let Some(app) = addable.get(idx) {
            self.pinned_apps.push(crate::panel::PinnedApp {
                label: app.name.clone(),
                program: app.program.clone(),
                args: app.args.clone(),
                terminal: app.terminal,
                icon_name: app.icon_name.clone(),
            });
            self.save_pinned_apps();
            if let Some(ref icon_name) = app.icon_name {
                self.icon_cache.warm(&[icon_name.as_str()], 22);
                self.icon_cache.warm(&[icon_name.as_str()], 24);
            }
            self.settings_pinned_adding = false;
            self.draw_panel(qh, RepaintReason::Pointer);
            self.draw_launcher(qh, RepaintReason::Pointer);
        }
    }

    pub(crate) fn handle_desktop_context_menu_action(
        &mut self,
        qh: &QueueHandle<MeridianShell>,
        action: DesktopContextMenuAction,
    ) {
        match action {
            DesktopContextMenuAction::Terminal => {
                let command = meridian_ipc::ShellCommand::LaunchApp {
                    program: "sh".to_string(),
                    args: Vec::new(),
                    terminal: true,
                };
                if !self.ipc.send(&command) {
                    tracing::warn!("IPC unavailable, desktop terminal launch skipped");
                }
            }
            DesktopContextMenuAction::Launcher => {
                if !self.launcher_state.open {
                    self.handle_panel_click(qh, crate::wayland::ClickAction::ToggleLauncher);
                }
            }
            DesktopContextMenuAction::FileManager => {
                let command = meridian_ipc::ShellCommand::LaunchApp {
                    program: "nautilus".to_string(),
                    args: Vec::new(),
                    terminal: false,
                };
                if !self.ipc.send(&command) {
                    tracing::warn!("IPC unavailable, desktop file manager launch skipped");
                }
            }
            DesktopContextMenuAction::Settings => {
                self.open_settings_category(qh, crate::settings_view::SettingsCategory::Theme);
            }
            DesktopContextMenuAction::LockScreen => {
                let wayland_display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
                let xdg_runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();
                if let Err(e) = std::process::Command::new("meridian-lock")
                    .env("WAYLAND_DISPLAY", &wayland_display)
                    .env("XDG_RUNTIME_DIR", &xdg_runtime)
                    .spawn()
                {
                    tracing::warn!("failed to spawn meridian-lock: {}", e);
                }
            }
        }
    }

    pub(crate) fn handle_settings_sub_action(
        &mut self,
        qh: &QueueHandle<MeridianShell>,
        action: crate::context_menu::SettingsSubAction,
    ) {
        use crate::context_menu::SettingsSubAction;
        let cat = match action {
            SettingsSubAction::Display => crate::settings_view::SettingsCategory::Display,
            SettingsSubAction::Wallpaper => crate::settings_view::SettingsCategory::Wallpaper,
            SettingsSubAction::Theme => crate::settings_view::SettingsCategory::Theme,
            SettingsSubAction::Sound => crate::settings_view::SettingsCategory::Sound,
            SettingsSubAction::Network => crate::settings_view::SettingsCategory::Network,
            SettingsSubAction::Power => crate::settings_view::SettingsCategory::Power,
        };
        self.open_settings_category(qh, cat);
    }

    pub(crate) fn handle_context_menu_action(
        &mut self,
        qh: &QueueHandle<MeridianShell>,
        action: ContextMenuAction,
        cm: &ContextMenuState,
    ) {
        match action {
            ContextMenuAction::Launch => {
                if let Some(ref wid) = cm.running_window_id {
                    self.ipc
                        .send(&meridian_ipc::ShellCommand::FocusWindow { id: wid.clone() });
                } else {
                    let _ = std::process::Command::new(cm.exec.as_ref()).spawn();
                }
            }
            ContextMenuAction::NewWindow => {
                let _ = std::process::Command::new(cm.exec.as_ref()).spawn();
            }
            ContextMenuAction::LaunchInTerminal => {
                let _ = std::process::Command::new("kitty")
                    .args(["-e", cm.exec.as_ref()])
                    .spawn();
            }
            ContextMenuAction::PinToPanel => {
                if !self
                    .pinned_apps
                    .iter()
                    .any(|p| p.program == cm.exec.as_ref())
                {
                    let icon_name = self
                        .launcher_state
                        .apps
                        .iter()
                        .find(|a| a.program == cm.exec.as_ref())
                        .and_then(|a| a.icon_name.clone());
                    self.pinned_apps.push(PinnedApp {
                        label: cm.app_name.to_string(),
                        program: cm.exec.to_string(),
                        args: vec![],
                        terminal: false,
                        icon_name,
                    });
                    self.save_pinned_apps();
                    self.draw_panel(qh, crate::wayland::RepaintReason::Pointer);
                }
            }
            ContextMenuAction::UnpinFromPanel => {
                self.pinned_apps.retain(|p| p.program != cm.exec.as_ref());
                self.save_pinned_apps();
                self.draw_panel(qh, crate::wayland::RepaintReason::Pointer);
            }
            ContextMenuAction::RemoveFromLauncher => {
                self.hidden_execs.insert(cm.exec.to_string());
                self.save_hidden_apps();
                self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
            }
        }
    }
}
