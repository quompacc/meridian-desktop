//! Typed, capability-scoped document-to-host bridge.
//!
//! The contract exposes only explicit launcher, Quick Settings and validated
//! app-activation capabilities. Unknown versions, fields and capabilities fail
//! closed.

use meridian_ipc::{AppearanceTheme, AppearanceWallpaperMode, QuickSettingsPowerProfile};
use serde::Deserialize;

pub(crate) const HANDLER_NAME: &str = "meridian";
const SCHEMA_VERSION: u32 = 1;
const MAX_MESSAGE_BYTES: usize = 4096;
const MAX_REQUEST_ID_BYTES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    CloseLauncher,
    ToggleLauncher,
    ToggleQuickSettings,
    CloseQuickSettings,
    OpenSystemSettings,
    RefreshSettings,
    RefreshAppearance,
    SetAppearanceTheme {
        theme: AppearanceTheme,
    },
    PickAppearanceWallpaper,
    SetAppearanceWallpaperMode {
        mode: AppearanceWallpaperMode,
    },
    RefreshNetwork,
    ConnectNetwork {
        ssid: String,
        password: Option<String>,
    },
    DisconnectNetwork,
    SetAudioVolume {
        percent: u8,
    },
    ToggleAudioMute,
    SetPowerProfile {
        profile: QuickSettingsPowerProfile,
    },
    LaunchApp {
        desktop_id: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u32,
    capability: String,
    request_id: String,
    #[serde(default)]
    desktop_id: Option<String>,
    #[serde(default)]
    volume_percent: Option<u8>,
    #[serde(default)]
    power_profile: Option<QuickSettingsPowerProfile>,
    #[serde(default)]
    ssid: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    appearance_theme: Option<AppearanceTheme>,
    #[serde(default)]
    wallpaper_mode: Option<AppearanceWallpaperMode>,
}

pub(crate) fn decode(raw: &str) -> Result<Command, String> {
    if raw.len() > MAX_MESSAGE_BYTES {
        return Err("message exceeds bridge limit".to_string());
    }
    let request: Request = serde_json::from_str(raw).map_err(|_| "invalid request schema")?;
    if request.version != SCHEMA_VERSION {
        return Err("unsupported bridge version".to_string());
    }
    if request.request_id.is_empty()
        || request.request_id.len() > MAX_REQUEST_ID_BYTES
        || !request
            .request_id
            .bytes()
            .all(|byte| byte.is_ascii_graphic())
    {
        return Err("invalid request id".to_string());
    }
    let no_network_value = request.ssid.is_none() && request.password.is_none();
    let no_quick_settings_value =
        request.volume_percent.is_none() && request.power_profile.is_none() && no_network_value;
    let no_appearance_value =
        request.appearance_theme.is_none() && request.wallpaper_mode.is_none();
    let no_control_value = no_quick_settings_value && no_appearance_value;
    match request.capability.as_str() {
        "launcher.close" if request.desktop_id.is_none() && no_control_value => {
            Ok(Command::CloseLauncher)
        }
        "panel.toggle-launcher" if request.desktop_id.is_none() && no_control_value => {
            Ok(Command::ToggleLauncher)
        }
        "panel.toggle-quick-settings" if request.desktop_id.is_none() && no_control_value => {
            Ok(Command::ToggleQuickSettings)
        }
        "quick-settings.close" if request.desktop_id.is_none() && no_control_value => {
            Ok(Command::CloseQuickSettings)
        }
        "quick-settings.open-settings" if request.desktop_id.is_none() && no_control_value => {
            Ok(Command::OpenSystemSettings)
        }
        "settings.refresh" if request.desktop_id.is_none() && no_control_value => {
            Ok(Command::RefreshSettings)
        }
        "settings.appearance.refresh" if request.desktop_id.is_none() && no_control_value => {
            Ok(Command::RefreshAppearance)
        }
        "settings.appearance.set-theme"
            if request.desktop_id.is_none()
                && no_quick_settings_value
                && request.wallpaper_mode.is_none()
                && request.appearance_theme.is_some() =>
        {
            Ok(Command::SetAppearanceTheme {
                theme: request
                    .appearance_theme
                    .expect("guarded by match condition"),
            })
        }
        "settings.appearance.pick-wallpaper"
            if request.desktop_id.is_none() && no_control_value =>
        {
            Ok(Command::PickAppearanceWallpaper)
        }
        "settings.appearance.set-wallpaper-mode"
            if request.desktop_id.is_none()
                && no_quick_settings_value
                && request.appearance_theme.is_none()
                && request.wallpaper_mode.is_some() =>
        {
            Ok(Command::SetAppearanceWallpaperMode {
                mode: request.wallpaper_mode.expect("guarded by match condition"),
            })
        }
        "quick-settings.network.refresh" if request.desktop_id.is_none() && no_control_value => {
            Ok(Command::RefreshNetwork)
        }
        "quick-settings.network.disconnect" if request.desktop_id.is_none() && no_control_value => {
            Ok(Command::DisconnectNetwork)
        }
        "quick-settings.network.connect"
            if request.desktop_id.is_none()
                && request.volume_percent.is_none()
                && request.power_profile.is_none()
                && no_appearance_value
                && valid_network_value(request.ssid.as_deref(), request.password.as_deref()) =>
        {
            Ok(Command::ConnectNetwork {
                ssid: request.ssid.expect("guarded by match condition"),
                password: request.password,
            })
        }
        "quick-settings.audio.set-volume"
            if request.desktop_id.is_none()
                && request.power_profile.is_none()
                && no_network_value
                && no_appearance_value
                && request.volume_percent.is_some_and(|value| value <= 100) =>
        {
            Ok(Command::SetAudioVolume {
                percent: request.volume_percent.expect("guarded by match condition"),
            })
        }
        "quick-settings.audio.toggle-mute" if request.desktop_id.is_none() && no_control_value => {
            Ok(Command::ToggleAudioMute)
        }
        "quick-settings.power.set-profile"
            if request.desktop_id.is_none()
                && request.volume_percent.is_none()
                && no_network_value
                && no_appearance_value
                && request.power_profile.is_some() =>
        {
            Ok(Command::SetPowerProfile {
                profile: request.power_profile.expect("guarded by match condition"),
            })
        }
        "launcher.launch" if no_control_value => {
            let desktop_id = request.desktop_id.ok_or("missing desktop id")?;
            if desktop_id.is_empty()
                || desktop_id.len() > 512
                || desktop_id.chars().any(char::is_control)
            {
                return Err("invalid desktop id".to_string());
            }
            Ok(Command::LaunchApp { desktop_id })
        }
        _ => Err("capability denied".to_string()),
    }
}

fn valid_network_value(ssid: Option<&str>, password: Option<&str>) -> bool {
    ssid.is_some_and(|ssid| {
        !ssid.is_empty() && ssid.len() <= 128 && !ssid.chars().any(char::is_control)
    }) && password.is_none_or(|password| {
        !password.is_empty() && password.len() <= 256 && !password.chars().any(char::is_control)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_allows_only_launcher_lifecycle_and_catalog_activation() {
        assert_eq!(
            decode(r#"{"version":1,"capability":"launcher.close","request_id":"request-1"}"#),
            Ok(Command::CloseLauncher)
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"panel.toggle-launcher","request_id":"request-panel"}"#
            ),
            Ok(Command::ToggleLauncher)
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"panel.toggle-quick-settings","request_id":"request-quick"}"#
            ),
            Ok(Command::ToggleQuickSettings)
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"quick-settings.close","request_id":"request-close"}"#
            ),
            Ok(Command::CloseQuickSettings)
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"quick-settings.open-settings","request_id":"request-settings"}"#
            ),
            Ok(Command::OpenSystemSettings)
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"settings.refresh","request_id":"request-settings-refresh"}"#
            ),
            Ok(Command::RefreshSettings)
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"settings.appearance.refresh","request_id":"request-appearance"}"#
            ),
            Ok(Command::RefreshAppearance)
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"settings.appearance.set-theme","request_id":"request-theme","appearance_theme":"light"}"#
            ),
            Ok(Command::SetAppearanceTheme {
                theme: AppearanceTheme::Light,
            })
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"settings.appearance.pick-wallpaper","request_id":"request-wallpaper"}"#
            ),
            Ok(Command::PickAppearanceWallpaper)
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"settings.appearance.set-wallpaper-mode","request_id":"request-wallpaper-mode","wallpaper_mode":"fit"}"#
            ),
            Ok(Command::SetAppearanceWallpaperMode {
                mode: AppearanceWallpaperMode::Fit,
            })
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"quick-settings.network.refresh","request_id":"request-network-refresh"}"#
            ),
            Ok(Command::RefreshNetwork)
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"quick-settings.network.connect","request_id":"request-network-connect","ssid":"Meridian","password":"secret phrase"}"#
            ),
            Ok(Command::ConnectNetwork {
                ssid: "Meridian".to_string(),
                password: Some("secret phrase".to_string()),
            })
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"quick-settings.network.disconnect","request_id":"request-network-disconnect"}"#
            ),
            Ok(Command::DisconnectNetwork)
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"quick-settings.audio.set-volume","request_id":"request-volume","volume_percent":72}"#
            ),
            Ok(Command::SetAudioVolume { percent: 72 })
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"quick-settings.audio.toggle-mute","request_id":"request-mute"}"#
            ),
            Ok(Command::ToggleAudioMute)
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"quick-settings.power.set-profile","request_id":"request-power","power_profile":"eco"}"#
            ),
            Ok(Command::SetPowerProfile {
                profile: QuickSettingsPowerProfile::Eco,
            })
        );
        assert_eq!(
            decode(
                r#"{"version":1,"capability":"launcher.launch","request_id":"request-2","desktop_id":"firefox.desktop"}"#
            ),
            Ok(Command::LaunchApp {
                desktop_id: "firefox.desktop".to_string()
            })
        );
        assert!(
            decode(r#"{"version":1,"capability":"process.spawn","request_id":"request-2"}"#)
                .is_err()
        );
    }

    #[test]
    fn malformed_version_id_and_extra_fields_fail_closed() {
        for request in [
            r#"{"version":2,"capability":"launcher.close","request_id":"request-1"}"#,
            r#"{"version":1,"capability":"launcher.close","request_id":""}"#,
            r#"{"version":1,"capability":"launcher.close","request_id":"a b"}"#,
            r#"{"version":1,"capability":"launcher.close","request_id":"ok","extra":true}"#,
            r#"{"version":1,"capability":"launcher.launch","request_id":"ok"}"#,
            r#"{"version":1,"capability":"launcher.launch","request_id":"ok","desktop_id":""}"#,
            r#"{"version":1,"capability":"quick-settings.audio.set-volume","request_id":"ok","volume_percent":101}"#,
            r#"{"version":1,"capability":"quick-settings.power.set-profile","request_id":"ok","power_profile":"turbo"}"#,
            r#"{"version":1,"capability":"quick-settings.audio.toggle-mute","request_id":"ok","volume_percent":20}"#,
            r#"{"version":1,"capability":"quick-settings.network.connect","request_id":"ok","ssid":""}"#,
            r#"{"version":1,"capability":"quick-settings.network.connect","request_id":"ok","ssid":"bad\nssid"}"#,
            r#"{"version":1,"capability":"quick-settings.network.connect","request_id":"ok","ssid":"Meridian","password":""}"#,
            r#"{"version":1,"capability":"settings.appearance.set-theme","request_id":"ok","appearance_theme":"blue"}"#,
            r#"{"version":1,"capability":"settings.appearance.set-theme","request_id":"ok"}"#,
            r#"{"version":1,"capability":"settings.appearance.set-wallpaper-mode","request_id":"ok","wallpaper_mode":"crop"}"#,
            r#"{"version":1,"capability":"settings.appearance.pick-wallpaper","request_id":"ok","wallpaper_mode":"fill"}"#,
        ] {
            assert!(decode(request).is_err(), "accepted {request}");
        }
    }

    #[test]
    fn oversized_message_is_rejected_before_parsing() {
        assert!(decode(&"x".repeat(MAX_MESSAGE_BYTES + 1)).is_err());
    }
}
