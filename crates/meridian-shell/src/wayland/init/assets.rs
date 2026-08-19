use meridian_config::ThemeConfig;

use crate::{icons::IconCache, panel};

pub(in crate::wayland) fn build_icon_cache(
    theme: &ThemeConfig,
    pinned_apps: &[panel::PinnedApp],
) -> IconCache {
    let mut icon_cache = IconCache::new_for_theme(&theme.icons.theme, &theme.colors.text.to_hex());
    // Only warm icons needed immediately by the panel and tray. Launcher grid
    // icons remain lazy so SVG decoding cannot delay the first panel frame.
    icon_cache.warm(
        &[
            "utilities-terminal",
            "chromium",
            "firefox",
            "org.kde.dolphin",
        ],
        22,
    );
    icon_cache.warm(
        &[
            "network-wired-symbolic",
            "network-wired-disconnected-symbolic",
            "network-wireless-signal-excellent-symbolic",
            "network-wireless-signal-good-symbolic",
            "network-wireless-signal-none-symbolic",
            "network-wireless-disconnected-symbolic",
            "network-vpn-symbolic",
            "network-offline-symbolic",
            "camera-photo-symbolic",
            "audio-volume-high-symbolic",
            "audio-volume-medium-symbolic",
            "audio-volume-low-symbolic",
            "audio-volume-muted-symbolic",
        ],
        22,
    );
    icon_cache.warm(crate::battery::ICON_NAMES, 22);
    icon_cache.warm(
        &[
            "thunderbird",
            "chromium",
            "system-file-manager",
            "gwenview",
            "amarok",
            "marble",
            "akregator",
            "org.kde.discover",
            "org.kde.korganizer",
            "org.kde.kweather",
            "org.kde.knotes",
        ],
        64,
    );
    icon_cache.warm(
        &[
            "system-shutdown",
            "system-reboot",
            "system-suspend",
            "system-lock-screen",
            "system-log-out",
        ],
        32,
    );

    let pinned_icons: Vec<&str> = pinned_apps
        .iter()
        .filter_map(|app| app.icon_name.as_deref())
        .filter(|name| !name.is_empty())
        .collect();
    if !pinned_icons.is_empty() {
        icon_cache.warm(&pinned_icons, 22);
        icon_cache.warm(&pinned_icons, 24);
        icon_cache.warm(&pinned_icons, 48);
    }

    icon_cache
}
