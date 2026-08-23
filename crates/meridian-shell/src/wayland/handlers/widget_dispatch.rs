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

/// The external command for a power action, or `None` when it is handled
/// internally. Lock always asks the compositor-owned supervisor over IPC;
/// logout asks the compositor to quit. Linux drives systemd for the remaining
/// actions; FreeBSD and other non-Linux targets use shutdown(8) and acpiconf(8).
#[cfg(target_os = "linux")]
fn power_action_command(action: WidgetAction) -> Option<(&'static str, &'static [&'static str])> {
    match action {
        WidgetAction::PowerOff => Some(("systemctl", &["poweroff"])),
        WidgetAction::PowerRestart => Some(("systemctl", &["reboot"])),
        WidgetAction::PowerSleep => Some(("systemctl", &["suspend"])),
        _ => None,
    }
}

#[cfg(not(target_os = "linux"))]
fn power_action_command(action: WidgetAction) -> Option<(&'static str, &'static [&'static str])> {
    match action {
        WidgetAction::PowerOff => Some(("shutdown", &["-p", "now"])),
        WidgetAction::PowerRestart => Some(("shutdown", &["-r", "now"])),
        WidgetAction::PowerSleep => Some(("acpiconf", &["-s", "3"])),
        _ => None,
    }
}

#[cfg(test)]
mod power_tests {
    use super::power_action_command;
    use crate::widget_action::WidgetAction;

    #[test]
    fn power_off_maps_to_platform_command() {
        let (prog, args) = power_action_command(WidgetAction::PowerOff).expect("command");
        #[cfg(target_os = "linux")]
        assert_eq!((prog, args), ("systemctl", &["poweroff"][..]));
        #[cfg(not(target_os = "linux"))]
        assert_eq!((prog, args), ("shutdown", &["-p", "now"][..]));
    }

    #[test]
    fn logout_has_no_external_command() {
        assert!(power_action_command(WidgetAction::PowerLogout).is_none());
    }

    #[test]
    fn lock_has_no_external_command() {
        assert!(power_action_command(WidgetAction::PowerLock).is_none());
    }
}
