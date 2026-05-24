use wayland_client::QueueHandle;

use super::MeridianShell;
use crate::{
    context_menu::{ContextMenuAction, ContextMenuState},
    panel::PinnedApp,
    wayland::{CommitReason, RepaintReason},
};

impl MeridianShell {
    #[allow(dead_code)]
    pub(super) fn dispatch_widget_action(
        &mut self,
        qh: &QueueHandle<MeridianShell>,
        action: crate::widget_action::WidgetAction,
    ) {
        use crate::widget_action::WidgetAction;

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
            self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
        }

        match action {
            WidgetAction::ToggleUiPreview => {
                self.app_view_open = true;
                self.launcher_settings_open = false;
                self.ui_preview_widget_state = None;
                self.app_view_scroll_y = 0;
                self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
            }
            WidgetAction::ShowTileView => {
                self.app_view_open = false;
                self.launcher_settings_open = false;
                self.search_query.clear();
                self.ui_preview_widget_state = None;
                self.app_view_scroll_y = 0;
                self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
            }
            WidgetAction::SetCategory(cat) => {
                self.app_view_category = cat;
                self.search_query.clear();
                self.ui_preview_widget_state = None;
                self.app_view_scroll_y = 0;
                self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
            }
            WidgetAction::LaunchApp { program, args } => {
                match std::process::Command::new(&program).args(&args).spawn() {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("launch failed: {:?}", e);
                    }
                }
                // Mirror handle_launcher_click's behavior: any launch via
                // a widget click should dismiss the launcher so the new
                // window is not occluded. No-op when launcher is closed
                // (close_launcher_after_launch checks).
                self.close_launcher_after_launch(qh, crate::wayland::RepaintReason::Pointer);
            }
            WidgetAction::LaunchExec(exec) => {
                match std::process::Command::new(&exec).spawn() {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("launch failed: {:?}", e);
                    }
                }
                self.close_launcher_after_launch(qh, crate::wayland::RepaintReason::Pointer);
            }
            WidgetAction::FocusWindow(id) => {
                self.ipc
                    .send(&meridian_ipc::ShellCommand::FocusWindow { id });
            }
            WidgetAction::ToggleCalendar => {
                self.toggle_calendar_popup(CommitReason::Input);
                self.draw_panel(qh, crate::wayland::RepaintReason::Pointer);
                if self.calendar_popup_open {
                    self.draw_calendar_popup(qh, crate::wayland::RepaintReason::Pointer);
                }
            }
            WidgetAction::ToggleNetworkPopup => {
                self.toggle_network_popup(CommitReason::Input);
                self.draw_panel(qh, crate::wayland::RepaintReason::Pointer);
                if self.network_popup_open {
                    self.draw_network_popup(qh, crate::wayland::RepaintReason::Pointer);
                }
            }
            WidgetAction::ToggleWorkspacePopup => {
                self.toggle_workspace_popup(CommitReason::Input);
                self.draw_panel(qh, crate::wayland::RepaintReason::Pointer);
                if self.workspace_popup_open {
                    self.draw_workspace_popup(qh, crate::wayland::RepaintReason::Pointer);
                }
            }
            WidgetAction::ToggleSettings => {
                self.launcher_settings_open = true;
                self.app_view_open = false;
                self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
            }
            WidgetAction::SetSettingsCategory(cat) => {
                self.settings_category = cat;
                if cat == crate::settings_view::SettingsCategory::Wallpaper
                    && self.wallpaper_thumbnails.is_empty()
                {
                    self.load_wallpaper_thumbnails();
                }
                self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
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
            WidgetAction::BrowseWallpaper => {
                // Close launcher so the file dialog opens in the foreground.
                self.close_launcher_after_launch(qh, crate::wayland::RepaintReason::Pointer);
                self.spawn_file_picker();
            }
            WidgetAction::PowerOff => {
                if self.try_consume_armed_power("power-off") {
                    self.close_launcher_after_launch(qh, crate::wayland::RepaintReason::Pointer);
                    std::thread::spawn(|| { let _ = std::process::Command::new("systemctl").arg("poweroff").status(); });
                } else {
                    self.arm_power(qh, "power-off");
                }
            }
            WidgetAction::PowerRestart => {
                if self.try_consume_armed_power("power-restart") {
                    self.close_launcher_after_launch(qh, crate::wayland::RepaintReason::Pointer);
                    std::thread::spawn(|| { let _ = std::process::Command::new("systemctl").arg("reboot").status(); });
                } else {
                    self.arm_power(qh, "power-restart");
                }
            }
            WidgetAction::PowerSleep => {
                if self.try_consume_armed_power("power-sleep") {
                    self.close_launcher_after_launch(qh, crate::wayland::RepaintReason::Pointer);
                    std::thread::spawn(|| { let _ = std::process::Command::new("systemctl").arg("suspend").status(); });
                } else {
                    self.arm_power(qh, "power-sleep");
                }
            }
            WidgetAction::PowerLock => {
                if self.try_consume_armed_power("power-lock") {
                    self.close_launcher_after_launch(qh, crate::wayland::RepaintReason::Pointer);
                    std::thread::spawn(|| { let _ = std::process::Command::new("loginctl").arg("lock-session").status(); });
                } else {
                    self.arm_power(qh, "power-lock");
                }
            }
            WidgetAction::PowerLogout => {
                if self.try_consume_armed_power("power-logout") {
                    tracing::info!("power: logout requested — exiting shell");
                    std::process::exit(0);
                } else {
                    self.arm_power(qh, "power-logout");
                }
            }
            WidgetAction::PinnedMoveUp(idx) => {
                if idx > 0 && idx < self.pinned_apps.len() {
                    self.pinned_apps.swap(idx - 1, idx);
                    self.save_pinned_apps();
                    self.draw_panel(qh, RepaintReason::Pointer);
                    self.draw_launcher(qh, RepaintReason::Pointer);
                }
            }
            WidgetAction::PinnedMoveDown(idx) => {
                if idx + 1 < self.pinned_apps.len() {
                    self.pinned_apps.swap(idx, idx + 1);
                    self.save_pinned_apps();
                    self.draw_panel(qh, RepaintReason::Pointer);
                    self.draw_launcher(qh, RepaintReason::Pointer);
                }
            }
            WidgetAction::PinnedRemove(idx) => {
                if idx < self.pinned_apps.len() {
                    self.pinned_apps.remove(idx);
                    self.save_pinned_apps();
                    self.draw_panel(qh, RepaintReason::Pointer);
                    self.draw_launcher(qh, RepaintReason::Pointer);
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
                let pinned_programs: std::collections::HashSet<&str> =
                    self.pinned_apps.iter().map(|p| p.program.as_str()).collect();
                let mut addable: Vec<&crate::launcher::DesktopApp> = self.launcher_state.apps.iter()
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
        }
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
                    self.ipc.send(&meridian_ipc::ShellCommand::FocusWindow { id: wid.clone() });
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
                if !self.pinned_apps.iter().any(|p| p.program == cm.exec.as_ref()) {
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

#[cfg(test)]
mod tests {}
