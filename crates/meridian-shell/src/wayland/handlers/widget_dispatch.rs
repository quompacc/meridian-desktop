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
                self.ui_preview_enabled = false;
                self.ui_preview_widget_state = None;
                self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
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
}

#[cfg(test)]
mod tests {}
