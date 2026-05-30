use crate::settings_view::SettingsCategory;

const SETTINGS_THEME_PREFIX: &str = "settings-theme-";
const SETTINGS_WALLPAPER_PREFIX: &str = "settings-wallpaper-";
const CURSOR_SIZE_PREFIX: &str = "cursor-size-";
const CURSOR_THEME_PREFIX: &str = "cursor-theme-";
const IDLE_TIMEOUT_PREFIX: &str = "idle-timeout-";
const VOLUME_SET_PREFIX: &str = "vol-set-";
const PINNED_MOVE_UP_PREFIX: &str = "pinned-move-up-";
const PINNED_MOVE_DOWN_PREFIX: &str = "pinned-move-dn-";
const PINNED_REMOVE_PREFIX: &str = "pinned-remove-";
const PINNED_ADD_APP_PREFIX: &str = "pinned-add-app-";
const DISPLAY_PRIMARY_PREFIX: &str = "display-primary-";
const DISPLAY_MODE_TOGGLE_PREFIX: &str = "display-mode-toggle-";
const DISPLAY_MODE_SELECT_PREFIX: &str = "display-mode-select-";

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
    LaunchApp {
        program: String,
        args: Vec<String>,
    },
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
    SetCursorSize(u32),
    ApplyCursorThemeByIndex(usize),
    SetIdleTimeout(Option<u64>),
    SetDefaultSinkVolume(u8),
    ToggleDefaultSinkMute,
    BrowseWallpaper,
    PinnedMoveUp(usize),
    PinnedMoveDown(usize),
    PinnedRemove(usize),
    PinnedOpenAdd,
    PinnedCloseAdd,
    PinnedAddApp(usize),
    SetPrimaryOutput(usize),
    ToggleOutputModeDropdown(usize),
    SetOutputMode {
        output_index: usize,
        mode_index: usize,
    },
}

pub(crate) fn action_for_id(id: &str) -> Option<WidgetAction> {
    exact_action_for_id(id)
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
        .or_else(|| {
            parse_indexed_action(id, CURSOR_SIZE_PREFIX, |n| {
                WidgetAction::SetCursorSize(n as u32)
            })
        })
        .or_else(|| {
            parse_indexed_action(
                id,
                CURSOR_THEME_PREFIX,
                WidgetAction::ApplyCursorThemeByIndex,
            )
        })
        .or_else(|| {
            parse_indexed_action(id, IDLE_TIMEOUT_PREFIX, |n| {
                WidgetAction::SetIdleTimeout(Some(n as u64))
            })
        })
        .or_else(|| {
            parse_indexed_action(id, VOLUME_SET_PREFIX, |n| {
                WidgetAction::SetDefaultSinkVolume(n.min(100) as u8)
            })
        })
        .or_else(|| parse_indexed_action(id, PINNED_MOVE_UP_PREFIX, WidgetAction::PinnedMoveUp))
        .or_else(|| parse_indexed_action(id, PINNED_MOVE_DOWN_PREFIX, WidgetAction::PinnedMoveDown))
        .or_else(|| parse_indexed_action(id, PINNED_REMOVE_PREFIX, WidgetAction::PinnedRemove))
        .or_else(|| parse_indexed_action(id, PINNED_ADD_APP_PREFIX, WidgetAction::PinnedAddApp))
        .or_else(|| {
            parse_indexed_action(id, DISPLAY_PRIMARY_PREFIX, WidgetAction::SetPrimaryOutput)
        })
        .or_else(|| {
            parse_indexed_action(
                id,
                DISPLAY_MODE_TOGGLE_PREFIX,
                WidgetAction::ToggleOutputModeDropdown,
            )
        })
        .or_else(|| parse_display_mode_select_action(id))
}

fn exact_action_for_id(id: &str) -> Option<WidgetAction> {
    match id {
        "panel-launcher" => Some(WidgetAction::ToggleSettings), // panel button now opens settings directly? no — keep as toggle
        "panel-network" => Some(WidgetAction::ToggleNetworkPopup),
        "panel-workspace" => Some(WidgetAction::ToggleWorkspacePopup),
        "panel-clock" => Some(WidgetAction::ToggleCalendar),
        "power-off" => Some(WidgetAction::PowerOff),
        "power-restart" => Some(WidgetAction::PowerRestart),
        "power-sleep" => Some(WidgetAction::PowerSleep),
        "power-lock" => Some(WidgetAction::PowerLock),
        "power-logout" => Some(WidgetAction::PowerLogout),
        "launcher-settings" | "show-tile-view" => Some(WidgetAction::ToggleSettings),
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
        "idle-timeout-off" => Some(WidgetAction::SetIdleTimeout(None)),
        "mute-toggle" => Some(WidgetAction::ToggleDefaultSinkMute),
        _ => None,
    }
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

fn parse_display_mode_select_action(id: &str) -> Option<WidgetAction> {
    let rest = id.strip_prefix(DISPLAY_MODE_SELECT_PREFIX)?;
    let (output, mode) = rest.split_once('-')?;
    Some(WidgetAction::SetOutputMode {
        output_index: output.parse().ok()?,
        mode_index: mode.parse().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::{action_for_id, WidgetAction};

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
    }

    #[test]
    fn action_for_id_indexed_ids() {
        assert_eq!(
            action_for_id("settings-theme-12"),
            Some(WidgetAction::ApplyThemeByIndex(12))
        );
        assert_eq!(
            action_for_id("pinned-remove-8"),
            Some(WidgetAction::PinnedRemove(8))
        );
        assert_eq!(
            action_for_id("display-mode-select-1-3"),
            Some(WidgetAction::SetOutputMode {
                output_index: 1,
                mode_index: 3
            })
        );
    }

    #[test]
    fn action_for_id_cursor_size() {
        assert_eq!(
            action_for_id("cursor-size-32"),
            Some(WidgetAction::SetCursorSize(32))
        );
        assert_eq!(action_for_id("cursor-size-"), None);
        assert_eq!(action_for_id("cursor-size-big"), None);
    }

    #[test]
    fn action_for_id_cursor_theme() {
        assert_eq!(
            action_for_id("cursor-theme-3"),
            Some(WidgetAction::ApplyCursorThemeByIndex(3))
        );
        // cursor-size and cursor-theme share the "cursor-" stem but route apart.
        assert_eq!(
            action_for_id("cursor-size-24"),
            Some(WidgetAction::SetCursorSize(24))
        );
        assert_eq!(action_for_id("cursor-theme-"), None);
        assert_eq!(action_for_id("cursor-theme-x"), None);
    }

    #[test]
    fn action_for_id_idle_timeout() {
        assert_eq!(
            action_for_id("idle-timeout-300"),
            Some(WidgetAction::SetIdleTimeout(Some(300)))
        );
        assert_eq!(
            action_for_id("idle-timeout-off"),
            Some(WidgetAction::SetIdleTimeout(None))
        );
        assert_eq!(action_for_id("idle-timeout-"), None);
        assert_eq!(action_for_id("idle-timeout-nope"), None);
    }

    #[test]
    fn action_for_id_volume_and_mute() {
        assert_eq!(
            action_for_id("vol-set-50"),
            Some(WidgetAction::SetDefaultSinkVolume(50))
        );
        // Clamped so an oversized id can never amplify past unity.
        assert_eq!(
            action_for_id("vol-set-250"),
            Some(WidgetAction::SetDefaultSinkVolume(100))
        );
        assert_eq!(
            action_for_id("mute-toggle"),
            Some(WidgetAction::ToggleDefaultSinkMute)
        );
        assert_eq!(action_for_id("vol-set-"), None);
        assert_eq!(action_for_id("vol-set-x"), None);
    }

    #[test]
    fn action_for_id_indexed_ids_reject_malformed_suffixes() {
        assert_eq!(action_for_id("settings-theme-"), None);
        assert_eq!(action_for_id("settings-theme-abc"), None);
    }

    #[test]
    fn action_for_id_unknown() {
        assert_eq!(action_for_id("unknown-id"), None);
        assert_eq!(action_for_id(""), None);
    }
}
