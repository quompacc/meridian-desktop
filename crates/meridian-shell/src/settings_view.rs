// settings_view.rs — widget-based settings sub-page for the launcher.

use meridian_ui::{
    effect::{paint_fill, paint_text, rounded_rect_path},
    style::{Color, Palette},
    widget::{Button, Container, Widget},
    Rect, Theme, WidgetState, WidgetStyle, UiSize, ui_length,
};
use tiny_skia::{Pixmap, PixmapMut, PixmapPaint, PixmapRef, Transform};

use crate::icons::{IconCache, IconImage};
use crate::panel::PinnedApp;
use meridian_config::{WallpaperEntry, WallpaperMode};

// ─── SettingsCategory ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    Theme,
    Cursor,
    Wallpaper,
    PinnedApps,
}

impl SettingsCategory {
    pub const ALL: &'static [SettingsCategory] = &[
        SettingsCategory::Theme,
        SettingsCategory::Cursor,
        SettingsCategory::Wallpaper,
        SettingsCategory::PinnedApps,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            SettingsCategory::Theme => "Theme",
            SettingsCategory::Cursor => "Cursor",
            SettingsCategory::Wallpaper => "Wallpaper",
            SettingsCategory::PinnedApps => "Pinned Apps",
        }
    }

    pub fn chip_id(&self) -> &'static str {
        match self {
            SettingsCategory::Theme => "settings-cat-theme",
            SettingsCategory::Cursor => "settings-cat-cursor",
            SettingsCategory::Wallpaper => "settings-cat-wallpaper",
            SettingsCategory::PinnedApps => "settings-cat-pinned",
        }
    }

    pub fn placeholder(&self) -> &'static str {
        match self {
            SettingsCategory::Theme => "",
            SettingsCategory::Cursor => "Cursor theme + size — coming soon",
            SettingsCategory::Wallpaper => "Wallpaper path + mode — coming soon",
            SettingsCategory::PinnedApps => "Reorder / add / remove pinned apps — coming soon",
        }
    }
}

impl Default for SettingsCategory {
    fn default() -> Self {
        SettingsCategory::Theme
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

pub(crate) const THEME_WIDGET_IDS: &[&'static str] = &[
    "settings-theme-0",  "settings-theme-1",  "settings-theme-2",  "settings-theme-3",
    "settings-theme-4",  "settings-theme-5",  "settings-theme-6",  "settings-theme-7",
    "settings-theme-8",  "settings-theme-9",  "settings-theme-10", "settings-theme-11",
    "settings-theme-12", "settings-theme-13", "settings-theme-14", "settings-theme-15",
    "settings-theme-16", "settings-theme-17", "settings-theme-18", "settings-theme-19",
];

pub(crate) const WALLPAPER_WIDGET_IDS: &[&'static str] = &[
    "settings-wallpaper-0",  "settings-wallpaper-1",  "settings-wallpaper-2",  "settings-wallpaper-3",  "settings-wallpaper-4",
    "settings-wallpaper-5",  "settings-wallpaper-6",  "settings-wallpaper-7",  "settings-wallpaper-8",  "settings-wallpaper-9",
    "settings-wallpaper-10", "settings-wallpaper-11", "settings-wallpaper-12", "settings-wallpaper-13", "settings-wallpaper-14",
    "settings-wallpaper-15", "settings-wallpaper-16", "settings-wallpaper-17", "settings-wallpaper-18", "settings-wallpaper-19",
    "settings-wallpaper-20", "settings-wallpaper-21", "settings-wallpaper-22", "settings-wallpaper-23", "settings-wallpaper-24",
    "settings-wallpaper-25", "settings-wallpaper-26", "settings-wallpaper-27", "settings-wallpaper-28", "settings-wallpaper-29",
    "settings-wallpaper-30", "settings-wallpaper-31", "settings-wallpaper-32", "settings-wallpaper-33", "settings-wallpaper-34",
    "settings-wallpaper-35", "settings-wallpaper-36", "settings-wallpaper-37", "settings-wallpaper-38", "settings-wallpaper-39",
];

const PINNED_UP_IDS: [&str; 16] = [
    "pinned-move-up-0",  "pinned-move-up-1",  "pinned-move-up-2",  "pinned-move-up-3",
    "pinned-move-up-4",  "pinned-move-up-5",  "pinned-move-up-6",  "pinned-move-up-7",
    "pinned-move-up-8",  "pinned-move-up-9",  "pinned-move-up-10", "pinned-move-up-11",
    "pinned-move-up-12", "pinned-move-up-13", "pinned-move-up-14", "pinned-move-up-15",
];
const PINNED_DN_IDS: [&str; 16] = [
    "pinned-move-dn-0",  "pinned-move-dn-1",  "pinned-move-dn-2",  "pinned-move-dn-3",
    "pinned-move-dn-4",  "pinned-move-dn-5",  "pinned-move-dn-6",  "pinned-move-dn-7",
    "pinned-move-dn-8",  "pinned-move-dn-9",  "pinned-move-dn-10", "pinned-move-dn-11",
    "pinned-move-dn-12", "pinned-move-dn-13", "pinned-move-dn-14", "pinned-move-dn-15",
];
const PINNED_RM_IDS: [&str; 16] = [
    "pinned-remove-0",  "pinned-remove-1",  "pinned-remove-2",  "pinned-remove-3",
    "pinned-remove-4",  "pinned-remove-5",  "pinned-remove-6",  "pinned-remove-7",
    "pinned-remove-8",  "pinned-remove-9",  "pinned-remove-10", "pinned-remove-11",
    "pinned-remove-12", "pinned-remove-13", "pinned-remove-14", "pinned-remove-15",
];

struct SettingsHeader {
    width: i32,
}

impl Widget for SettingsHeader {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize { width: ui_length(self.width as f32), height: ui_length(HEADER_HEIGHT as f32) },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, 0) {
            paint_fill(canvas, &path, theme.palette.surface);
        }
        paint_text(canvas, "Settings", area.x + 20, area.y + area.height - 12, 13.0, theme.palette.text_dim);
        // Thin accent underline
        let strip = Rect { x: area.x + 20, y: area.y + area.height - 2, width: 52, height: 2 };
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
            size: UiSize { width: ui_length(self.row_width as f32), height: ui_length(SIDEBAR_ROW_H as f32) },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = match state {
            WidgetState::Idle if self.is_selected => theme.palette.surface_alt,
            WidgetState::Idle    => theme.palette.surface,
            WidgetState::Hovered => theme.palette.surface.lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.10),
            WidgetState::Pressed => theme.palette.surface.lerp(Color::rgb(0, 0, 0), 0.15),
        };
        if let Some(path) = rounded_rect_path(area, 0) {
            paint_fill(canvas, &path, bg);
        }
        if self.is_selected {
            let strip = Rect { x: area.x, y: area.y + 8, width: 3, height: area.height - 16 };
            if let Some(path) = rounded_rect_path(strip, 1) {
                paint_fill(canvas, &path, self.accent);
            }
        }
        let text_color = if self.is_selected { self.accent } else { theme.palette.text };
        paint_text(canvas, self.cat.label(), area.x + 14, area.y + area.height - 14, 12.5, text_color);
    }
}

struct VerticalDivider {
    height: i32,
    color: Color,
}

impl Widget for VerticalDivider {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize { width: ui_length(1.0), height: ui_length(self.height as f32) },
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
            size: UiSize { width: ui_length(self.row_width as f32), height: ui_length(THEME_ROW_H as f32) },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = match state {
            WidgetState::Idle => {
                if self.is_selected {
                    theme.palette.surface.lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.08)
                } else {
                    theme.palette.surface
                }
            }
            WidgetState::Hovered => theme.palette.surface.lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.14),
            WidgetState::Pressed => theme.palette.surface.lerp(Color::rgb(0, 0, 0), 0.18),
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        if self.is_selected {
            let strip = Rect { x: area.x + 4, y: area.y + 8, width: 3, height: area.height - 16 };
            if let Some(path) = rounded_rect_path(strip, 1) {
                paint_fill(canvas, &path, self.accent);
            }
        }
        let text_color = if self.is_selected { self.accent } else { theme.palette.text };
        paint_text(canvas, &self.name, area.x + 16, area.y + area.height - 14, 13.0, text_color);
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
            size: UiSize { width: ui_length(self.row_width as f32), height: ui_length(WALLPAPER_ROW_H as f32) },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = match state {
            WidgetState::Idle => {
                if self.is_selected { theme.palette.surface.lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.08) }
                else { theme.palette.surface }
            }
            WidgetState::Hovered => theme.palette.surface.lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.12),
            WidgetState::Pressed => theme.palette.surface.lerp(Color::rgb(0, 0, 0), 0.18),
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        if self.is_selected {
            let strip = Rect { x: area.x + 4, y: area.y + 6, width: 3, height: area.height - 12 };
            if let Some(path) = rounded_rect_path(strip, 1) {
                paint_fill(canvas, &path, self.accent);
            }
        }
        // Thumbnail on the left (96x54 inside 64px-tall row).
        let thumb_left = area.x + 8;
        if let Some((tw, th, ref data)) = self.thumbnail {
            let thumb_y = area.y + (area.height - th as i32) / 2;
            if let Some(pm_ref) = PixmapRef::from_bytes(data, tw, th) {
                canvas.draw_pixmap(thumb_left, thumb_y, pm_ref, &PixmapPaint::default(), Transform::identity(), None);
            }
        } else {
            // Placeholder rectangle when thumbnail not yet loaded.
            let ph = Rect { x: thumb_left, y: area.y + 5, width: WALLPAPER_THUMB_W as i32, height: WALLPAPER_THUMB_H as i32 };
            if let Some(path) = rounded_rect_path(ph, 2) {
                paint_fill(canvas, &path, theme.palette.surface_alt);
            }
        }
        let text_x = area.x + 8 + WALLPAPER_THUMB_W as i32 + 8;
        let text_color = if self.is_selected { self.accent } else { theme.palette.text };
        paint_text(canvas, &self.display_name, text_x, area.y + area.height - 16, 12.0, text_color);
    }
}

struct WallpaperBrowseRow {
    row_width: i32,
    accent: Color,
}

impl Widget for WallpaperBrowseRow {
    fn id(&self) -> Option<&'static str> { Some("wallpaper-browse") }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize { width: ui_length(self.row_width as f32), height: ui_length(WALLPAPER_ROW_H as f32) },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = match state {
            WidgetState::Idle    => theme.palette.surface,
            WidgetState::Hovered => theme.palette.surface.lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.12),
            WidgetState::Pressed => theme.palette.surface.lerp(Color::rgb(0, 0, 0), 0.18),
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        // Icon area — dim filled rectangle with "..." hint.
        let icon = Rect { x: area.x + 8, y: area.y + (area.height - WALLPAPER_THUMB_H as i32) / 2,
                          width: WALLPAPER_THUMB_W as i32, height: WALLPAPER_THUMB_H as i32 };
        if let Some(path) = rounded_rect_path(icon, 4) {
            paint_fill(canvas, &path, self.accent.lerp(Color::rgb(0, 0, 0), 0.55));
        }
        paint_text(canvas, "\u{2026}", icon.x + icon.width / 2 - 6, icon.y + icon.height - 8, 14.0, self.accent);
        let text_x = area.x + 8 + WALLPAPER_THUMB_W as i32 + 8;
        paint_text(canvas, "Browse for image\u{2026}", text_x, area.y + area.height - 16, 12.0, self.accent);
    }
}


struct PinnedAppLabel {
    label: Box<str>,
    program: Box<str>,
    width: i32,
}

impl Widget for PinnedAppLabel {
    fn id(&self) -> Option<&'static str> { None }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize { width: ui_length(self.width as f32), height: ui_length(PINNED_ROW_H as f32) },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, 0) {
            paint_fill(canvas, &path, theme.palette.surface);
        }
        paint_text(canvas, &self.label,   area.x + 10, area.y + 16, 13.0, theme.palette.text);
        paint_text(canvas, &self.program, area.x + 10, area.y + 34, 11.0, theme.palette.text_dim);
    }
}

struct SettingsPlaceholder {
    width: i32,
    text: &'static str,
}

impl Widget for SettingsPlaceholder {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize { width: ui_length(self.width as f32), height: ui_length(60.0) },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        paint_text(canvas, self.text, area.x + 20, area.y + 36, 13.0, theme.palette.text_dim);
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

fn icon_image_to_pixmap(img: &IconImage) -> Option<Pixmap> {
    let mut pixmap = Pixmap::new(img.width, img.height)?;
    let data = pixmap.data_mut();
    for (i, chunk) in img.bgra.chunks_exact(4).enumerate() {
        let (b, g, r, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
        let o = i * 4;
        data[o]     = ((r as u16 * a as u16) / 255) as u8;
        data[o + 1] = ((g as u16 * a as u16) / 255) as u8;
        data[o + 2] = ((b as u16 * a as u16) / 255) as u8;
        data[o + 3] = a;
    }
    Some(pixmap)
}

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
    icon_cache: &IconCache,
) -> Box<dyn Widget> {
    let pal = Palette::TOKYO_NIGHT_METRO;

    let header = Box::new(SettingsHeader { width: width as i32 }) as Box<dyn Widget>;

    // Root-category chip bar — "Appearance" is the only root for now.
    // Future roots (System, Network, …) get added here as more chips.
    let root_chips: Vec<Box<dyn Widget>> = vec![
        Box::new(Button::with_id(
            "settings-root-appearance",
            "Appearance",
            pal.accent,
            ROOT_CHIP_W,
            ROOT_CHIP_H,
        )) as Box<dyn Widget>,
    ];
    let chip_bar = Container::centered_viewport(
        width,
        CHIPS_BAR_HEIGHT,
        vec![Box::new(Container::row(8, root_chips)) as Box<dyn Widget>],
    );

    let divider_color = Color::rgba(pal.accent.r, pal.accent.g, pal.accent.b, 180);
    let content_h = height.saturating_sub(
        HEADER_HEIGHT + CHIPS_BAR_HEIGHT + FOOTER_HEIGHT + 2 * DIVIDER_HEIGHT,
    );
    let content_w = width.saturating_sub(SIDEBAR_W + 1);

    // Left sidebar — sub-categories of the selected root category
    let sidebar_rows: Vec<Box<dyn Widget>> = SettingsCategory::ALL
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

    let vsep = Box::new(VerticalDivider { height: content_h as i32, color: divider_color }) as Box<dyn Widget>;

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
                ("wallpaper-mode-fill",   "Fill",   WallpaperMode::Fill),
                ("wallpaper-mode-fit",    "Fit",    WallpaperMode::Fit),
                ("wallpaper-mode-center", "Center", WallpaperMode::Center),
                ("wallpaper-mode-tile",   "Tile",   WallpaperMode::Tile),
            ].iter().map(|(id, label, mode)| {
                let accent = if *mode == wallpaper_mode { pal.accent } else { pal.surface };
                Box::new(Button::with_id(id, label, accent, 80, 32)) as Box<dyn Widget>
            }).collect();
            let mode_bar = Container::centered_viewport(
                content_w, WALLPAPER_MODE_BAR_H,
                vec![Box::new(Container::row(8, mode_chips)) as Box<dyn Widget>],
            );
            let list_h = content_h.saturating_sub(WALLPAPER_MODE_BAR_H);
            let max_visible = ((list_h + 2) / (WALLPAPER_ROW_H as u32 + 2))
                .min(WALLPAPER_WIDGET_IDS.len() as u32) as usize;
            let row_w = content_w as i32;
            let mut rows: Vec<Box<dyn Widget>> = Vec::new();
            rows.push(Box::new(WallpaperBrowseRow { row_width: row_w, accent: pal.accent }));
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
                        is_selected: current_wallpaper.map_or(false, |c| c == entry.apply_path.as_str()),
                        accent: pal.accent,
                        row_width: row_w,
                    }));
                }
            }
            let wallpaper_list = Container::centered_viewport(
                content_w, list_h,
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
        SettingsCategory::PinnedApps => {
            let count = pinned_apps.len().min(PINNED_MAX);
            if count == 0 {
                Box::new(Container::centered_viewport(content_w, content_h,
                    vec![Box::new(SettingsPlaceholder { width: content_w as i32, text: "No pinned apps. Right-click an app in the launcher to pin it." }) as Box<dyn Widget>],
                ))
            } else {
                let rows: Vec<Box<dyn Widget>> = pinned_apps.iter().take(PINNED_MAX).enumerate().map(|(i, app)| {
                    let label_w = content_w as i32 - PINNED_BTN_W * 3;
                    let is_first = i == 0;
                    let is_last = i + 1 == count;
                    let up_color = if is_first { pal.text_dim } else { pal.accent };
                    let dn_color = if is_last  { pal.text_dim } else { pal.accent };
                    let label = Box::new(PinnedAppLabel {
                        label: app.label.as_str().into(),
                        program: app.program.as_str().into(),
                        width: label_w,
                    }) as Box<dyn Widget>;
                    let btn_up = Box::new(Button::with_id(PINNED_UP_IDS[i], "↑", up_color, PINNED_BTN_W, PINNED_ROW_H)) as Box<dyn Widget>;
                    let btn_dn = Box::new(Button::with_id(PINNED_DN_IDS[i], "↓", dn_color, PINNED_BTN_W, PINNED_ROW_H)) as Box<dyn Widget>;
                    let btn_rm = Box::new(Button::with_id(PINNED_RM_IDS[i], "×", pal.error,  PINNED_BTN_W, PINNED_ROW_H)) as Box<dyn Widget>;
                    Box::new(Container::row(0, vec![label, btn_up, btn_dn, btn_rm])) as Box<dyn Widget>
                }).collect();
                Box::new(Container::centered_viewport(content_w, content_h,
                    vec![Box::new(Container::column(2, rows)) as Box<dyn Widget>],
                ))
            }
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

    let body = Box::new(Container::row(
        0,
        vec![sidebar, vsep, content],
    )) as Box<dyn Widget>;

    let power_off_icon     = icon_cache.lookup("system-shutdown",    POWER_ICON_SIZE).and_then(icon_image_to_pixmap);
    let power_restart_icon = icon_cache.lookup("system-reboot",      POWER_ICON_SIZE).and_then(icon_image_to_pixmap);
    let power_sleep_icon   = icon_cache.lookup("system-suspend",     POWER_ICON_SIZE).and_then(icon_image_to_pixmap);
    let power_lock_icon    = icon_cache.lookup("system-lock-screen", POWER_ICON_SIZE).and_then(icon_image_to_pixmap);
    let power_logout_icon  = icon_cache.lookup("system-log-out",     POWER_ICON_SIZE).and_then(icon_image_to_pixmap);

    let footer_left = vec![
        Box::new(Button::with_id(
            "show-tile-view",
            "\u{2190} Home",
            pal.accent,
            FOOTER_SWITCH_WIDTH,
            FOOTER_SWITCH_HEIGHT,
        )) as Box<dyn Widget>,
    ];

    let footer_right = vec![
        Box::new(Button::with_id_and_icon("power-off",     "Off",  pal.error,      FOOTER_POWER_BUTTON_SIZE, FOOTER_POWER_BUTTON_SIZE, power_off_icon))     as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon("power-restart", "Rst",  pal.warning,    FOOTER_POWER_BUTTON_SIZE, FOOTER_POWER_BUTTON_SIZE, power_restart_icon)) as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon("power-sleep",   "Zzz",  pal.accent,     FOOTER_POWER_BUTTON_SIZE, FOOTER_POWER_BUTTON_SIZE, power_sleep_icon))   as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon("power-lock",    "Lock", pal.accent_alt, FOOTER_POWER_BUTTON_SIZE, FOOTER_POWER_BUTTON_SIZE, power_lock_icon))    as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon("power-logout",  "Out",  pal.success,    FOOTER_POWER_BUTTON_SIZE, FOOTER_POWER_BUTTON_SIZE, power_logout_icon))  as Box<dyn Widget>,
    ];

    let footer = Container::footer_row(
        width,
        FOOTER_HEIGHT as i32,
        FOOTER_PADDING_X,
        FOOTER_CLUSTER_GAP,
        footer_left,
        footer_right,
    );

    let make_divider = || {
        Box::new(Divider { width: width as i32, color: divider_color }) as Box<dyn Widget>
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
    icon_cache: &IconCache,
    state_fn: &dyn Fn(&[usize]) -> WidgetState,
) {
    let expected = (width as usize).saturating_mul(height as usize).saturating_mul(4);
    if canvas.len() != expected {
        return;
    }
    let Some(mut pixmap) = Pixmap::new(width, height) else { return };
    let theme = Theme::TOKYO_NIGHT_METRO;
    pixmap.fill(tiny_skia::Color::from_rgba8(
        theme.palette.background.r,
        theme.palette.background.g,
        theme.palette.background.b,
        theme.palette.background.a,
    ));
    let root = build_settings_widget_tree(width, height, selected, available_themes, current_theme, available_wallpapers, wallpaper_thumbnails, current_wallpaper, wallpaper_mode, pinned_apps, icon_cache);
    if let Ok(layout) = meridian_ui::compute_layout(&*root, meridian_ui::PixelSize { width, height }) {
        let mut pm = pixmap.as_mut();
        let _ = meridian_ui::render(&*root, &layout, &mut pm, &theme, state_fn);
    }
    blit_rgba_to_argb(pixmap.data(), canvas);
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
