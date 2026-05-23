use wayland_client::QueueHandle;

use super::MeridianShell;
use crate::{
    context_menu::{ContextMenuAction, ContextMenuState},
    panel::PinnedApp,
    wayland::CommitReason,
};

impl MeridianShell {
    #[allow(dead_code)]
    pub(super) fn dispatch_widget_action(
        &mut self,
        qh: &QueueHandle<MeridianShell>,
        action: crate::widget_action::WidgetAction,
    ) {
        use crate::widget_action::WidgetAction;
        match action {
            WidgetAction::ToggleUiPreview => {
                self.app_view_open = true;
                self.launcher_settings_open = false;
                self.ui_preview_widget_state = None;
                self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
            }
            WidgetAction::ShowTileView => {
                self.app_view_open = false;
                self.launcher_settings_open = false;
                self.search_query.clear();
                self.ui_preview_widget_state = None;
                self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
            }
            WidgetAction::SetCategory(cat) => {
                self.app_view_category = cat;
                self.search_query.clear();
                self.ui_preview_widget_state = None;
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
                self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
            }
            WidgetAction::ApplyThemeByIndex(idx) => {
                if let Some(name) = self.available_themes.get(idx).cloned() {
                    self.apply_theme(qh, name);
                }
            }
            WidgetAction::PowerOff
            | WidgetAction::PowerRestart
            | WidgetAction::PowerSleep
            | WidgetAction::PowerLock
            | WidgetAction::PowerLogout => {
                tracing::info!("power action requested: {:?}", action);
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
                    self.draw_panel(qh, crate::wayland::RepaintReason::Pointer);
                }
            }
            ContextMenuAction::UnpinFromPanel => {
                self.pinned_apps.retain(|p| p.program != cm.exec.as_ref());
                self.draw_panel(qh, crate::wayland::RepaintReason::Pointer);
            }
        }
    }
}

#[cfg(test)]
mod tests {}
