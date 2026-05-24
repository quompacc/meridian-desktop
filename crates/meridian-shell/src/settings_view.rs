// settings_view.rs — widget-based settings sub-page for the launcher.

use meridian_ui::{
    effect::{paint_fill, paint_text, rounded_rect_path},
    style::{Color, Palette},
    widget::{Button, Container, Widget},
    Rect, Theme, WidgetState, WidgetStyle, UiSize, ui_length,
};
use tiny_skia::{Pixmap, PixmapMut};

use crate::icons::{IconCache, IconImage};
use meridian_config::WallpaperMode;

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
const CHIPS_BAR_HEIGHT: u32 = 52;
const CHIP_WIDTH: i32 = 120;
const CHIP_HEIGHT: i32 = 36;
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
const WALLPAPER_ROW_H: i32 = 40;

struct WallpaperRow {
    index: usize,
    display_name: Box<str>,
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
                if self.is_selected {
                    theme.palette.surface.lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.08)
                } else {
                    theme.palette.surface
                }
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
        let text_color = if self.is_selected { self.accent } else { theme.palette.text };
        paint_text(canvas, &self.display_name, area.x + 16, area.y + area.height - 12, 12.0, text_color);
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


fn wallpaper_display_name(path: &str) -> String {
    const SKIP: &[&str] = &[
        "usr", "share", "wallpapers", "backgrounds",
        "contents", "images", "pictures",
    ];
    let meaningful: Vec<&str> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .filter(|p| {
            let lo = p.to_ascii_lowercase();
            !SKIP.iter().any(|s| *s == lo.as_str())
        })
        .collect();
    let filename = *meaningful.last().unwrap_or(&path);
    let stem = filename.rsplitn(2, '.').last().unwrap_or(filename);
    if meaningful.len() >= 2 {
        let pack = meaningful[meaningful.len() - 2];
        format!("{} · {}", pack, stem)
    } else {
        stem.to_string()
    }
}

pub(crate) fn build_settings_widget_tree(
    width: u32,
    height: u32,
    selected: SettingsCategory,
    available_themes: &[String],
    current_theme: &str,
    available_wallpapers: &[String],
    current_wallpaper: Option<&str>,
    wallpaper_mode: WallpaperMode,
    icon_cache: &IconCache,
) -> Box<dyn Widget> {
    let pal = Palette::TOKYO_NIGHT_METRO;

    let header = Box::new(SettingsHeader { width: width as i32 }) as Box<dyn Widget>;

    let chips: Vec<Box<dyn Widget>> = SettingsCategory::ALL
        .iter()
        .map(|cat| {
            let accent = if *cat == selected { pal.accent } else { pal.surface };
            Box::new(Button::with_id(cat.chip_id(), cat.label(), accent, CHIP_WIDTH, CHIP_HEIGHT))
                as Box<dyn Widget>
        })
        .collect();

    let chip_bar = Container::centered_viewport(
        width,
        CHIPS_BAR_HEIGHT,
        vec![Box::new(Container::row(8, chips)) as Box<dyn Widget>],
    );

    let divider_color = Color::rgba(pal.accent.r, pal.accent.g, pal.accent.b, 180);
    let content_h = height.saturating_sub(
        HEADER_HEIGHT + CHIPS_BAR_HEIGHT + FOOTER_HEIGHT + 2 * DIVIDER_HEIGHT,
    );

    let content: Box<dyn Widget> = match selected {
        SettingsCategory::Theme => {
            let row_w = width as i32;
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
                width,
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
                Box::new(Button::with_id(id, label, accent, CHIP_WIDTH, CHIP_HEIGHT)) as Box<dyn Widget>
            }).collect();
            let mode_bar = Container::centered_viewport(
                width, WALLPAPER_MODE_BAR_H,
                vec![Box::new(Container::row(8, mode_chips)) as Box<dyn Widget>],
            );
            let list_h = content_h.saturating_sub(WALLPAPER_MODE_BAR_H);
            // Cap rows to what actually fits so the list never overflows footer/header.
            // Each row takes WALLPAPER_ROW_H + 2px gap (column gap) except the last.
            let max_visible = ((list_h + 2) / (WALLPAPER_ROW_H as u32 + 2))
                .min(WALLPAPER_WIDGET_IDS.len() as u32) as usize;
            let row_w = width as i32;
            let rows: Vec<Box<dyn Widget>> = if available_wallpapers.is_empty() {
                vec![Box::new(SettingsPlaceholder {
                    width: row_w,
                    text: "No wallpapers found in /usr/share/wallpapers or ~/Pictures",
                }) as Box<dyn Widget>]
            } else {
                available_wallpapers
                    .iter()
                    .take(max_visible)
                    .enumerate()
                    .map(|(i, path)| {
                        Box::new(WallpaperRow {
                            index: i,
                            display_name: wallpaper_display_name(path).into(),
                            is_selected: current_wallpaper.map_or(false, |c| c == path.as_str()),
                            accent: pal.accent,
                            row_width: row_w,
                        }) as Box<dyn Widget>
                    })
                    .collect()
            };
            let wallpaper_list = Container::centered_viewport(
                width, list_h,
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
        other => Box::new(Container::centered_viewport(
            width,
            content_h,
            vec![Box::new(SettingsPlaceholder {
                width: width as i32,
                text: other.placeholder(),
            }) as Box<dyn Widget>],
        )),
    };

    // Footer — same power buttons as app_view, "← Home" on the left.
    let power_off_icon = icon_cache.lookup("system-shutdown", POWER_ICON_SIZE).and_then(icon_image_to_pixmap);
    let power_restart_icon = icon_cache.lookup("system-reboot", POWER_ICON_SIZE).and_then(icon_image_to_pixmap);
    let power_sleep_icon = icon_cache.lookup("system-suspend", POWER_ICON_SIZE).and_then(icon_image_to_pixmap);
    let power_lock_icon = icon_cache.lookup("system-lock-screen", POWER_ICON_SIZE).and_then(icon_image_to_pixmap);
    let power_logout_icon = icon_cache.lookup("system-log-out", POWER_ICON_SIZE).and_then(icon_image_to_pixmap);

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
        Box::new(Button::with_id_and_icon("power-off",     "Off",  pal.error,    FOOTER_POWER_BUTTON_SIZE, FOOTER_POWER_BUTTON_SIZE, power_off_icon))     as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon("power-restart", "Rst",  pal.warning,  FOOTER_POWER_BUTTON_SIZE, FOOTER_POWER_BUTTON_SIZE, power_restart_icon)) as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon("power-sleep",   "Zzz",  pal.accent,   FOOTER_POWER_BUTTON_SIZE, FOOTER_POWER_BUTTON_SIZE, power_sleep_icon))   as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon("power-lock",    "Lock", pal.accent_alt, FOOTER_POWER_BUTTON_SIZE, FOOTER_POWER_BUTTON_SIZE, power_lock_icon))  as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon("power-logout",  "Out",  pal.success,  FOOTER_POWER_BUTTON_SIZE, FOOTER_POWER_BUTTON_SIZE, power_logout_icon))  as Box<dyn Widget>,
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
            content,
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
    available_wallpapers: &[String],
    current_wallpaper: Option<&str>,
    wallpaper_mode: WallpaperMode,
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
    let root = build_settings_widget_tree(width, height, selected, available_themes, current_theme, available_wallpapers, current_wallpaper, wallpaper_mode, icon_cache);
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
