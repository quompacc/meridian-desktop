use wayland_client::QueueHandle;

use super::MeridianShell;
use crate::{
    context_menu::{ContextMenuAction, ContextMenuState, DesktopContextMenuAction},
    panel::PinnedApp,
    wayland::{CommitReason, RepaintReason},
    widget_action::WidgetAction,
};

include!("widget_dispatch/dispatch.rs");
include!("widget_dispatch/settings_and_context.rs");

/// Map the three privileged actions to the platform session-service API. Lock
/// and logout stay on the existing compositor IPC path.
fn system_power_action(action: WidgetAction) -> Option<crate::system_power::SystemPowerAction> {
    use crate::system_power::SystemPowerAction;
    match action {
        WidgetAction::PowerOff => Some(SystemPowerAction::PowerOff),
        WidgetAction::PowerRestart => Some(SystemPowerAction::Reboot),
        WidgetAction::PowerSleep => Some(SystemPowerAction::Suspend),
        _ => None,
    }
}

#[cfg(test)]
mod power_tests {
    use super::system_power_action;
    use crate::system_power::SystemPowerAction;
    use crate::widget_action::WidgetAction;

    #[test]
    fn privileged_actions_map_to_session_service_requests() {
        assert_eq!(
            system_power_action(WidgetAction::PowerOff),
            Some(SystemPowerAction::PowerOff)
        );
        assert_eq!(
            system_power_action(WidgetAction::PowerRestart),
            Some(SystemPowerAction::Reboot)
        );
        assert_eq!(
            system_power_action(WidgetAction::PowerSleep),
            Some(SystemPowerAction::Suspend)
        );
    }

    #[test]
    fn logout_has_no_external_command() {
        assert!(system_power_action(WidgetAction::PowerLogout).is_none());
    }

    #[test]
    fn lock_has_no_external_command() {
        assert!(system_power_action(WidgetAction::PowerLock).is_none());
    }
}
