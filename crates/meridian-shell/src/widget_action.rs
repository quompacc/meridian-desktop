#[derive(Debug, Clone, PartialEq)]
pub(crate) enum WidgetAction {
    ToggleUiPreview,
    LaunchApp { program: String, args: Vec<String> },
    LaunchExec(String),
    PowerOff,
    PowerRestart,
    PowerSleep,
    PowerLock,
    PowerLogout,
}

pub(crate) fn action_for_id(id: &str) -> Option<WidgetAction> {
    match id {
        "apps-switch" => Some(WidgetAction::ToggleUiPreview),
        "power-off" => Some(WidgetAction::PowerOff),
        "power-restart" => Some(WidgetAction::PowerRestart),
        "power-sleep" => Some(WidgetAction::PowerSleep),
        "power-lock" => Some(WidgetAction::PowerLock),
        "power-logout" => Some(WidgetAction::PowerLogout),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{action_for_id, WidgetAction};

    #[test]
    fn action_for_id_apps_switch() {
        assert_eq!(
            action_for_id("apps-switch"),
            Some(WidgetAction::ToggleUiPreview)
        );
    }

    #[test]
    fn action_for_id_power_off() {
        assert_eq!(action_for_id("power-off"), Some(WidgetAction::PowerOff));
    }

    #[test]
    fn action_for_id_unknown() {
        assert_eq!(action_for_id("unknown-id"), None);
    }

    #[test]
    fn action_for_id_empty() {
        assert_eq!(action_for_id(""), None);
    }

    #[test]
    fn launch_app_variant_is_debug_printable() {
        let action = WidgetAction::LaunchApp {
            program: "firefox".to_string(),
            args: vec!["--new-window".to_string()],
        };
        let dbg = format!("{:?}", action);
        assert!(dbg.contains("LaunchApp"));
        assert!(dbg.contains("firefox"));
    }
}
