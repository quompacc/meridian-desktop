// settings_view.rs — widget-based settings sub-page for the launcher.

use meridian_ui::{
    effect::{paint_fill, paint_text, rounded_rect_path},
    style::Color,
    ui_length,
    widget::{Button, Container, Widget},
    Rect, Theme, UiSize, WidgetState, WidgetStyle,
};
use tiny_skia::{Pixmap, PixmapMut, PixmapPaint, PixmapRef, Transform};

use crate::audio::{AudioDevice, AudioServiceState, AudioSnapshot};
use crate::icons::{icon_image_to_pixmap, IconCache};
use crate::launcher::DesktopApp;
use crate::panel::PinnedApp;
use crate::power_footer::build_power_footer_buttons;
use crate::printers::{PrinterInfo, PrinterServiceState, PrinterSnapshot};
use meridian_config::{ThemeConfig, WallpaperEntry, WallpaperMode};
use meridian_ipc::{OutputModeState, OutputWorkspaceState};

use crate::ui::tokens::theme_from_config;

// ─── SettingsCategory ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SettingsRootCategory {
    #[default]
    Desktop,
    System,
}

impl SettingsRootCategory {
    pub const ALL: &'static [SettingsRootCategory] =
        &[SettingsRootCategory::Desktop, SettingsRootCategory::System];

    pub fn label(&self) -> &'static str {
        match self {
            SettingsRootCategory::Desktop => "Desktop",
            SettingsRootCategory::System => "System",
        }
    }

    pub fn chip_id(&self) -> &'static str {
        match self {
            SettingsRootCategory::Desktop => "settings-root-desktop",
            SettingsRootCategory::System => "settings-root-system",
        }
    }

    pub fn first_category(&self) -> SettingsCategory {
        match self {
            SettingsRootCategory::Desktop => SettingsCategory::Theme,
            SettingsRootCategory::System => SettingsCategory::SystemOverview,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SettingsCategory {
    #[default]
    Theme,
    Cursor,
    Display,
    Wallpaper,
    PinnedApps,
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

    pub fn root(&self) -> SettingsRootCategory {
        match self {
            SettingsCategory::Theme
            | SettingsCategory::Cursor
            | SettingsCategory::Wallpaper
            | SettingsCategory::PinnedApps => SettingsRootCategory::Desktop,
            SettingsCategory::SystemOverview
            | SettingsCategory::Display
            | SettingsCategory::Network
            | SettingsCategory::Bluetooth
            | SettingsCategory::Sound
            | SettingsCategory::Printers
            | SettingsCategory::Power
            | SettingsCategory::Users
            | SettingsCategory::Updates => SettingsRootCategory::System,
        }
    }

    pub fn all_for_root(root: SettingsRootCategory) -> &'static [SettingsCategory] {
        match root {
            SettingsRootCategory::Desktop => SettingsCategory::DESKTOP,
            SettingsRootCategory::System => SettingsCategory::SYSTEM,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SettingsCategory::Theme => "Theme",
            SettingsCategory::Cursor => "Cursor",
            SettingsCategory::Display => "Display",
            SettingsCategory::Wallpaper => "Wallpaper",
            SettingsCategory::PinnedApps => "Pinned Apps",
            SettingsCategory::SystemOverview => "Overview",
            SettingsCategory::Network => "Network",
            SettingsCategory::Bluetooth => "Bluetooth",
            SettingsCategory::Sound => "Sound",
            SettingsCategory::Printers => "Printers",
            SettingsCategory::Power => "Power",
            SettingsCategory::Users => "Users",
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

    pub fn placeholder(&self) -> &'static str {
        match self {
            SettingsCategory::Theme => "",
            SettingsCategory::Cursor => "Cursor theme + size - coming soon",
            SettingsCategory::Display => "",
            SettingsCategory::Wallpaper => "Wallpaper path + mode - coming soon",
            SettingsCategory::PinnedApps => "Reorder / add / remove pinned apps - coming soon",
            SettingsCategory::SystemOverview => "System overview - coming soon",
            SettingsCategory::Network => "Network status and connections - coming soon",
            SettingsCategory::Bluetooth => "Bluetooth devices and pairing - coming soon",
            SettingsCategory::Sound => "Audio devices and volume routing - coming soon",
            SettingsCategory::Printers => "Printer setup and queue status - coming soon",
            SettingsCategory::Power => "Power mode, suspend, and battery settings - coming soon",
            SettingsCategory::Users => "Users, login, and authentication - coming soon",
            SettingsCategory::Updates => "System update status - coming soon",
        }
    }

    pub fn skeleton_detail(&self) -> &'static str {
        match self {
            SettingsCategory::SystemOverview => {
                "Device summary, Meridian version, session state, and quick health checks."
            }
            SettingsCategory::Network => {
                "Connections, Wi-Fi, Ethernet, VPN, DNS, and connection diagnostics."
            }
            SettingsCategory::Bluetooth => "Adapters, pairing, trusted devices, and input devices.",
            SettingsCategory::Sound => {
                "PipeWire devices, input/output routing, and volume defaults."
            }
            SettingsCategory::Printers => {
                "Printer discovery, CUPS queues, default printer, and print-job status."
            }
            SettingsCategory::Power => {
                "Suspend policy, power profiles, battery status, brightness, and idle behavior."
            }
            SettingsCategory::Users => {
                "Local users, login options, password flow, and YubiKey authentication."
            }
            SettingsCategory::Updates => {
                "Package update status, last check, pending restarts, and maintenance actions."
            }
            _ => self.placeholder(),
        }
    }
}

// ─── Widget-based launcher sub-page ─────────────────────────────────────────

const HEADER_HEIGHT: u32 = 44;
const CHIPS_BAR_HEIGHT: u32 = 44;
const ROOT_CHIP_W: i32 = 120;
const ROOT_CHIP_H: i32 = 32;
const SIDEBAR_W: u32 = 160;
const SIDEBAR_ROW_H: i32 = 44;
const FOOTER_HEIGHT: u32 = 56;
const FOOTER_SWITCH_WIDTH: i32 = 144;
const FOOTER_SWITCH_HEIGHT: i32 = 48;
const FOOTER_POWER_BUTTON_SIZE: i32 = 48;
const FOOTER_PADDING_X: i32 = 28;
const FOOTER_CLUSTER_GAP: i32 = 8;
const POWER_ICON_SIZE: u32 = 32;
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
const SYSTEM_CARD_H: i32 = 116;
const PRINTER_SUMMARY_H: i32 = 92;
const PRINTER_ROW_H: i32 = 72;
const PRINTER_MAX: usize = 8;
const SOUND_SUMMARY_H: i32 = 92;
const SOUND_ROW_H: i32 = 72;
const SOUND_MAX: usize = 8;

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

const PINNED_UP_IDS: [&str; 16] = [
    "pinned-move-up-0",
    "pinned-move-up-1",
    "pinned-move-up-2",
    "pinned-move-up-3",
    "pinned-move-up-4",
    "pinned-move-up-5",
    "pinned-move-up-6",
    "pinned-move-up-7",
    "pinned-move-up-8",
    "pinned-move-up-9",
    "pinned-move-up-10",
    "pinned-move-up-11",
    "pinned-move-up-12",
    "pinned-move-up-13",
    "pinned-move-up-14",
    "pinned-move-up-15",
];
const PINNED_DN_IDS: [&str; 16] = [
    "pinned-move-dn-0",
    "pinned-move-dn-1",
    "pinned-move-dn-2",
    "pinned-move-dn-3",
    "pinned-move-dn-4",
    "pinned-move-dn-5",
    "pinned-move-dn-6",
    "pinned-move-dn-7",
    "pinned-move-dn-8",
    "pinned-move-dn-9",
    "pinned-move-dn-10",
    "pinned-move-dn-11",
    "pinned-move-dn-12",
    "pinned-move-dn-13",
    "pinned-move-dn-14",
    "pinned-move-dn-15",
];
const PINNED_RM_IDS: [&str; 16] = [
    "pinned-remove-0",
    "pinned-remove-1",
    "pinned-remove-2",
    "pinned-remove-3",
    "pinned-remove-4",
    "pinned-remove-5",
    "pinned-remove-6",
    "pinned-remove-7",
    "pinned-remove-8",
    "pinned-remove-9",
    "pinned-remove-10",
    "pinned-remove-11",
    "pinned-remove-12",
    "pinned-remove-13",
    "pinned-remove-14",
    "pinned-remove-15",
];
const PINNED_ADD_IDS: [&str; 16] = [
    "pinned-add-app-0",
    "pinned-add-app-1",
    "pinned-add-app-2",
    "pinned-add-app-3",
    "pinned-add-app-4",
    "pinned-add-app-5",
    "pinned-add-app-6",
    "pinned-add-app-7",
    "pinned-add-app-8",
    "pinned-add-app-9",
    "pinned-add-app-10",
    "pinned-add-app-11",
    "pinned-add-app-12",
    "pinned-add-app-13",
    "pinned-add-app-14",
    "pinned-add-app-15",
];
const DISPLAY_PRIMARY_IDS: [&str; 16] = [
    "display-primary-0",
    "display-primary-1",
    "display-primary-2",
    "display-primary-3",
    "display-primary-4",
    "display-primary-5",
    "display-primary-6",
    "display-primary-7",
    "display-primary-8",
    "display-primary-9",
    "display-primary-10",
    "display-primary-11",
    "display-primary-12",
    "display-primary-13",
    "display-primary-14",
    "display-primary-15",
];
const DISPLAY_MODE_TOGGLE_IDS: [&str; 16] = [
    "display-mode-toggle-0",
    "display-mode-toggle-1",
    "display-mode-toggle-2",
    "display-mode-toggle-3",
    "display-mode-toggle-4",
    "display-mode-toggle-5",
    "display-mode-toggle-6",
    "display-mode-toggle-7",
    "display-mode-toggle-8",
    "display-mode-toggle-9",
    "display-mode-toggle-10",
    "display-mode-toggle-11",
    "display-mode-toggle-12",
    "display-mode-toggle-13",
    "display-mode-toggle-14",
    "display-mode-toggle-15",
];
const DISPLAY_MODE_OPTION_IDS: [[&str; DISPLAY_MODE_OPTION_MAX]; 16] = [
    [
        "display-mode-select-0-0",
        "display-mode-select-0-1",
        "display-mode-select-0-2",
        "display-mode-select-0-3",
        "display-mode-select-0-4",
        "display-mode-select-0-5",
        "display-mode-select-0-6",
        "display-mode-select-0-7",
    ],
    [
        "display-mode-select-1-0",
        "display-mode-select-1-1",
        "display-mode-select-1-2",
        "display-mode-select-1-3",
        "display-mode-select-1-4",
        "display-mode-select-1-5",
        "display-mode-select-1-6",
        "display-mode-select-1-7",
    ],
    [
        "display-mode-select-2-0",
        "display-mode-select-2-1",
        "display-mode-select-2-2",
        "display-mode-select-2-3",
        "display-mode-select-2-4",
        "display-mode-select-2-5",
        "display-mode-select-2-6",
        "display-mode-select-2-7",
    ],
    [
        "display-mode-select-3-0",
        "display-mode-select-3-1",
        "display-mode-select-3-2",
        "display-mode-select-3-3",
        "display-mode-select-3-4",
        "display-mode-select-3-5",
        "display-mode-select-3-6",
        "display-mode-select-3-7",
    ],
    [
        "display-mode-select-4-0",
        "display-mode-select-4-1",
        "display-mode-select-4-2",
        "display-mode-select-4-3",
        "display-mode-select-4-4",
        "display-mode-select-4-5",
        "display-mode-select-4-6",
        "display-mode-select-4-7",
    ],
    [
        "display-mode-select-5-0",
        "display-mode-select-5-1",
        "display-mode-select-5-2",
        "display-mode-select-5-3",
        "display-mode-select-5-4",
        "display-mode-select-5-5",
        "display-mode-select-5-6",
        "display-mode-select-5-7",
    ],
    [
        "display-mode-select-6-0",
        "display-mode-select-6-1",
        "display-mode-select-6-2",
        "display-mode-select-6-3",
        "display-mode-select-6-4",
        "display-mode-select-6-5",
        "display-mode-select-6-6",
        "display-mode-select-6-7",
    ],
    [
        "display-mode-select-7-0",
        "display-mode-select-7-1",
        "display-mode-select-7-2",
        "display-mode-select-7-3",
        "display-mode-select-7-4",
        "display-mode-select-7-5",
        "display-mode-select-7-6",
        "display-mode-select-7-7",
    ],
    [
        "display-mode-select-8-0",
        "display-mode-select-8-1",
        "display-mode-select-8-2",
        "display-mode-select-8-3",
        "display-mode-select-8-4",
        "display-mode-select-8-5",
        "display-mode-select-8-6",
        "display-mode-select-8-7",
    ],
    [
        "display-mode-select-9-0",
        "display-mode-select-9-1",
        "display-mode-select-9-2",
        "display-mode-select-9-3",
        "display-mode-select-9-4",
        "display-mode-select-9-5",
        "display-mode-select-9-6",
        "display-mode-select-9-7",
    ],
    [
        "display-mode-select-10-0",
        "display-mode-select-10-1",
        "display-mode-select-10-2",
        "display-mode-select-10-3",
        "display-mode-select-10-4",
        "display-mode-select-10-5",
        "display-mode-select-10-6",
        "display-mode-select-10-7",
    ],
    [
        "display-mode-select-11-0",
        "display-mode-select-11-1",
        "display-mode-select-11-2",
        "display-mode-select-11-3",
        "display-mode-select-11-4",
        "display-mode-select-11-5",
        "display-mode-select-11-6",
        "display-mode-select-11-7",
    ],
    [
        "display-mode-select-12-0",
        "display-mode-select-12-1",
        "display-mode-select-12-2",
        "display-mode-select-12-3",
        "display-mode-select-12-4",
        "display-mode-select-12-5",
        "display-mode-select-12-6",
        "display-mode-select-12-7",
    ],
    [
        "display-mode-select-13-0",
        "display-mode-select-13-1",
        "display-mode-select-13-2",
        "display-mode-select-13-3",
        "display-mode-select-13-4",
        "display-mode-select-13-5",
        "display-mode-select-13-6",
        "display-mode-select-13-7",
    ],
    [
        "display-mode-select-14-0",
        "display-mode-select-14-1",
        "display-mode-select-14-2",
        "display-mode-select-14-3",
        "display-mode-select-14-4",
        "display-mode-select-14-5",
        "display-mode-select-14-6",
        "display-mode-select-14-7",
    ],
    [
        "display-mode-select-15-0",
        "display-mode-select-15-1",
        "display-mode-select-15-2",
        "display-mode-select-15-3",
        "display-mode-select-15-4",
        "display-mode-select-15-5",
        "display-mode-select-15-6",
        "display-mode-select-15-7",
    ],
];

struct SettingsHeader {
    width: i32,
}

impl Widget for SettingsHeader {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length(HEADER_HEIGHT as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, 0) {
            paint_fill(canvas, &path, theme.palette.surface);
        }
        paint_text(
            canvas,
            "Settings",
            area.x + 20,
            area.y + area.height - 12,
            13.0,
            theme.palette.text_dim,
        );
        // Thin accent underline
        let strip = Rect {
            x: area.x + 20,
            y: area.y + area.height - 2,
            width: 52,
            height: 2,
        };
        if let Some(path) = rounded_rect_path(strip, 0) {
            paint_fill(canvas, &path, theme.palette.accent);
        }
    }
}

struct SettingsSidebarRow {
    cat: SettingsCategory,
    is_selected: bool,
    accent: Color,
    row_width: i32,
}

impl Widget for SettingsSidebarRow {
    fn id(&self) -> Option<&'static str> {
        Some(self.cat.chip_id())
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(SIDEBAR_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = match state {
            WidgetState::Idle if self.is_selected => theme.palette.surface_alt,
            WidgetState::Idle => theme.palette.surface,
            WidgetState::Hovered => theme
                .palette
                .surface
                .lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.10),
            WidgetState::Pressed => theme.palette.surface.lerp(Color::rgb(0, 0, 0), 0.15),
        };
        if let Some(path) = rounded_rect_path(area, 0) {
            paint_fill(canvas, &path, bg);
        }
        if self.is_selected {
            let strip = Rect {
                x: area.x,
                y: area.y + 8,
                width: 3,
                height: area.height - 16,
            };
            if let Some(path) = rounded_rect_path(strip, 1) {
                paint_fill(canvas, &path, self.accent);
            }
        }
        let text_color = if self.is_selected {
            self.accent
        } else {
            theme.palette.text
        };
        paint_text(
            canvas,
            self.cat.label(),
            area.x + 14,
            area.y + area.height - 14,
            12.5,
            text_color,
        );
    }
}

struct VerticalDivider {
    height: i32,
    color: Color,
}

impl Widget for VerticalDivider {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(1.0),
                height: ui_length(self.height as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, _theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, 0) {
            paint_fill(canvas, &path, self.color);
        }
    }
}

struct ThemeRow {
    index: usize,
    name: Box<str>,
    is_selected: bool,
    accent: Color,
    row_width: i32,
}

impl Widget for ThemeRow {
    fn id(&self) -> Option<&'static str> {
        THEME_WIDGET_IDS.get(self.index).copied()
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(THEME_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = match state {
            WidgetState::Idle => {
                if self.is_selected {
                    theme
                        .palette
                        .surface
                        .lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.08)
                } else {
                    theme.palette.surface
                }
            }
            WidgetState::Hovered => theme
                .palette
                .surface
                .lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.14),
            WidgetState::Pressed => theme.palette.surface.lerp(Color::rgb(0, 0, 0), 0.18),
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        if self.is_selected {
            let strip = Rect {
                x: area.x + 4,
                y: area.y + 8,
                width: 3,
                height: area.height - 16,
            };
            if let Some(path) = rounded_rect_path(strip, 1) {
                paint_fill(canvas, &path, self.accent);
            }
        }
        let text_color = if self.is_selected {
            self.accent
        } else {
            theme.palette.text
        };
        paint_text(
            canvas,
            &self.name,
            area.x + 16,
            area.y + area.height - 14,
            13.0,
            text_color,
        );
    }
}

const WALLPAPER_MODE_BAR_H: u32 = 52;
const WALLPAPER_ROW_H: i32 = 64;
const WALLPAPER_THUMB_W: u32 = 96;
const WALLPAPER_THUMB_H: u32 = 54;

struct WallpaperRow {
    index: usize,
    display_name: Box<str>,
    thumbnail: Option<(u32, u32, Vec<u8>)>,
    is_selected: bool,
    accent: Color,
    row_width: i32,
}

impl Widget for WallpaperRow {
    fn id(&self) -> Option<&'static str> {
        WALLPAPER_WIDGET_IDS.get(self.index).copied()
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(WALLPAPER_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = match state {
            WidgetState::Idle => {
                if self.is_selected {
                    theme
                        .palette
                        .surface
                        .lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.08)
                } else {
                    theme.palette.surface
                }
            }
            WidgetState::Hovered => theme
                .palette
                .surface
                .lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.12),
            WidgetState::Pressed => theme.palette.surface.lerp(Color::rgb(0, 0, 0), 0.18),
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        if self.is_selected {
            let strip = Rect {
                x: area.x + 4,
                y: area.y + 6,
                width: 3,
                height: area.height - 12,
            };
            if let Some(path) = rounded_rect_path(strip, 1) {
                paint_fill(canvas, &path, self.accent);
            }
        }
        // Thumbnail on the left (96x54 inside 64px-tall row).
        let thumb_left = area.x + 8;
        if let Some((tw, th, ref data)) = self.thumbnail {
            let thumb_y = area.y + (area.height - th as i32) / 2;
            if let Some(pm_ref) = PixmapRef::from_bytes(data, tw, th) {
                canvas.draw_pixmap(
                    thumb_left,
                    thumb_y,
                    pm_ref,
                    &PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
            }
        } else {
            // Placeholder rectangle when thumbnail not yet loaded.
            let ph = Rect {
                x: thumb_left,
                y: area.y + 5,
                width: WALLPAPER_THUMB_W as i32,
                height: WALLPAPER_THUMB_H as i32,
            };
            if let Some(path) = rounded_rect_path(ph, 2) {
                paint_fill(canvas, &path, theme.palette.surface_alt);
            }
        }
        let text_x = area.x + 8 + WALLPAPER_THUMB_W as i32 + 8;
        let text_color = if self.is_selected {
            self.accent
        } else {
            theme.palette.text
        };
        paint_text(
            canvas,
            &self.display_name,
            text_x,
            area.y + area.height - 16,
            12.0,
            text_color,
        );
    }
}

struct WallpaperBrowseRow {
    row_width: i32,
    accent: Color,
}

impl Widget for WallpaperBrowseRow {
    fn id(&self) -> Option<&'static str> {
        Some("wallpaper-browse")
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(WALLPAPER_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = match state {
            WidgetState::Idle => theme.palette.surface,
            WidgetState::Hovered => theme
                .palette
                .surface
                .lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.12),
            WidgetState::Pressed => theme.palette.surface.lerp(Color::rgb(0, 0, 0), 0.18),
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        // Icon area — dim filled rectangle with "..." hint.
        let icon = Rect {
            x: area.x + 8,
            y: area.y + (area.height - WALLPAPER_THUMB_H as i32) / 2,
            width: WALLPAPER_THUMB_W as i32,
            height: WALLPAPER_THUMB_H as i32,
        };
        if let Some(path) = rounded_rect_path(icon, 4) {
            paint_fill(canvas, &path, self.accent.lerp(Color::rgb(0, 0, 0), 0.55));
        }
        paint_text(
            canvas,
            "\u{2026}",
            icon.x + icon.width / 2 - 6,
            icon.y + icon.height - 8,
            14.0,
            self.accent,
        );
        let text_x = area.x + 8 + WALLPAPER_THUMB_W as i32 + 8;
        paint_text(
            canvas,
            "Browse for image\u{2026}",
            text_x,
            area.y + area.height - 16,
            12.0,
            self.accent,
        );
    }
}

struct PinnedAppLabel {
    label: Box<str>,
    program: Box<str>,
    width: i32,
    icon: Option<Pixmap>,
}

impl Widget for PinnedAppLabel {
    fn id(&self) -> Option<&'static str> {
        None
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length(PINNED_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, 0) {
            paint_fill(canvas, &path, theme.palette.surface);
        }
        let text_x = if let Some(ref icon) = self.icon {
            let iw = icon.width() as i32;
            let ih = icon.height() as i32;
            let ix = area.x + 6;
            let iy = area.y + (area.height - ih) / 2;
            canvas.draw_pixmap(
                ix,
                iy,
                icon.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
            area.x + 6 + iw + 6
        } else {
            area.x + 10
        };
        paint_text(
            canvas,
            &self.label,
            text_x,
            area.y + 16,
            13.0,
            theme.palette.text,
        );
        paint_text(
            canvas,
            &self.program,
            text_x,
            area.y + 32,
            11.0,
            theme.palette.text_dim,
        );
    }
}

struct SettingsPlaceholder {
    width: i32,
    text: &'static str,
}

impl Widget for SettingsPlaceholder {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length(60.0),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        paint_text(
            canvas,
            self.text,
            area.x + 20,
            area.y + 36,
            13.0,
            theme.palette.text_dim,
        );
    }
}

struct SettingsSkeletonCard {
    title: &'static str,
    detail: &'static str,
    row_width: i32,
    accent: Color,
}

impl Widget for SettingsSkeletonCard {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(SYSTEM_CARD_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, theme.palette.surface);
        }

        let strip = Rect {
            x: area.x,
            y: area.y + 10,
            width: 3,
            height: area.height - 20,
        };
        if let Some(path) = rounded_rect_path(strip, 1) {
            paint_fill(canvas, &path, self.accent);
        }

        paint_text(
            canvas,
            self.title,
            area.x + 18,
            area.y + 32,
            14.0,
            theme.palette.text,
        );
        paint_text(
            canvas,
            "COMING SOON",
            area.x + 18,
            area.y + 56,
            10.5,
            self.accent,
        );
        paint_text(
            canvas,
            self.detail,
            area.x + 18,
            area.y + 86,
            12.0,
            theme.palette.text_dim,
        );
    }
}

struct SoundSummaryCard {
    snapshot: AudioSnapshot,
    row_width: i32,
    accent: Color,
}

impl Widget for SoundSummaryCard {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(SOUND_SUMMARY_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, theme.palette.surface);
        }
        let service_text = match self.snapshot.service {
            AudioServiceState::Running => "PIPEWIRE RUNNING",
            AudioServiceState::Unavailable => "AUDIO UNAVAILABLE",
        };
        let status_color = match self.snapshot.service {
            AudioServiceState::Running => self.accent,
            AudioServiceState::Unavailable => theme.palette.error,
        };
        paint_text(
            canvas,
            "Sound",
            area.x + 18,
            area.y + 30,
            14.0,
            theme.palette.text,
        );
        paint_text(
            canvas,
            service_text,
            area.x + 18,
            area.y + 52,
            10.5,
            status_color,
        );

        let output_text = self
            .snapshot
            .default_output
            .as_ref()
            .map(|device| format!("Output: {}", fit_text(&device.name, 40)))
            .unwrap_or_else(|| "Output: none".to_string());
        let count_text = format!(
            "{} outputs / {} inputs",
            self.snapshot.outputs.len(),
            self.snapshot.inputs.len()
        );
        paint_text(
            canvas,
            &output_text,
            area.x + 18,
            area.y + 76,
            12.0,
            theme.palette.text_dim,
        );
        paint_text(
            canvas,
            &count_text,
            area.x + area.width - 170,
            area.y + 76,
            12.0,
            theme.palette.text_dim,
        );
    }
}

struct SoundDeviceRow {
    label: &'static str,
    device: AudioDevice,
    row_width: i32,
    accent: Color,
}

impl Widget for SoundDeviceRow {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(SOUND_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, theme.palette.surface);
        }
        if self.device.is_default {
            let strip = Rect {
                x: area.x,
                y: area.y + 8,
                width: 3,
                height: area.height - 16,
            };
            if let Some(path) = rounded_rect_path(strip, 1) {
                paint_fill(canvas, &path, self.accent);
            }
        }

        let title = format!("{} {}", self.label, fit_text(&self.device.name, 42));
        paint_text(
            canvas,
            &title,
            area.x + 18,
            area.y + 26,
            13.5,
            theme.palette.text,
        );
        if self.device.is_default {
            paint_text(
                canvas,
                "DEFAULT",
                area.x + area.width - 88,
                area.y + 26,
                10.5,
                self.accent,
            );
        }

        let volume = self
            .device
            .volume_percent
            .map(|value| format!("{}%", value))
            .unwrap_or_else(|| "volume unknown".to_string());
        let detail = if self.device.muted {
            format!("id {} / {} / muted", self.device.id, volume)
        } else {
            format!("id {} / {}", self.device.id, volume)
        };
        paint_text(
            canvas,
            &detail,
            area.x + 18,
            area.y + 52,
            12.0,
            theme.palette.text_dim,
        );
    }
}

struct PrinterSummaryCard {
    snapshot: PrinterSnapshot,
    row_width: i32,
    accent: Color,
}

impl Widget for PrinterSummaryCard {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(PRINTER_SUMMARY_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, theme.palette.surface);
        }

        let service_text = match self.snapshot.service {
            PrinterServiceState::Running => "CUPS RUNNING",
            PrinterServiceState::Stopped => "CUPS STOPPED",
            PrinterServiceState::Unavailable => "CUPS UNAVAILABLE",
        };
        let status_color = match self.snapshot.service {
            PrinterServiceState::Running => self.accent,
            PrinterServiceState::Stopped | PrinterServiceState::Unavailable => theme.palette.error,
        };
        paint_text(
            canvas,
            "Printers",
            area.x + 18,
            area.y + 30,
            14.0,
            theme.palette.text,
        );
        paint_text(
            canvas,
            service_text,
            area.x + 18,
            area.y + 52,
            10.5,
            status_color,
        );

        let default_text = self
            .snapshot
            .default_printer
            .as_deref()
            .map(|name| format!("Default: {}", fit_text(name, 44)))
            .unwrap_or_else(|| "Default: none".to_string());
        let queue_text = format!(
            "{} configured / {} queued",
            self.snapshot.printers.len(),
            self.snapshot.job_count
        );
        paint_text(
            canvas,
            &default_text,
            area.x + 18,
            area.y + 76,
            12.0,
            theme.palette.text_dim,
        );
        paint_text(
            canvas,
            &queue_text,
            area.x + area.width - 190,
            area.y + 76,
            12.0,
            theme.palette.text_dim,
        );
    }
}

struct PrinterRow {
    printer: PrinterInfo,
    row_width: i32,
    accent: Color,
}

impl Widget for PrinterRow {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(PRINTER_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, theme.palette.surface);
        }
        if self.printer.is_default {
            let strip = Rect {
                x: area.x,
                y: area.y + 8,
                width: 3,
                height: area.height - 16,
            };
            if let Some(path) = rounded_rect_path(strip, 1) {
                paint_fill(canvas, &path, self.accent);
            }
        }

        let title = fit_text(&self.printer.name, 42);
        paint_text(
            canvas,
            &title,
            area.x + 18,
            area.y + 26,
            13.5,
            theme.palette.text,
        );
        if self.printer.is_default {
            paint_text(
                canvas,
                "DEFAULT",
                area.x + area.width - 88,
                area.y + 26,
                10.5,
                self.accent,
            );
        }

        let accepting = match self.printer.accepting {
            Some(true) => "accepting",
            Some(false) => "not accepting",
            None => "accepting unknown",
        };
        let state = if self.printer.enabled {
            "enabled"
        } else {
            "disabled"
        };
        let detail = format!(
            "{} / {} / {} jobs / {}",
            state,
            accepting,
            self.printer.job_count,
            fit_text(&self.printer.status, 48)
        );
        paint_text(
            canvas,
            &detail,
            area.x + 18,
            area.y + 52,
            12.0,
            theme.palette.text_dim,
        );
    }
}

fn fit_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

struct DisplayOutputRow {
    output_id: u32,
    name: Box<str>,
    workspace: usize,
    primary: bool,
    focused: bool,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    scale_millis: u32,
    transform: Option<Box<str>>,
    refresh_millihz: Option<i32>,
    mode_count: usize,
    row_width: i32,
    accent: Color,
}

impl Widget for DisplayOutputRow {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(DISPLAY_CARD_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        let bg = if self.focused {
            theme.palette.surface.lerp(self.accent, 0.08)
        } else {
            theme.palette.surface
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }

        let monitor_color = if self.focused {
            self.accent
        } else {
            theme.palette.text_dim
        };
        let screen = Rect {
            x: area.x + 16,
            y: area.y + 18,
            width: 78,
            height: 48,
        };
        if let Some(path) = rounded_rect_path(screen, 4) {
            paint_fill(canvas, &path, monitor_color);
        }
        let inner = Rect {
            x: screen.x + 4,
            y: screen.y + 4,
            width: screen.width - 8,
            height: screen.height - 8,
        };
        if let Some(path) = rounded_rect_path(inner, 2) {
            paint_fill(canvas, &path, theme.palette.background);
        }
        let preview = Rect {
            x: inner.x + 6,
            y: inner.y + 6,
            width: inner.width - 12,
            height: inner.height - 12,
        };
        if let Some(path) = rounded_rect_path(preview, 1) {
            paint_fill(canvas, &path, bg.lerp(monitor_color, 0.18));
        }
        let stand = Rect {
            x: screen.x + 34,
            y: screen.y + screen.height,
            width: 10,
            height: 10,
        };
        if let Some(path) = rounded_rect_path(stand, 1) {
            paint_fill(canvas, &path, monitor_color);
        }
        let foot = Rect {
            x: screen.x + 24,
            y: screen.y + screen.height + 9,
            width: 30,
            height: 4,
        };
        if let Some(path) = rounded_rect_path(foot, 2) {
            paint_fill(canvas, &path, monitor_color);
        }

        paint_text(
            canvas,
            &self.name,
            area.x + 112,
            area.y + 26,
            14.0,
            if self.focused {
                self.accent
            } else {
                theme.palette.text
            },
        );

        let badges = display_badge_text(self.focused, self.primary);
        paint_text(
            canvas,
            &badges,
            area.x + 112,
            area.y + 45,
            11.0,
            if self.focused {
                self.accent
            } else {
                theme.palette.text_dim
            },
        );

        let refresh = self
            .refresh_millihz
            .map(|millihz| format!("{:.2} Hz", millihz as f32 / 1000.0))
            .unwrap_or_else(|| "refresh n/a".to_string());
        let transform = self.transform.as_deref().unwrap_or("transform n/a");
        let mode = if self.width > 0 && self.height > 0 {
            format!("{} x {} @ {}", self.width, self.height, refresh)
        } else {
            "geometry n/a".to_string()
        };
        paint_text(
            canvas,
            &mode,
            area.x + 112,
            area.y + 65,
            12.0,
            theme.palette.text,
        );

        let details = if self.width > 0 && self.height > 0 {
            format!(
                "pos {},{} · scale {:.2} · {} · workspace {} · id {} · {} modes",
                self.x,
                self.y,
                self.scale_millis as f32 / 1000.0,
                transform,
                self.workspace.clamp(1, 9),
                self.output_id,
                self.mode_count
            )
        } else {
            format!(
                "scale {:.2} · {} · workspace {} · id {} · {} modes",
                self.scale_millis as f32 / 1000.0,
                transform,
                self.workspace.clamp(1, 9),
                self.output_id,
                self.mode_count
            )
        };
        paint_text(
            canvas,
            &details,
            area.x + 112,
            area.y + 84,
            10.0,
            theme.palette.text_dim,
        );
    }
}

fn display_badge_text(focused: bool, primary: bool) -> String {
    match (focused, primary) {
        (true, true) => "FOCUSED   PRIMARY".to_string(),
        (true, false) => "FOCUSED".to_string(),
        (false, true) => "PRIMARY".to_string(),
        (false, false) => "AVAILABLE".to_string(),
    }
}

fn display_mode_label(mode: &OutputModeState) -> String {
    let refresh = mode
        .refresh_millihz
        .map(|millihz| format!("{:.2} Hz", millihz as f32 / 1000.0))
        .unwrap_or_else(|| "Hz n/a".to_string());
    let suffix = if mode.preferred { " pref" } else { "" };
    format!("{} x {} @ {}{}", mode.width, mode.height, refresh, suffix)
}

fn selected_display_mode(output: &OutputWorkspaceState) -> Option<&OutputModeState> {
    output
        .modes
        .iter()
        .find(|mode| mode.current)
        .or_else(|| {
            output.modes.iter().find(|mode| {
                mode.width == output.width
                    && mode.height == output.height
                    && mode.refresh_millihz == output.refresh_millihz
            })
        })
        .or_else(|| output.modes.first())
}

struct DisplayModeComboButton {
    index: usize,
    label: Box<str>,
    expanded: bool,
    enabled: bool,
    accent: Color,
}

impl Widget for DisplayModeComboButton {
    fn id(&self) -> Option<&'static str> {
        if self.enabled {
            DISPLAY_MODE_TOGGLE_IDS.get(self.index).copied()
        } else {
            None
        }
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(DISPLAY_MODE_COMBO_W as f32),
                height: ui_length(DISPLAY_CARD_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let base = if self.expanded {
            theme.palette.surface_alt.lerp(self.accent, 0.10)
        } else if self.enabled {
            theme.palette.surface_alt
        } else {
            theme.palette.surface
        };
        let bg = match state {
            WidgetState::Idle => base,
            WidgetState::Hovered if self.enabled => base.lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.10),
            WidgetState::Pressed if self.enabled => base.lerp(Color::rgb(0, 0, 0), 0.16),
            _ => base,
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        let color = if self.enabled {
            theme.palette.text
        } else {
            theme.palette.text_dim
        };
        paint_text(
            canvas,
            "Mode",
            area.x + 14,
            area.y + 28,
            11.0,
            theme.palette.text_dim,
        );
        paint_text(
            canvas,
            &fit_text(&self.label, 23),
            area.x + 14,
            area.y + 58,
            11.0,
            color,
        );
        paint_text(
            canvas,
            if self.expanded { "^" } else { "v" },
            area.x + area.width - 22,
            area.y + 58,
            13.0,
            if self.enabled {
                self.accent
            } else {
                theme.palette.text_dim
            },
        );
    }
}

struct DisplayModeOptionRow {
    output_index: usize,
    mode_index: usize,
    label: Box<str>,
    selected: bool,
    row_width: i32,
    accent: Color,
}

impl Widget for DisplayModeOptionRow {
    fn id(&self) -> Option<&'static str> {
        DISPLAY_MODE_OPTION_IDS
            .get(self.output_index)
            .and_then(|ids| ids.get(self.mode_index))
            .copied()
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(DISPLAY_MODE_OPTION_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let base = if self.selected {
            theme.palette.surface_alt.lerp(self.accent, 0.14)
        } else {
            theme.palette.surface_alt
        };
        let bg = match state {
            WidgetState::Idle => base,
            WidgetState::Hovered => base.lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.10),
            WidgetState::Pressed => base.lerp(Color::rgb(0, 0, 0), 0.16),
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        paint_text(
            canvas,
            if self.selected { "*" } else { "" },
            area.x + 14,
            area.y + 23,
            12.0,
            self.accent,
        );
        paint_text(
            canvas,
            &fit_text(&self.label, 58),
            area.x + 34,
            area.y + 23,
            12.0,
            if self.selected {
                self.accent
            } else {
                theme.palette.text
            },
        );
    }
}

struct DisplayPrimaryButton {
    index: usize,
    active: bool,
    accent: Color,
}

impl Widget for DisplayPrimaryButton {
    fn id(&self) -> Option<&'static str> {
        if self.active {
            None
        } else {
            DISPLAY_PRIMARY_IDS.get(self.index).copied()
        }
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(DISPLAY_PRIMARY_BTN_W as f32),
                height: ui_length(DISPLAY_CARD_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let base = if self.active {
            theme.palette.surface_alt.lerp(self.accent, 0.12)
        } else {
            theme.palette.surface_alt
        };
        let bg = match state {
            WidgetState::Idle => base,
            WidgetState::Hovered => base.lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.10),
            WidgetState::Pressed => base.lerp(Color::rgb(0, 0, 0), 0.16),
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }

        let dot = Rect {
            x: area.x + area.width / 2 - 5,
            y: area.y + 18,
            width: 10,
            height: 10,
        };
        if let Some(path) = rounded_rect_path(dot, 5) {
            paint_fill(
                canvas,
                &path,
                if self.active {
                    self.accent
                } else {
                    theme.palette.text_dim
                },
            );
        }
        let color = if self.active {
            self.accent
        } else {
            theme.palette.text
        };
        paint_text(
            canvas,
            if self.active { "Primary" } else { "Make" },
            area.x + 18,
            area.y + 55,
            12.0,
            color,
        );
        paint_text(
            canvas,
            if self.active { "active" } else { "Primary" },
            area.x + 18,
            area.y + 73,
            12.0,
            color,
        );
    }
}

struct AddAppRow {
    index: usize,
    name: Box<str>,
    accent: Color,
    row_width: i32,
    icon: Option<Pixmap>,
}

impl Widget for AddAppRow {
    fn id(&self) -> Option<&'static str> {
        PINNED_ADD_IDS.get(self.index).copied()
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(PINNED_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = match state {
            WidgetState::Idle => theme.palette.surface_alt,
            WidgetState::Hovered => theme
                .palette
                .surface_alt
                .lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.10),
            WidgetState::Pressed => theme.palette.surface_alt.lerp(Color::rgb(0, 0, 0), 0.15),
        };
        if let Some(path) = rounded_rect_path(area, 0) {
            paint_fill(canvas, &path, bg);
        }
        let text_x = if let Some(ref icon) = self.icon {
            let iw = icon.width() as i32;
            let ih = icon.height() as i32;
            let ix = area.x + 6;
            let iy = area.y + (area.height - ih) / 2;
            canvas.draw_pixmap(
                ix,
                iy,
                icon.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
            area.x + 6 + iw + 6
        } else {
            area.x + 10
        };
        paint_text(
            canvas,
            &self.name,
            text_x,
            area.y + area.height - 14,
            13.0,
            self.accent,
        );
    }
}

struct Divider {
    width: i32,
    color: Color,
}

impl Widget for Divider {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length(DIVIDER_HEIGHT as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, _theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, 0) {
            paint_fill(canvas, &path, self.color);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_settings_widget_tree(
    width: u32,
    height: u32,
    selected: SettingsCategory,
    available_themes: &[String],
    current_theme: &str,
    available_wallpapers: &[WallpaperEntry],
    wallpaper_thumbnails: &[Option<(u32, u32, Vec<u8>)>],
    current_wallpaper: Option<&str>,
    wallpaper_mode: WallpaperMode,
    pinned_apps: &[PinnedApp],
    output_workspaces: &[OutputWorkspaceState],
    display_mode_dropdown_open: Option<usize>,
    printer_snapshot: &PrinterSnapshot,
    audio_snapshot: &AudioSnapshot,
    pinned_adding: bool,
    all_apps: &[DesktopApp],
    icon_cache: &IconCache,
    armed_power: Option<(&str, f32)>,
    theme: &Theme,
) -> Box<dyn Widget> {
    let pal = theme.palette;

    let header = Box::new(SettingsHeader {
        width: width as i32,
    }) as Box<dyn Widget>;

    let selected_root = selected.root();

    // Root-category chip bar. The content area stays compact while each root
    // owns its own sidebar skeleton.
    let root_chips: Vec<Box<dyn Widget>> = SettingsRootCategory::ALL
        .iter()
        .map(|root| {
            let accent = if *root == selected_root {
                pal.accent
            } else {
                pal.text_dim
            };
            Box::new(Button::with_id(
                root.chip_id(),
                root.label(),
                accent,
                ROOT_CHIP_W,
                ROOT_CHIP_H,
            )) as Box<dyn Widget>
        })
        .collect();
    let chip_bar = Container::centered_viewport(
        width,
        CHIPS_BAR_HEIGHT,
        vec![Box::new(Container::row(8, root_chips)) as Box<dyn Widget>],
    );

    let divider_color = Color::rgba(pal.accent.r, pal.accent.g, pal.accent.b, 180);
    let content_h = height
        .saturating_sub(HEADER_HEIGHT + CHIPS_BAR_HEIGHT + FOOTER_HEIGHT + 2 * DIVIDER_HEIGHT);
    let content_w = width.saturating_sub(SIDEBAR_W + 1);

    // Left sidebar — sub-categories of the selected root category
    let sidebar_rows: Vec<Box<dyn Widget>> = SettingsCategory::all_for_root(selected_root)
        .iter()
        .map(|cat| {
            Box::new(SettingsSidebarRow {
                cat: *cat,
                is_selected: *cat == selected,
                accent: pal.accent,
                row_width: SIDEBAR_W as i32,
            }) as Box<dyn Widget>
        })
        .collect();
    let sidebar = Box::new(Container::centered_viewport(
        SIDEBAR_W,
        content_h,
        vec![Box::new(Container::column(0, sidebar_rows)) as Box<dyn Widget>],
    )) as Box<dyn Widget>;

    let vsep = Box::new(VerticalDivider {
        height: content_h as i32,
        color: divider_color,
    }) as Box<dyn Widget>;

    let content: Box<dyn Widget> = match selected {
        SettingsCategory::Theme => {
            let row_w = content_w as i32;
            let rows: Vec<Box<dyn Widget>> = available_themes
                .iter()
                .take(THEME_WIDGET_IDS.len())
                .enumerate()
                .map(|(i, name)| {
                    Box::new(ThemeRow {
                        index: i,
                        name: name.as_str().into(),
                        is_selected: name.as_str() == current_theme,
                        accent: pal.accent,
                        row_width: row_w,
                    }) as Box<dyn Widget>
                })
                .collect();
            Box::new(Container::centered_viewport(
                content_w,
                content_h,
                vec![Box::new(Container::column(4, rows)) as Box<dyn Widget>],
            ))
        }
        SettingsCategory::Wallpaper => {
            let mode_chips: Vec<Box<dyn Widget>> = [
                ("wallpaper-mode-fill", "Fill", WallpaperMode::Fill),
                ("wallpaper-mode-fit", "Fit", WallpaperMode::Fit),
                ("wallpaper-mode-center", "Center", WallpaperMode::Center),
                ("wallpaper-mode-tile", "Tile", WallpaperMode::Tile),
            ]
            .iter()
            .map(|(id, label, mode)| {
                let accent = if *mode == wallpaper_mode {
                    pal.accent
                } else {
                    pal.surface
                };
                Box::new(Button::with_id(id, label, accent, 80, 32)) as Box<dyn Widget>
            })
            .collect();
            let mode_bar = Container::centered_viewport(
                content_w,
                WALLPAPER_MODE_BAR_H,
                vec![Box::new(Container::row(8, mode_chips)) as Box<dyn Widget>],
            );
            let list_h = content_h.saturating_sub(WALLPAPER_MODE_BAR_H);
            let max_visible = ((list_h + 2) / (WALLPAPER_ROW_H as u32 + 2))
                .min(WALLPAPER_WIDGET_IDS.len() as u32) as usize;
            let row_w = content_w as i32;
            let mut rows: Vec<Box<dyn Widget>> = Vec::new();
            rows.push(Box::new(WallpaperBrowseRow {
                row_width: row_w,
                accent: pal.accent,
            }));
            let entry_slots = max_visible.saturating_sub(1);
            if available_wallpapers.is_empty() {
                rows.push(Box::new(SettingsPlaceholder {
                    width: row_w,
                    text: "No wallpapers found in /usr/share/wallpapers or ~/Pictures",
                }));
            } else {
                for (i, entry) in available_wallpapers.iter().take(entry_slots).enumerate() {
                    let thumbnail = wallpaper_thumbnails.get(i).and_then(|t| t.clone());
                    rows.push(Box::new(WallpaperRow {
                        index: i,
                        display_name: entry.display_name.as_str().into(),
                        thumbnail,
                        is_selected: current_wallpaper == Some(entry.apply_path.as_str()),
                        accent: pal.accent,
                        row_width: row_w,
                    }));
                }
            }
            let wallpaper_list = Container::centered_viewport(
                content_w,
                list_h,
                vec![Box::new(Container::column(2, rows)) as Box<dyn Widget>],
            );
            Box::new(Container::column(
                0,
                vec![
                    Box::new(mode_bar) as Box<dyn Widget>,
                    Box::new(wallpaper_list) as Box<dyn Widget>,
                ],
            ))
        }
        SettingsCategory::Display => {
            let row_w = content_w as i32;
            let mut rows: Vec<Box<dyn Widget>> = Vec::new();
            if output_workspaces.is_empty() {
                rows.push(Box::new(SettingsPlaceholder {
                    width: row_w,
                    text: "No output snapshot received yet",
                }));
            } else {
                for (idx, output) in output_workspaces
                    .iter()
                    .take(DISPLAY_OUTPUT_MAX)
                    .enumerate()
                {
                    let name = output
                        .output_name
                        .as_deref()
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("Output {}", output.output_id));
                    let row = Box::new(DisplayOutputRow {
                        output_id: output.output_id,
                        name: name.into(),
                        workspace: output.active_workspace,
                        primary: output.primary,
                        focused: output.focused,
                        x: output.x,
                        y: output.y,
                        width: output.width,
                        height: output.height,
                        scale_millis: output.scale_millis,
                        transform: output.transform.as_deref().map(Into::into),
                        refresh_millihz: output.refresh_millihz,
                        mode_count: output.modes.len(),
                        row_width: (row_w - DISPLAY_PRIMARY_BTN_W - DISPLAY_MODE_COMBO_W - 16)
                            .max(180),
                        accent: pal.accent,
                    }) as Box<dyn Widget>;
                    let combo_label = selected_display_mode(output)
                        .map(display_mode_label)
                        .unwrap_or_else(|| "No modes".to_string());
                    let expanded = display_mode_dropdown_open == Some(idx);
                    let mode_combo = Box::new(DisplayModeComboButton {
                        index: idx,
                        label: combo_label.into(),
                        expanded,
                        enabled: !output.modes.is_empty(),
                        accent: pal.accent,
                    }) as Box<dyn Widget>;
                    let primary_button = Box::new(DisplayPrimaryButton {
                        index: idx,
                        active: output.primary,
                        accent: pal.accent,
                    }) as Box<dyn Widget>;
                    rows.push(Box::new(Container::row(
                        8,
                        vec![row, mode_combo, primary_button],
                    )));

                    if expanded {
                        for (mode_idx, mode) in output
                            .modes
                            .iter()
                            .take(DISPLAY_MODE_OPTION_MAX)
                            .enumerate()
                        {
                            rows.push(Box::new(DisplayModeOptionRow {
                                output_index: idx,
                                mode_index: mode_idx,
                                label: display_mode_label(mode).into(),
                                selected: mode.current,
                                row_width: row_w,
                                accent: pal.accent,
                            }));
                        }
                    }
                }
            }
            Box::new(Container::centered_viewport(
                content_w,
                content_h,
                vec![Box::new(Container::column(4, rows)) as Box<dyn Widget>],
            ))
        }
        SettingsCategory::PinnedApps => {
            if pinned_adding {
                // ── Add-app sub-view ──
                let pinned_programs: std::collections::HashSet<&str> =
                    pinned_apps.iter().map(|p| p.program.as_str()).collect();
                let mut addable: Vec<&DesktopApp> = all_apps
                    .iter()
                    .filter(|a| !pinned_programs.contains(a.program.as_str()))
                    .collect();
                addable.sort_by(|a, b| a.name.cmp(&b.name));

                let back_btn = Box::new(Button::with_id(
                    "pinned-add-close",
                    "← Back",
                    pal.accent,
                    content_w as i32,
                    36,
                )) as Box<dyn Widget>;

                let rows: Vec<Box<dyn Widget>> = addable
                    .iter()
                    .take(PINNED_ADD_IDS.len())
                    .enumerate()
                    .map(|(i, app)| {
                        let row_icon = app
                            .icon_name
                            .as_deref()
                            .and_then(|n| icon_cache.lookup(n, 24))
                            .and_then(icon_image_to_pixmap);
                        Box::new(AddAppRow {
                            index: i,
                            name: app.name.as_str().into(),
                            accent: pal.text,
                            row_width: content_w as i32,
                            icon: row_icon,
                        }) as Box<dyn Widget>
                    })
                    .collect();

                let list_h = content_h.saturating_sub(40);
                let list = Box::new(Container::centered_viewport(
                    content_w,
                    list_h,
                    vec![Box::new(Container::column(2, rows)) as Box<dyn Widget>],
                )) as Box<dyn Widget>;

                Box::new(Container::column(0, vec![back_btn, list]))
            } else {
                // ── Normal pinned list ──
                let count = pinned_apps.len().min(PINNED_MAX);

                let add_btn = Box::new(Button::with_id(
                    "pinned-add-open",
                    "+ Add App",
                    pal.accent,
                    content_w as i32,
                    36,
                )) as Box<dyn Widget>;

                if count == 0 {
                    let placeholder = Box::new(SettingsPlaceholder {
                        width: content_w as i32,
                        text: "No pinned apps. Use + Add App or right-click in the launcher.",
                    }) as Box<dyn Widget>;
                    Box::new(Container::column(4, vec![add_btn, placeholder]))
                } else {
                    let rows: Vec<Box<dyn Widget>> = pinned_apps
                        .iter()
                        .take(PINNED_MAX)
                        .enumerate()
                        .map(|(i, app)| {
                            let label_w = content_w as i32 - PINNED_BTN_W * 3;
                            let is_first = i == 0;
                            let is_last = i + 1 == count;
                            let up_color = if is_first { pal.text_dim } else { pal.accent };
                            let dn_color = if is_last { pal.text_dim } else { pal.accent };
                            let app_icon = app
                                .icon_name
                                .as_deref()
                                .and_then(|n| icon_cache.lookup(n, 24))
                                .and_then(icon_image_to_pixmap);
                            let label = Box::new(PinnedAppLabel {
                                label: app.label.clone().into(),
                                program: app.program.clone().into(),
                                width: label_w,
                                icon: app_icon,
                            }) as Box<dyn Widget>;
                            let btn_up = Box::new(Button::with_id(
                                PINNED_UP_IDS[i],
                                "↑",
                                up_color,
                                PINNED_BTN_W,
                                PINNED_ROW_H,
                            )) as Box<dyn Widget>;
                            let btn_dn = Box::new(Button::with_id(
                                PINNED_DN_IDS[i],
                                "↓",
                                dn_color,
                                PINNED_BTN_W,
                                PINNED_ROW_H,
                            )) as Box<dyn Widget>;
                            let btn_rm = Box::new(Button::with_id(
                                PINNED_RM_IDS[i],
                                "×",
                                pal.error,
                                PINNED_BTN_W,
                                PINNED_ROW_H,
                            )) as Box<dyn Widget>;
                            Box::new(Container::row(0, vec![label, btn_up, btn_dn, btn_rm]))
                                as Box<dyn Widget>
                        })
                        .collect();

                    let list_h = content_h.saturating_sub(40);
                    let list = Box::new(Container::centered_viewport(
                        content_w,
                        list_h,
                        vec![Box::new(Container::column(2, rows)) as Box<dyn Widget>],
                    )) as Box<dyn Widget>;

                    Box::new(Container::column(4, vec![add_btn, list]))
                }
            }
        }
        SettingsCategory::SystemOverview
        | SettingsCategory::Network
        | SettingsCategory::Bluetooth
        | SettingsCategory::Power
        | SettingsCategory::Users
        | SettingsCategory::Updates => Box::new(Container::centered_viewport(
            content_w,
            content_h,
            vec![Box::new(Container::column(
                4,
                vec![Box::new(SettingsSkeletonCard {
                    title: selected.label(),
                    detail: selected.skeleton_detail(),
                    row_width: content_w as i32,
                    accent: pal.accent,
                }) as Box<dyn Widget>],
            )) as Box<dyn Widget>],
        )),
        SettingsCategory::Sound => {
            let row_w = content_w as i32;
            let mut rows: Vec<Box<dyn Widget>> = vec![Box::new(SoundSummaryCard {
                snapshot: audio_snapshot.clone(),
                row_width: row_w,
                accent: pal.accent,
            }) as Box<dyn Widget>];

            if audio_snapshot.service != AudioServiceState::Running {
                rows.push(Box::new(SettingsPlaceholder {
                    width: row_w,
                    text: "PipeWire/WirePlumber status is not available",
                }));
            } else if audio_snapshot.outputs.is_empty() && audio_snapshot.inputs.is_empty() {
                rows.push(Box::new(SettingsPlaceholder {
                    width: row_w,
                    text: "No audio devices reported",
                }));
            } else {
                for device in audio_snapshot.outputs.iter().take(SOUND_MAX) {
                    rows.push(Box::new(SoundDeviceRow {
                        label: "Output",
                        device: device.clone(),
                        row_width: row_w,
                        accent: pal.accent,
                    }));
                }
                for device in audio_snapshot.inputs.iter().take(SOUND_MAX) {
                    rows.push(Box::new(SoundDeviceRow {
                        label: "Input",
                        device: device.clone(),
                        row_width: row_w,
                        accent: pal.accent,
                    }));
                }
            }

            Box::new(Container::centered_viewport(
                content_w,
                content_h,
                vec![Box::new(Container::column(4, rows)) as Box<dyn Widget>],
            ))
        }
        SettingsCategory::Printers => {
            let row_w = content_w as i32;
            let mut rows: Vec<Box<dyn Widget>> = vec![Box::new(PrinterSummaryCard {
                snapshot: printer_snapshot.clone(),
                row_width: row_w,
                accent: pal.accent,
            }) as Box<dyn Widget>];

            if printer_snapshot.service != PrinterServiceState::Running {
                rows.push(Box::new(SettingsPlaceholder {
                    width: row_w,
                    text: printer_service_message(printer_snapshot.service),
                }));
            } else if printer_snapshot.printers.is_empty() {
                rows.push(Box::new(SettingsPlaceholder {
                    width: row_w,
                    text: "No printers configured yet",
                }));
            } else {
                for printer in printer_snapshot.printers.iter().take(PRINTER_MAX) {
                    rows.push(Box::new(PrinterRow {
                        printer: printer.clone(),
                        row_width: row_w,
                        accent: pal.accent,
                    }));
                }
            }

            Box::new(Container::centered_viewport(
                content_w,
                content_h,
                vec![Box::new(Container::column(4, rows)) as Box<dyn Widget>],
            ))
        }
        other => Box::new(Container::centered_viewport(
            content_w,
            content_h,
            vec![Box::new(SettingsPlaceholder {
                width: content_w as i32,
                text: other.placeholder(),
            }) as Box<dyn Widget>],
        )),
    };

    let body = Box::new(Container::row(0, vec![sidebar, vsep, content])) as Box<dyn Widget>;

    let footer_left = vec![Box::new(Button::with_id(
        "show-tile-view",
        "\u{2190} Home",
        pal.accent,
        FOOTER_SWITCH_WIDTH,
        FOOTER_SWITCH_HEIGHT,
    )) as Box<dyn Widget>];

    let footer_right = build_power_footer_buttons(
        icon_cache,
        &pal,
        FOOTER_POWER_BUTTON_SIZE,
        POWER_ICON_SIZE,
        armed_power,
    );

    let footer = Container::footer_row(
        width,
        FOOTER_HEIGHT as i32,
        FOOTER_PADDING_X,
        FOOTER_CLUSTER_GAP,
        footer_left,
        footer_right,
    );

    let make_divider = || {
        Box::new(Divider {
            width: width as i32,
            color: divider_color,
        }) as Box<dyn Widget>
    };

    Box::new(Container::column(
        0,
        vec![
            header,
            Box::new(chip_bar) as Box<dyn Widget>,
            make_divider(),
            body,
            make_divider(),
            Box::new(footer) as Box<dyn Widget>,
        ],
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_settings_launcher(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    selected: SettingsCategory,
    available_themes: &[String],
    current_theme: &str,
    available_wallpapers: &[WallpaperEntry],
    wallpaper_thumbnails: &[Option<(u32, u32, Vec<u8>)>],
    current_wallpaper: Option<&str>,
    wallpaper_mode: WallpaperMode,
    pinned_apps: &[PinnedApp],
    output_workspaces: &[OutputWorkspaceState],
    display_mode_dropdown_open: Option<usize>,
    printer_snapshot: &PrinterSnapshot,
    audio_snapshot: &AudioSnapshot,
    pinned_adding: bool,
    all_apps: &[DesktopApp],
    icon_cache: &IconCache,
    armed_power: Option<(&str, f32)>,
    theme_config: &ThemeConfig,
    state_fn: &dyn Fn(&[usize]) -> WidgetState,
) {
    let expected = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if canvas.len() != expected {
        return;
    }
    let Some(mut pixmap) = Pixmap::new(width, height) else {
        return;
    };
    let theme = theme_from_config(theme_config);
    pixmap.fill(tiny_skia::Color::from_rgba8(
        theme.palette.background.r,
        theme.palette.background.g,
        theme.palette.background.b,
        theme.palette.background.a,
    ));
    let root = build_settings_widget_tree(
        width,
        height,
        selected,
        available_themes,
        current_theme,
        available_wallpapers,
        wallpaper_thumbnails,
        current_wallpaper,
        wallpaper_mode,
        pinned_apps,
        output_workspaces,
        display_mode_dropdown_open,
        printer_snapshot,
        audio_snapshot,
        pinned_adding,
        all_apps,
        icon_cache,
        armed_power,
        &theme,
    );
    if let Ok(layout) =
        meridian_ui::compute_layout(&*root, meridian_ui::PixelSize { width, height })
    {
        let mut pm = pixmap.as_mut();
        let _ = meridian_ui::render(&*root, &layout, &mut pm, &theme, state_fn);
    }
    blit_rgba_to_argb(pixmap.data(), canvas);
}

fn printer_service_message(service: PrinterServiceState) -> &'static str {
    match service {
        PrinterServiceState::Running => "",
        PrinterServiceState::Stopped => "CUPS scheduler is not running",
        PrinterServiceState::Unavailable => "lpstat is not available",
    }
}

fn blit_rgba_to_argb(src: &[u8], dst: &mut [u8]) {
    if src.len() != dst.len() || !src.len().is_multiple_of(4) {
        return;
    }
    for (s, d) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        d[0] = s[2];
        d[1] = s[1];
        d[2] = s[0];
        d[3] = s[3];
    }
}
