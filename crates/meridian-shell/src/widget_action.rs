use crate::app_view::AppCategory;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum WidgetAction {
    ToggleUiPreview,
    ShowTileView,
    SetCategory(AppCategory),
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
        "show-tile-view" => Some(WidgetAction::ShowTileView),
        "cat-internet" => Some(WidgetAction::SetCategory(AppCategory::Internet)),
        "cat-kreativ" => Some(WidgetAction::SetCategory(AppCategory::Kreativ)),
        "cat-buero" => Some(WidgetAction::SetCategory(AppCategory::Buero)),
        "cat-entwicklung" => Some(WidgetAction::SetCategory(AppCategory::Entwicklung)),
        "cat-system" => Some(WidgetAction::SetCategory(AppCategory::System)),
        "cat-spiele" => Some(WidgetAction::SetCategory(AppCategory::Spiele)),
        "cat-alle" => Some(WidgetAction::SetCategory(AppCategory::Alle)),
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
    use crate::app_view::AppCategory;

    use super::{action_for_id, WidgetAction};

    #[test]
    fn action_for_id_apps_switch() {
        assert_eq!(
            action_for_id("apps-switch"),
            Some(WidgetAction::ToggleUiPreview)
        );
    }

    #[test]
    fn action_for_id_show_tile_view() {
        assert_eq!(
            action_for_id("show-tile-view"),
            Some(WidgetAction::ShowTileView)
        );
    }

    #[test]
    fn action_for_id_set_category() {
        assert_eq!(
            action_for_id("cat-internet"),
            Some(WidgetAction::SetCategory(AppCategory::Internet))
        );
        assert_eq!(
            action_for_id("cat-kreativ"),
            Some(WidgetAction::SetCategory(AppCategory::Kreativ))
        );
        assert_eq!(
            action_for_id("cat-buero"),
            Some(WidgetAction::SetCategory(AppCategory::Buero))
        );
        assert_eq!(
            action_for_id("cat-entwicklung"),
            Some(WidgetAction::SetCategory(AppCategory::Entwicklung))
        );
        assert_eq!(
            action_for_id("cat-system"),
            Some(WidgetAction::SetCategory(AppCategory::System))
        );
        assert_eq!(
            action_for_id("cat-spiele"),
            Some(WidgetAction::SetCategory(AppCategory::Spiele))
        );
        assert_eq!(
            action_for_id("cat-alle"),
            Some(WidgetAction::SetCategory(AppCategory::Alle))
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
