//! One-shot Settings data refreshes kept off the Wayland event loop.

use std::sync::mpsc::Sender;

use crate::settings_view::SettingsCategory;

pub(crate) enum SettingsData {
    SystemInfo(crate::sysinfo::SystemInfo),
    Printers(crate::printers::PrinterSnapshot),
    Audio(crate::audio::AudioSnapshot),
    Network {
        profiles: Vec<crate::network::ConnectionProfile>,
        wifi: Vec<crate::network::WifiNetwork>,
    },
    Bluetooth(crate::bluetooth::BluetoothSnapshot),
    DefaultApps {
        index: crate::default_apps::MimeAppIndex,
        current: std::collections::HashMap<crate::default_apps::DefaultAppCategory, String>,
    },
    WallpaperThumbnails(Vec<Option<(u32, u32, Vec<u8>)>>),
}

pub(crate) struct SettingsRefreshResult {
    pub category: SettingsCategory,
    pub data: SettingsData,
}

pub(crate) fn supports(category: SettingsCategory) -> bool {
    matches!(
        category,
        SettingsCategory::SystemOverview
            | SettingsCategory::Printers
            | SettingsCategory::Sound
            | SettingsCategory::Network
            | SettingsCategory::Bluetooth
            | SettingsCategory::DefaultApps
            | SettingsCategory::Wallpaper
    )
}

pub(crate) fn spawn(
    category: SettingsCategory,
    wallpapers: Vec<meridian_config::WallpaperEntry>,
    tx: Sender<SettingsRefreshResult>,
) {
    if !supports(category) {
        return;
    }
    std::thread::spawn(move || {
        let data = match category {
            SettingsCategory::SystemOverview => {
                SettingsData::SystemInfo(crate::sysinfo::SystemInfo::gather())
            }
            SettingsCategory::Printers => {
                SettingsData::Printers(crate::printers::PrinterSnapshot::poll())
            }
            SettingsCategory::Sound => SettingsData::Audio(crate::audio::AudioSnapshot::poll()),
            SettingsCategory::Network => SettingsData::Network {
                profiles: crate::network::list_saved_connections(),
                wifi: crate::network::scan_wifi_networks(),
            },
            SettingsCategory::Bluetooth => {
                SettingsData::Bluetooth(crate::bluetooth::BluetoothSnapshot::poll())
            }
            SettingsCategory::DefaultApps => SettingsData::DefaultApps {
                index: crate::default_apps::MimeAppIndex::load_system(),
                current: crate::default_apps::snapshot_current_defaults(),
            },
            SettingsCategory::Wallpaper => {
                let settings = meridian_tokens::Settings::DEFAULT;
                SettingsData::WallpaperThumbnails(
                    wallpapers
                        .iter()
                        .map(|entry| {
                            crate::wayland::load_wallpaper_thumbnail(
                                &entry.thumbnail_path,
                                settings.wallpaper_thumbnail_width,
                                settings.wallpaper_thumbnail_height,
                            )
                        })
                        .collect(),
                )
            }
            _ => return,
        };
        let _ = tx.send(SettingsRefreshResult { category, data });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_io_backed_pages_request_a_refresh() {
        assert!(supports(SettingsCategory::SystemOverview));
        assert!(supports(SettingsCategory::Printers));
        assert!(supports(SettingsCategory::Sound));
        assert!(supports(SettingsCategory::Network));
        assert!(supports(SettingsCategory::Bluetooth));
        assert!(supports(SettingsCategory::DefaultApps));
        assert!(supports(SettingsCategory::Wallpaper));
        assert!(!supports(SettingsCategory::Theme));
        assert!(!supports(SettingsCategory::Cursor));
        assert!(!supports(SettingsCategory::Display));
    }
}
