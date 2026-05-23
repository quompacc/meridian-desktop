// settings_view.rs
//
// Two co-existing parts:
//   1. draw_settings (Painter-based) — used by the dormant overlay.
//   2. build_settings_widget_tree / draw_settings_launcher — widget-based
//      launcher sub-page (the active path).

use std::cell::RefCell;

use meridian_config::ThemeConfig;
// PainterRect = crate::Rect {x,y,w,h} used by draw_settings.
use crate::{Painter, Rect as PainterRect, TextRenderer};

use meridian_ui::{
    effect::{paint_fill, paint_text, rounded_rect_path},
    style::{Color, Palette},
    widget::{Button, Container, Widget},
    Rect, Theme, WidgetState, WidgetStyle, UiSize, ui_length,
};
use tiny_skia::{Pixmap, PixmapMut};

use crate::icons::{IconCache, IconImage};

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

// ─── Dormant overlay drawing (used by draw_settings_popup) ───────────────────

const SIDEBAR_WIDTH: i32 = 180;
const SIDEBAR_ITEM_HEIGHT: i32 = 44;
const SIDEBAR_PAD_X: i32 = 16;
const SIDEBAR_TOP_PAD: i32 = 20;
const CONTENT_PAD_X: i32 = 32;
const CONTENT_TOP_PAD: i32 = 28;
const TITLE_HEIGHT: i32 = 28;
const ACCENT_STRIP_H: i32 = 2;
const OVERLAY_THEME_ROW_H: i32 = 36;

pub fn draw_settings(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    width: u32,
    height: u32,
    selected: SettingsCategory,
    available_themes: &[String],
    current_theme: &str,
) {
    let colors = &theme.colors;
    let width = width as i32;
    let height = height as i32;

    painter.clear(colors.surface_alt);
    painter.stroke_rect(PainterRect { x: 0, y: 0, w: width, h: height }, colors.border);
    painter.rect(PainterRect { x: 0, y: 0, w: SIDEBAR_WIDTH, h: height }, colors.surface);
    painter.rect(PainterRect { x: SIDEBAR_WIDTH, y: 0, w: 1, h: height }, colors.border);

    for (i, cat) in SettingsCategory::ALL.iter().enumerate() {
        let y = SIDEBAR_TOP_PAD + (i as i32) * SIDEBAR_ITEM_HEIGHT;
        if *cat == selected {
            painter.rect(PainterRect { x: 0, y, w: 3, h: SIDEBAR_ITEM_HEIGHT }, colors.accent);
        }
        let label_color = if *cat == selected { colors.accent } else { colors.text };
        painter.text_clipped(font, cat.label(), SIDEBAR_PAD_X, y + SIDEBAR_ITEM_HEIGHT / 2 + 6,
            SIDEBAR_WIDTH - 2 * SIDEBAR_PAD_X, label_color);
    }

    let content_x = SIDEBAR_WIDTH + CONTENT_PAD_X;
    let content_w = width - SIDEBAR_WIDTH - 2 * CONTENT_PAD_X;
    painter.text_clipped(font, selected.label(), content_x, CONTENT_TOP_PAD + 18, content_w, colors.accent);
    painter.rect(PainterRect { x: content_x, y: CONTENT_TOP_PAD + TITLE_HEIGHT, w: content_w, h: ACCENT_STRIP_H }, colors.accent);

    let body_y = CONTENT_TOP_PAD + TITLE_HEIGHT + ACCENT_STRIP_H + 30;
    match selected {
        SettingsCategory::Theme => {
            for (i, name) in available_themes.iter().enumerate() {
                let row_y = body_y + (i as i32) * OVERLAY_THEME_ROW_H;
                let is_selected = name.as_str() == current_theme;
                if is_selected {
                    painter.rect(PainterRect { x: content_x, y: row_y, w: 3, h: OVERLAY_THEME_ROW_H }, colors.accent);
                }
                let text_color = if is_selected { colors.accent } else { colors.text };
                painter.text_clipped(font, name, content_x + 10, row_y + OVERLAY_THEME_ROW_H / 2 + 6,
                    content_w - 10, text_color);
            }
        }
        SettingsCategory::Cursor => {
            painter.text_clipped(font, "Cursor theme + size — A3.4", content_x, body_y, content_w, colors.text_dim);
        }
        SettingsCategory::Wallpaper => {
            painter.text_clipped(font, "Wallpaper path + mode — A3.5", content_x, body_y, content_w, colors.text_dim);
        }
        SettingsCategory::PinnedApps => {
            painter.text_clipped(font, "Reorder / add / remove pinned apps — A3.6", content_x, body_y, content_w, colors.text_dim);
        }
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
    let root = build_settings_widget_tree(width, height, selected, available_themes, current_theme, icon_cache);
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
