use crate::app_view::AppCategory;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum WidgetAction {
    ToggleUiPreview,
    ShowTileView,
    SetCategory(AppCategory),
    LaunchApp { program: String, args: Vec<String> },
    LaunchExec(String),
    ToggleCalendar,
    ToggleNetworkPopup,
    ToggleWorkspacePopup,
    PowerOff,
    PowerRestart,
    PowerSleep,
    PowerLock,
    PowerLogout,
    ToggleSettings,
    SetSettingsCategory(crate::settings_view::SettingsCategory),
    ApplyThemeByIndex(usize),
    ApplyWallpaperByIndex(usize),
    SetWallpaperMode(meridian_config::WallpaperMode),
    BrowseWallpaper,
    PinnedMoveUp(usize),
    PinnedMoveDown(usize),
    PinnedRemove(usize),
    PinnedOpenAdd,
    PinnedCloseAdd,
    PinnedAddApp(usize),
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
        "panel-launcher" => Some(WidgetAction::ToggleUiPreview),
        "panel-network" => Some(WidgetAction::ToggleNetworkPopup),
        "panel-workspace" => Some(WidgetAction::ToggleWorkspacePopup),
        "panel-clock" => Some(WidgetAction::ToggleCalendar),
        "power-off" => Some(WidgetAction::PowerOff),
        "power-restart" => Some(WidgetAction::PowerRestart),
        "power-sleep" => Some(WidgetAction::PowerSleep),
        "power-lock" => Some(WidgetAction::PowerLock),
        "power-logout" => Some(WidgetAction::PowerLogout),
        "launcher-settings" => Some(WidgetAction::ToggleSettings),
        "settings-cat-theme" => Some(WidgetAction::SetSettingsCategory(
            crate::settings_view::SettingsCategory::Theme,
        )),
        "settings-cat-cursor" => Some(WidgetAction::SetSettingsCategory(
            crate::settings_view::SettingsCategory::Cursor,
        )),
        "settings-cat-wallpaper" => Some(WidgetAction::SetSettingsCategory(
            crate::settings_view::SettingsCategory::Wallpaper,
        )),
        "settings-cat-pinned" => Some(WidgetAction::SetSettingsCategory(
            crate::settings_view::SettingsCategory::PinnedApps,
        )),
        id if id.starts_with("settings-theme-") => id["settings-theme-".len()..]
            .parse::<usize>()
            .ok()
            .map(WidgetAction::ApplyThemeByIndex),
        "wallpaper-mode-fill" => Some(WidgetAction::SetWallpaperMode(
            meridian_config::WallpaperMode::Fill,
        )),
        "wallpaper-mode-fit" => Some(WidgetAction::SetWallpaperMode(
            meridian_config::WallpaperMode::Fit,
        )),
        "wallpaper-mode-center" => Some(WidgetAction::SetWallpaperMode(
            meridian_config::WallpaperMode::Center,
        )),
        "wallpaper-mode-tile" => Some(WidgetAction::SetWallpaperMode(
            meridian_config::WallpaperMode::Tile,
        )),
        "wallpaper-browse" => Some(WidgetAction::BrowseWallpaper),
        id if id.starts_with("settings-wallpaper-") => id["settings-wallpaper-".len()..]
            .parse::<usize>()
            .ok()
            .map(WidgetAction::ApplyWallpaperByIndex),
        id if id.starts_with("pinned-move-up-") => id["pinned-move-up-".len()..]
            .parse::<usize>()
            .ok()
            .map(WidgetAction::PinnedMoveUp),
        id if id.starts_with("pinned-move-dn-") => id["pinned-move-dn-".len()..]
            .parse::<usize>()
            .ok()
            .map(WidgetAction::PinnedMoveDown),
        id if id.starts_with("pinned-remove-") => id["pinned-remove-".len()..]
            .parse::<usize>()
            .ok()
            .map(WidgetAction::PinnedRemove),
        "pinned-add-open" => Some(WidgetAction::PinnedOpenAdd),
        "pinned-add-close" => Some(WidgetAction::PinnedCloseAdd),
        id if id.starts_with("pinned-add-app-") => id["pinned-add-app-".len()..]
            .parse::<usize>()
            .ok()
            .map(WidgetAction::PinnedAddApp),
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
    fn action_for_id_power_logout() {
        assert_eq!(
            action_for_id("power-logout"),
            Some(WidgetAction::PowerLogout)
        );
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
