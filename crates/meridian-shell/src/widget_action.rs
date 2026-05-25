use crate::{
    app_view::AppCategory,
    settings_view::{SettingsCategory, SettingsRootCategory},
};

const SETTINGS_THEME_PREFIX: &str = "settings-theme-";
const SETTINGS_WALLPAPER_PREFIX: &str = "settings-wallpaper-";
const PINNED_MOVE_UP_PREFIX: &str = "pinned-move-up-";
const PINNED_MOVE_DOWN_PREFIX: &str = "pinned-move-dn-";
const PINNED_REMOVE_PREFIX: &str = "pinned-remove-";
const PINNED_ADD_APP_PREFIX: &str = "pinned-add-app-";
const DISPLAY_PRIMARY_PREFIX: &str = "display-primary-";

const CATEGORY_ACTIONS: &[(&str, AppCategory)] = &[
    ("cat-internet", AppCategory::Internet),
    ("cat-kreativ", AppCategory::Kreativ),
    ("cat-buero", AppCategory::Buero),
    ("cat-entwicklung", AppCategory::Entwicklung),
    ("cat-system", AppCategory::System),
    ("cat-spiele", AppCategory::Spiele),
    ("cat-alle", AppCategory::Alle),
];

const SETTINGS_CATEGORY_ACTIONS: &[(&str, SettingsCategory)] = &[
    ("settings-cat-theme", SettingsCategory::Theme),
    ("settings-cat-cursor", SettingsCategory::Cursor),
    ("settings-cat-display", SettingsCategory::Display),
    ("settings-cat-wallpaper", SettingsCategory::Wallpaper),
    ("settings-cat-pinned", SettingsCategory::PinnedApps),
    (
        "settings-cat-system-overview",
        SettingsCategory::SystemOverview,
    ),
    ("settings-cat-network", SettingsCategory::Network),
    ("settings-cat-bluetooth", SettingsCategory::Bluetooth),
    ("settings-cat-sound", SettingsCategory::Sound),
    ("settings-cat-printers", SettingsCategory::Printers),
    ("settings-cat-power", SettingsCategory::Power),
    ("settings-cat-users", SettingsCategory::Users),
    ("settings-cat-updates", SettingsCategory::Updates),
];

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
    SetPrimaryOutput(usize),
}

pub(crate) fn action_for_id(id: &str) -> Option<WidgetAction> {
    exact_action_for_id(id)
        .or_else(|| category_action_for_id(id))
        .or_else(|| settings_category_action_for_id(id))
        .or_else(|| {
            parse_indexed_action(id, SETTINGS_THEME_PREFIX, WidgetAction::ApplyThemeByIndex)
        })
        .or_else(|| {
            parse_indexed_action(
                id,
                SETTINGS_WALLPAPER_PREFIX,
                WidgetAction::ApplyWallpaperByIndex,
            )
        })
        .or_else(|| parse_indexed_action(id, PINNED_MOVE_UP_PREFIX, WidgetAction::PinnedMoveUp))
        .or_else(|| parse_indexed_action(id, PINNED_MOVE_DOWN_PREFIX, WidgetAction::PinnedMoveDown))
        .or_else(|| parse_indexed_action(id, PINNED_REMOVE_PREFIX, WidgetAction::PinnedRemove))
        .or_else(|| parse_indexed_action(id, PINNED_ADD_APP_PREFIX, WidgetAction::PinnedAddApp))
        .or_else(|| {
            parse_indexed_action(id, DISPLAY_PRIMARY_PREFIX, WidgetAction::SetPrimaryOutput)
        })
}

fn exact_action_for_id(id: &str) -> Option<WidgetAction> {
    match id {
        "apps-switch" => Some(WidgetAction::ToggleUiPreview),
        "show-tile-view" => Some(WidgetAction::ShowTileView),
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
        "settings-root-desktop" => Some(WidgetAction::SetSettingsCategory(
            SettingsRootCategory::Desktop.first_category(),
        )),
        "settings-root-system" => Some(WidgetAction::SetSettingsCategory(
            SettingsRootCategory::System.first_category(),
        )),
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
        "pinned-add-open" => Some(WidgetAction::PinnedOpenAdd),
        "pinned-add-close" => Some(WidgetAction::PinnedCloseAdd),
        _ => None,
    }
}

fn category_action_for_id(id: &str) -> Option<WidgetAction> {
    CATEGORY_ACTIONS
        .iter()
        .find_map(|(candidate, category)| (*candidate == id).then_some(*category))
        .map(WidgetAction::SetCategory)
}

fn settings_category_action_for_id(id: &str) -> Option<WidgetAction> {
    SETTINGS_CATEGORY_ACTIONS
        .iter()
        .find_map(|(candidate, category)| (*candidate == id).then_some(*category))
        .map(WidgetAction::SetSettingsCategory)
}

fn parse_indexed_action(
    id: &str,
    prefix: &str,
    action: impl FnOnce(usize) -> WidgetAction,
) -> Option<WidgetAction> {
    id.strip_prefix(prefix)?.parse::<usize>().ok().map(action)
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
    fn action_for_id_settings_category() {
        assert_eq!(
            action_for_id("settings-root-desktop"),
            Some(WidgetAction::SetSettingsCategory(
                crate::settings_view::SettingsCategory::Theme
            ))
        );
        assert_eq!(
            action_for_id("settings-root-system"),
            Some(WidgetAction::SetSettingsCategory(
                crate::settings_view::SettingsCategory::SystemOverview
            ))
        );
        assert_eq!(
            action_for_id("settings-cat-display"),
            Some(WidgetAction::SetSettingsCategory(
                crate::settings_view::SettingsCategory::Display
            ))
        );
        assert_eq!(
            action_for_id("settings-cat-wallpaper"),
            Some(WidgetAction::SetSettingsCategory(
                crate::settings_view::SettingsCategory::Wallpaper
            ))
        );
        assert_eq!(
            action_for_id("settings-cat-printers"),
            Some(WidgetAction::SetSettingsCategory(
                crate::settings_view::SettingsCategory::Printers
            ))
        );
    }

    #[test]
    fn action_for_id_indexed_ids() {
        assert_eq!(
            action_for_id("settings-theme-12"),
            Some(WidgetAction::ApplyThemeByIndex(12))
        );
        assert_eq!(
            action_for_id("settings-wallpaper-3"),
            Some(WidgetAction::ApplyWallpaperByIndex(3))
        );
        assert_eq!(
            action_for_id("pinned-move-up-2"),
            Some(WidgetAction::PinnedMoveUp(2))
        );
        assert_eq!(
            action_for_id("pinned-move-dn-4"),
            Some(WidgetAction::PinnedMoveDown(4))
        );
        assert_eq!(
            action_for_id("pinned-remove-8"),
            Some(WidgetAction::PinnedRemove(8))
        );
        assert_eq!(
            action_for_id("pinned-add-app-5"),
            Some(WidgetAction::PinnedAddApp(5))
        );
        assert_eq!(
            action_for_id("display-primary-1"),
            Some(WidgetAction::SetPrimaryOutput(1))
        );
    }

    #[test]
    fn action_for_id_indexed_ids_reject_malformed_suffixes() {
        assert_eq!(action_for_id("settings-theme-"), None);
        assert_eq!(action_for_id("settings-theme-abc"), None);
        assert_eq!(action_for_id("pinned-remove-x"), None);
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
