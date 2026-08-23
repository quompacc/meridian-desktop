impl MeridianShell {
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
        let id = match action {
            WidgetAction::PowerOff => "power-off",
            WidgetAction::PowerRestart => "power-restart",
            WidgetAction::PowerSleep => "power-sleep",
            WidgetAction::PowerLock => "power-lock",
            WidgetAction::PowerLogout => "power-logout",
            _ => unreachable!("non power action routed to power dispatcher"),
        };
        if !self.try_consume_armed_power(id) {
            self.arm_power(qh, id);
            return;
        }

        self.close_launcher_after_launch(qh, RepaintReason::Pointer);
        if matches!(action, WidgetAction::PowerLock) {
            tracing::info!("power: lock requested through compositor supervisor");
            if !self.ipc.send(&meridian_ipc::ShellCommand::LockSession) {
                tracing::warn!("power: lock request failed - compositor IPC unavailable");
                self.arm_power(qh, id);
            }
            return;
        }

        let command = power_action_command(action);
        if let Some((program, args)) = command {
            std::thread::spawn(move || {
                let _ = std::process::Command::new(program).args(args).status();
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
                let (program, args) = crate::default_apps::pick_file_manager();
                tracing::info!(
                    "desktop menu: launching file manager {} {:?}",
                    program,
                    args
                );
                let command = meridian_ipc::ShellCommand::LaunchApp {
                    program,
                    args,
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
                if !self.ipc.send(&meridian_ipc::ShellCommand::LockSession) {
                    tracing::warn!("desktop lock request failed - compositor IPC unavailable");
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
