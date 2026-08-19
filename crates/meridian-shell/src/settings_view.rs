// settings_view.rs — widget-based settings sub-page for the launcher.

use meridian_tokens::Interaction;
use meridian_ui::{
    effect::{paint_fill, paint_text, rounded_rect_path},
    style::Color,
    ui_length,
    widget::{Button, Container, Widget},
    AlignItems, FlexDirection, JustifyContent, Rect, TaffyRect, Theme, UiSize, WidgetState,
    WidgetStyle,
};
use tiny_skia::{Pixmap, PixmapMut, PixmapPaint, PixmapRef, Transform};

use crate::audio::{AudioDevice, AudioServiceState, AudioSnapshot};
use crate::icons::{icon_image_to_pixmap, IconCache};
use crate::launcher::DesktopApp;
use crate::panel::PinnedApp;

use crate::bluetooth::BluetoothSnapshot;
use crate::network::{ConnectionProfile, NetworkState, WifiNetwork};
use crate::printers::{PrinterInfo, PrinterServiceState, PrinterSnapshot};
use crate::sysinfo::SystemInfo;
use meridian_config::{ThemeConfig, WallpaperEntry, WallpaperMode};
use meridian_ipc::{OutputModeState, OutputWorkspaceState};

use crate::ui::tokens::glass_theme_from_config;

// Settings-local design constants. Shared design values live in `meridian_tokens`;
// these are single-purpose to this view (named once instead of inline magic).
/// Tint pulling the "not set" handler label toward the theme error colour.
const NOT_SET_ERROR_TINT: f32 = 0.15;
/// Blend of the per-monitor preview colour into the display-preview body.
const MONITOR_PREVIEW_MIX: f32 = 0.18;
/// Opacity of the settings section divider (drawn over the accent colour).
const SETTINGS_DIVIDER_OPACITY: u8 = 140;

// ─── SettingsCategory ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SettingsCategory {
    #[default]
    Theme,
    Cursor,
    Display,
    Wallpaper,
    PinnedApps,
    DefaultApps,
    SystemOverview,
    Network,
    Bluetooth,
    Sound,
    Printers,
    Power,
    Users,
    Updates,
}

impl SettingsCategory {
    pub const DESKTOP: &'static [SettingsCategory] = &[
        SettingsCategory::Theme,
        SettingsCategory::Cursor,
        SettingsCategory::Wallpaper,
        SettingsCategory::PinnedApps,
        SettingsCategory::DefaultApps,
    ];

    pub const SYSTEM: &'static [SettingsCategory] = &[
        SettingsCategory::SystemOverview,
        SettingsCategory::Display,
        SettingsCategory::Network,
        SettingsCategory::Bluetooth,
        SettingsCategory::Sound,
        SettingsCategory::Printers,
        SettingsCategory::Power,
        SettingsCategory::Users,
        SettingsCategory::Updates,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            SettingsCategory::Theme => "Design",
            SettingsCategory::Cursor => "Mauszeiger",
            SettingsCategory::Display => "Anzeige",
            SettingsCategory::Wallpaper => "Hintergrund",
            SettingsCategory::PinnedApps => "Angeheftet",
            SettingsCategory::DefaultApps => "Standard-Apps",
            SettingsCategory::SystemOverview => "Übersicht",
            SettingsCategory::Network => "Netzwerk",
            SettingsCategory::Bluetooth => "Bluetooth",
            SettingsCategory::Sound => "Audio",
            SettingsCategory::Printers => "Drucker",
            SettingsCategory::Power => "Energie",
            SettingsCategory::Users => "Benutzer",
            SettingsCategory::Updates => "Updates",
        }
    }

    pub fn chip_id(&self) -> &'static str {
        match self {
            SettingsCategory::Theme => "settings-cat-theme",
            SettingsCategory::Cursor => "settings-cat-cursor",
            SettingsCategory::Display => "settings-cat-display",
            SettingsCategory::Wallpaper => "settings-cat-wallpaper",
            SettingsCategory::PinnedApps => "settings-cat-pinned",
            SettingsCategory::DefaultApps => "settings-cat-default-apps",
            SettingsCategory::SystemOverview => "settings-cat-system-overview",
            SettingsCategory::Network => "settings-cat-network",
            SettingsCategory::Bluetooth => "settings-cat-bluetooth",
            SettingsCategory::Sound => "settings-cat-sound",
            SettingsCategory::Printers => "settings-cat-printers",
            SettingsCategory::Power => "settings-cat-power",
            SettingsCategory::Users => "settings-cat-users",
            SettingsCategory::Updates => "settings-cat-updates",
        }
    }

    /// Extra German search terms so the settings search matches intent, not
    /// just the visible category label (e.g. "wlan" -> Netzwerk, "akku" -> Energie).
    pub fn search_keywords(&self) -> &'static [&'static str] {
        match self {
            SettingsCategory::Theme => &[
                "theme",
                "erscheinungsbild",
                "farbe",
                "farben",
                "dunkel",
                "hell",
                "dark",
                "light",
                "akzent",
                "palette",
                "stil",
            ],
            SettingsCategory::Cursor => &["cursor", "zeiger", "maus", "pointer", "größe"],
            SettingsCategory::Wallpaper => &[
                "wallpaper",
                "hintergrund",
                "bild",
                "tapete",
                "desktop",
                "foto",
            ],
            SettingsCategory::PinnedApps => &[
                "pinned",
                "angeheftet",
                "favoriten",
                "panel",
                "dock",
                "verknüpfung",
                "apps",
            ],
            SettingsCategory::DefaultApps => &[
                "standard",
                "default",
                "öffnen",
                "öffnen mit",
                "mime",
                "dateityp",
                "browser",
                "webbrowser",
                "e-mail",
                "mail",
                "texteditor",
                "editor",
                "bilder",
                "video",
                "audio",
                "musik",
                "pdf",
                "archiv",
                "dateimanager",
                "explorer",
            ],
            SettingsCategory::SystemOverview => &[
                "übersicht",
                "overview",
                "system",
                "info",
                "version",
                "gerät",
                "status",
            ],
            SettingsCategory::Display => &[
                "anzeige",
                "display",
                "monitor",
                "auflösung",
                "bildschirm",
                "skalierung",
                "helligkeit",
                "ausgabe",
            ],
            SettingsCategory::Network => &[
                "netzwerk",
                "network",
                "wlan",
                "wifi",
                "w-lan",
                "ethernet",
                "vpn",
                "dns",
                "internet",
                "verbindung",
                "lan",
            ],
            SettingsCategory::Bluetooth => &[
                "bluetooth",
                "funk",
                "kopfhörer",
                "gerät",
                "pairing",
                "koppeln",
                "headset",
            ],
            SettingsCategory::Sound => &[
                "audio",
                "sound",
                "ton",
                "lautstärke",
                "lautsprecher",
                "mikrofon",
                "pipewire",
                "wiedergabe",
            ],
            SettingsCategory::Printers => &["drucker", "printer", "cups", "drucken", "scan"],
            SettingsCategory::Power => &[
                "energie",
                "power",
                "akku",
                "batterie",
                "ruhezustand",
                "suspend",
                "standby",
                "helligkeit",
                "sparmodus",
            ],
            SettingsCategory::Users => &[
                "benutzer",
                "user",
                "konto",
                "anmeldung",
                "login",
                "passwort",
                "yubikey",
                "authentifizierung",
                "account",
            ],
            SettingsCategory::Updates => &[
                "updates",
                "aktualisierung",
                "paket",
                "upgrade",
                "wartung",
                "version",
            ],
        }
    }
}

// ─── Widget-based launcher sub-page ─────────────────────────────────────────

const HEADER_HEIGHT: u32 = 52;
const SIDEBAR_W: u32 = 160;
const SIDEBAR_ROW_H: i32 = 44;

const DIVIDER_HEIGHT: u32 = 2;
const THEME_ROW_H: i32 = 44;
const THEME_ROW_CORNER: i32 = 4;
const PINNED_ROW_H: i32 = 44;
const PINNED_BTN_W: i32 = 30;
const PINNED_MAX: usize = 16;
const DISPLAY_CARD_H: i32 = 104;
const DISPLAY_PRIMARY_BTN_W: i32 = 104;
const DISPLAY_MODE_COMBO_W: i32 = 188;
const DISPLAY_MODE_OPTION_H: i32 = 34;
const DISPLAY_MODE_OPTION_MAX: usize = 8;
const DISPLAY_OUTPUT_MAX: usize = 16;
const PRINTER_SUMMARY_H: i32 = 92;
const PRINTER_ROW_H: i32 = 72;
const PRINTER_MAX: usize = 8;
const SOUND_SUMMARY_H: i32 = 92;
const SOUND_ROW_H: i32 = 72;
const SOUND_MAX: usize = 8;

/// Selectable idle screen-blank timeouts for the Power page, each paired with
/// its static widget id and chip label. `None` = off; the numeric ids carry
/// the timeout in seconds. The ids are matched by
/// `widget_action::action_for_id` (`idle-timeout-off` and the `idle-timeout-`
/// prefix), so they must stay in sync with that parser.
pub(crate) const IDLE_TIMEOUT_OPTIONS: &[(Option<u64>, &str, &str)] = &[
    (None, "idle-timeout-off", "Aus"),
    (Some(60), "idle-timeout-60", "1 min"),
    (Some(300), "idle-timeout-300", "5 min"),
    (Some(600), "idle-timeout-600", "10 min"),
    (Some(900), "idle-timeout-900", "15 min"),
    (Some(1800), "idle-timeout-1800", "30 min"),
];

/// Volume preset chips for the default output on the Sound page, paired with
/// their static widget id and label. The ids are matched by
/// `widget_action::action_for_id` (the `vol-set-` prefix), so they must stay
/// in sync with that parser.
pub(crate) const VOLUME_PRESET_OPTIONS: &[(u8, &str, &str)] = &[
    (0, "vol-set-0", "0%"),
    (25, "vol-set-25", "25%"),
    (50, "vol-set-50", "50%"),
    (75, "vol-set-75", "75%"),
    (100, "vol-set-100", "100%"),
];

/// Static widget ids for the output (sink) device rows, indexed by position in
/// the snapshot. A click makes that device the default. Matched by
/// `widget_action::action_for_id` (the `audio-default-out-` prefix); length
/// matches SOUND_MAX so every shown row has an id.
pub(crate) const AUDIO_OUTPUT_IDS: &[&str] = &[
    "audio-default-out-0",
    "audio-default-out-1",
    "audio-default-out-2",
    "audio-default-out-3",
    "audio-default-out-4",
    "audio-default-out-5",
    "audio-default-out-6",
    "audio-default-out-7",
];

/// Static widget ids for the input (source) device rows; see AUDIO_OUTPUT_IDS.
/// Matched by the `audio-default-in-` prefix.
pub(crate) const AUDIO_INPUT_IDS: &[&str] = &[
    "audio-default-in-0",
    "audio-default-in-1",
    "audio-default-in-2",
    "audio-default-in-3",
    "audio-default-in-4",
    "audio-default-in-5",
    "audio-default-in-6",
    "audio-default-in-7",
];

/// Static widget ids for saved network-profile rows, indexed by position. A
/// click activates that profile. Matched by `widget_action::action_for_id`
/// (the `net-connect-` prefix). Length bounds how many profiles are shown.
pub(crate) const NETWORK_PROFILE_IDS: &[&str] = &[
    "net-connect-0",
    "net-connect-1",
    "net-connect-2",
    "net-connect-3",
    "net-connect-4",
    "net-connect-5",
    "net-connect-6",
    "net-connect-7",
];

/// Static widget ids for scanned Wi-Fi rows, indexed by position. A click
/// connects (or opens the password prompt). Matched by
/// `widget_action::action_for_id` (the `wifi-connect-` prefix). Length bounds
/// how many networks are shown.
pub(crate) const WIFI_NETWORK_IDS: &[&str] = &[
    "wifi-connect-0",
    "wifi-connect-1",
    "wifi-connect-2",
    "wifi-connect-3",
    "wifi-connect-4",
    "wifi-connect-5",
    "wifi-connect-6",
    "wifi-connect-7",
    "wifi-connect-8",
    "wifi-connect-9",
];

/// Static widget ids for Bluetooth device rows, indexed by position. A click
/// pairs (or connects, if already paired) that device. Matched by
/// `widget_action::action_for_id` (the `bt-device-` prefix). Length bounds how
/// many devices are shown.
pub(crate) const BT_DEVICE_IDS: &[&str] = &[
    "bt-device-0",
    "bt-device-1",
    "bt-device-2",
    "bt-device-3",
    "bt-device-4",
    "bt-device-5",
    "bt-device-6",
    "bt-device-7",
];

pub(crate) const THEME_WIDGET_IDS: &[&str] = &[
    "settings-theme-0",
    "settings-theme-1",
    "settings-theme-2",
    "settings-theme-3",
    "settings-theme-4",
    "settings-theme-5",
    "settings-theme-6",
    "settings-theme-7",
    "settings-theme-8",
    "settings-theme-9",
    "settings-theme-10",
    "settings-theme-11",
    "settings-theme-12",
    "settings-theme-13",
    "settings-theme-14",
    "settings-theme-15",
    "settings-theme-16",
    "settings-theme-17",
    "settings-theme-18",
    "settings-theme-19",
];

pub(crate) const WALLPAPER_WIDGET_IDS: &[&str] = &[
    "settings-wallpaper-0",
    "settings-wallpaper-1",
    "settings-wallpaper-2",
    "settings-wallpaper-3",
    "settings-wallpaper-4",
    "settings-wallpaper-5",
    "settings-wallpaper-6",
    "settings-wallpaper-7",
    "settings-wallpaper-8",
    "settings-wallpaper-9",
    "settings-wallpaper-10",
    "settings-wallpaper-11",
    "settings-wallpaper-12",
    "settings-wallpaper-13",
    "settings-wallpaper-14",
    "settings-wallpaper-15",
    "settings-wallpaper-16",
    "settings-wallpaper-17",
    "settings-wallpaper-18",
    "settings-wallpaper-19",
    "settings-wallpaper-20",
    "settings-wallpaper-21",
    "settings-wallpaper-22",
    "settings-wallpaper-23",
    "settings-wallpaper-24",
    "settings-wallpaper-25",
    "settings-wallpaper-26",
    "settings-wallpaper-27",
    "settings-wallpaper-28",
    "settings-wallpaper-29",
    "settings-wallpaper-30",
    "settings-wallpaper-31",
    "settings-wallpaper-32",
    "settings-wallpaper-33",
    "settings-wallpaper-34",
    "settings-wallpaper-35",
    "settings-wallpaper-36",
    "settings-wallpaper-37",
    "settings-wallpaper-38",
    "settings-wallpaper-39",
];

include!("settings_view/ids.rs");
include!("settings_view/basic_widgets.rs");
include!("settings_view/appearance_widgets.rs");
include!("settings_view/audio_system_widgets.rs");
include!("settings_view/network_device_widgets.rs");
include!("settings_view/display_widgets.rs");
include!("settings_view/display_controls.rs");
include!("settings_view/content_builders.rs");
include!("settings_view/draw.rs");
