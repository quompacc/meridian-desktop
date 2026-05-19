use wayland_client::QueueHandle;

use super::MeridianShell;

impl MeridianShell {
    pub(super) fn dispatch_widget_action(
        &mut self,
        qh: &QueueHandle<MeridianShell>,
        action: crate::widget_action::WidgetAction,
    ) {
        use crate::widget_action::WidgetAction;
        match action {
            WidgetAction::ToggleUiPreview => {
                self.app_view_open = true;
                self.ui_preview_widget_state = None;
                self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
            }
            WidgetAction::ShowTileView => {
                self.app_view_open = false;
                self.ui_preview_widget_state = None;
                self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
            }
            WidgetAction::SetCategory(cat) => {
                self.app_view_category = cat;
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
            }
            WidgetAction::LaunchExec(exec) => match std::process::Command::new(&exec).spawn() {
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("launch failed: {:?}", e);
                }
            },
            WidgetAction::PowerOff
            | WidgetAction::PowerRestart
            | WidgetAction::PowerSleep
            | WidgetAction::PowerLock
            | WidgetAction::PowerLogout => {
                tracing::info!("power action requested: {:?}", action);
            }
        }
    }
}

#[cfg(test)]
mod tests {}
