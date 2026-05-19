use meridian_ui::{
    effect::{paint_fill, paint_text, rounded_rect_path},
    paint::Rect,
    style::{Color, Palette},
    widget::{Button, Container, Widget},
    Theme, WidgetState,
};
use tiny_skia::{Pixmap, PixmapMut, PixmapPaint, Transform};

use crate::icons::{IconCache, IconImage};
use crate::launcher::DesktopApp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum AppCategory {
    Internet,
    Kreativ,
    Buero,
    Entwicklung,
    System,
    Spiele,
    #[default]
    Alle,
}

impl AppCategory {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Internet => "Internet",
            Self::Kreativ => "Kreativ",
            Self::Buero => "Büro",
            Self::Entwicklung => "Entwicklung",
            Self::System => "System",
            Self::Spiele => "Spiele",
            Self::Alle => "Alle",
        }
    }

    pub(crate) fn chip_id(&self) -> &'static str {
        match self {
            Self::Internet => "cat-internet",
            Self::Kreativ => "cat-kreativ",
            Self::Buero => "cat-buero",
            Self::Entwicklung => "cat-entwicklung",
            Self::System => "cat-system",
            Self::Spiele => "cat-spiele",
            Self::Alle => "cat-alle",
        }
    }

    pub(crate) fn accent(&self, pal: &Palette) -> Color {
        match self {
            Self::Internet => pal.accent,
            Self::Kreativ => pal.accent_alt,
            Self::Buero => pal.warning,
            Self::Entwicklung => pal.success,
            Self::System => pal.error,
            Self::Spiele => pal.accent,
            Self::Alle => pal.accent,
        }
    }

    fn tokens(&self) -> &'static [&'static str] {
        match self {
            Self::Internet => &["Network", "WebBrowser", "Email", "InstantMessaging", "Chat"],
            Self::Kreativ => &[
                "Graphics",
                "Photography",
                "Audio",
                "Video",
                "Music",
                "AudioVideo",
            ],
            Self::Buero => &[
                "Office",
                "WordProcessor",
                "Spreadsheet",
                "Presentation",
                "Viewer",
            ],
            Self::Entwicklung => &["Development", "IDE", "Debugger", "RevisionControl"],
            Self::System => &[
                "System",
                "Settings",
                "Security",
                "FileManager",
                "Filesystem",
            ],
            Self::Spiele => &["Game", "ActionGame", "ArcadeGame", "BoardGame", "LogicGame"],
            Self::Alle => &[],
        }
    }

    pub(crate) fn matches(&self, app: &DesktopApp) -> bool {
        if matches!(self, Self::Alle) {
            return true;
        }
        let tokens = self.tokens();
        app.categories.iter().any(|c| tokens.contains(&c.as_str()))
    }
}

const ALL_CATEGORIES: [AppCategory; 7] = [
    AppCategory::Alle,
    AppCategory::Internet,
    AppCategory::Kreativ,
    AppCategory::Buero,
    AppCategory::Entwicklung,
    AppCategory::System,
    AppCategory::Spiele,
];

fn icon_image_to_pixmap(img: &IconImage) -> Option<Pixmap> {
    let w = img.width;
    let h = img.height;
    let mut pixmap = Pixmap::new(w, h)?;
    let data = pixmap.data_mut();
    for (i, chunk) in img.bgra.chunks_exact(4).enumerate() {
        let b = chunk[0];
        let g = chunk[1];
        let r = chunk[2];
        let a = chunk[3];
        let out_idx = i * 4;
        data[out_idx] = ((r as u16 * a as u16) / 255) as u8;
        data[out_idx + 1] = ((g as u16 * a as u16) / 255) as u8;
        data[out_idx + 2] = ((b as u16 * a as u16) / 255) as u8;
        data[out_idx + 3] = a;
    }
    Some(pixmap)
}

const APP_CARD_WIDTH: i32 = 268;
const APP_CARD_HEIGHT: i32 = 52;
const APP_CARD_ICON_SIZE: u32 = 24;
const APP_CARD_CORNER_RADIUS: i32 = 4;

pub(crate) struct AppCard {
    pub(crate) label: Box<str>,
    pub(crate) exec: Box<str>,
    pub(crate) icon: Option<Pixmap>,
    #[allow(dead_code)]
    pub(crate) accent: Color,
}

impl Widget for AppCard {
    fn style(&self) -> meridian_ui::WidgetStyle {
        meridian_ui::WidgetStyle {
            size: meridian_ui::UiSize {
                width: meridian_ui::ui_length(APP_CARD_WIDTH as f32),
                height: meridian_ui::ui_length(APP_CARD_HEIGHT as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let body_color = match state {
            WidgetState::Idle => theme.palette.surface,
            WidgetState::Hovered => theme
                .palette
                .surface
                .lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.15),
            WidgetState::Pressed => theme.palette.surface.lerp(Color::rgb(0, 0, 0), 0.18),
        };

        if let Some(path) = rounded_rect_path(area, APP_CARD_CORNER_RADIUS) {
            paint_fill(canvas, &path, body_color);
        }

        if let Some(ref icon) = self.icon {
            let ih = icon.height() as i32;
            let x = area.x + 10;
            let y = area.y + (area.height - ih) / 2;
            canvas.draw_pixmap(
                x,
                y,
                icon.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }

        let text_x = area.x + 10 + 24 + 8;
        let text_baseline = area.y + area.height - 10;
        paint_text(
            canvas,
            &self.label,
            text_x,
            text_baseline,
            13.0,
            theme.palette.text,
        );
    }

    fn launch_exec(&self) -> Option<&str> {
        Some(&self.exec)
    }
}

const SEARCH_BAR_HEIGHT: u32 = 44;
const CHIPS_BAR_HEIGHT: u32 = 52;
const FOOTER_HEIGHT: u32 = 56;
const CHIP_WIDTH: i32 = 104;
const CHIP_HEIGHT: i32 = 36;
const FOOTER_SWITCH_WIDTH: i32 = 144;
const FOOTER_SWITCH_HEIGHT: i32 = 48;
const FOOTER_POWER_BUTTON_SIZE: i32 = 48;
const FOOTER_PADDING_X: i32 = 28;
const FOOTER_CLUSTER_GAP: i32 = 8;
const POWER_ICON_SIZE: u32 = 32;

pub(crate) fn build_app_view_widget_tree(
    width: u32,
    height: u32,
    apps: &[DesktopApp],
    category: AppCategory,
    icon_cache: &IconCache,
) -> Box<dyn Widget> {
    let pal = Palette::TOKYO_NIGHT_METRO;

    let search_bar: Box<dyn Widget> = Box::new(Button::new(
        "Apps suchen...",
        pal.surface,
        width as i32,
        SEARCH_BAR_HEIGHT as i32,
    ));

    let active_accent = category.accent(&pal);
    let chips: Vec<Box<dyn Widget>> = ALL_CATEGORIES
        .iter()
        .map(|cat| {
            let accent = if *cat == category {
                active_accent
            } else {
                pal.surface
            };
            Box::new(Button::with_id(
                cat.chip_id(),
                cat.label(),
                accent,
                CHIP_WIDTH,
                CHIP_HEIGHT,
            )) as Box<dyn Widget>
        })
        .collect();

    let chip_bar = Container::centered_viewport(
        width,
        CHIPS_BAR_HEIGHT,
        vec![Box::new(Container::row(8, chips)) as Box<dyn Widget>],
    );

    let filtered: Vec<&DesktopApp> = apps
        .iter()
        .filter(|app| !app.terminal && app.icon_name.is_some() && category.matches(app))
        .take(21)
        .collect();

    let mut cards: Vec<Box<dyn Widget>> = filtered
        .iter()
        .map(|app| {
            let icon_name = app.icon_name.as_deref().unwrap_or("");
            let maybe_pixmap = icon_cache
                .lookup(icon_name, APP_CARD_ICON_SIZE)
                .and_then(icon_image_to_pixmap);
            Box::new(AppCard {
                label: app.name.clone().into_boxed_str(),
                exec: app.program.clone().into_boxed_str(),
                icon: maybe_pixmap,
                accent: active_accent,
            }) as Box<dyn Widget>
        })
        .collect();

    let mut row_widgets: Vec<Box<dyn Widget>> = Vec::new();
    while !cards.is_empty() {
        let end = 3.min(cards.len());
        let row_cards: Vec<Box<dyn Widget>> = cards.drain(0..end).collect();
        row_widgets.push(Box::new(Container::row(8, row_cards)) as Box<dyn Widget>);
    }

    let grid_height = height.saturating_sub(SEARCH_BAR_HEIGHT + CHIPS_BAR_HEIGHT + FOOTER_HEIGHT);

    let grid = Container::centered_viewport(
        width,
        grid_height,
        vec![Box::new(Container::column(8, row_widgets)) as Box<dyn Widget>],
    );

    let footer_left = vec![Box::new(Button::with_id(
        "show-tile-view",
        "← Apps",
        pal.accent,
        FOOTER_SWITCH_WIDTH,
        FOOTER_SWITCH_HEIGHT,
    )) as Box<dyn Widget>];

    let power_off_icon = icon_cache
        .lookup("system-shutdown", POWER_ICON_SIZE)
        .and_then(icon_image_to_pixmap);
    let power_restart_icon = icon_cache
        .lookup("system-reboot", POWER_ICON_SIZE)
        .and_then(icon_image_to_pixmap);
    let power_sleep_icon = icon_cache
        .lookup("system-suspend", POWER_ICON_SIZE)
        .and_then(icon_image_to_pixmap);
    let power_lock_icon = icon_cache
        .lookup("system-lock-screen", POWER_ICON_SIZE)
        .and_then(icon_image_to_pixmap);
    let power_logout_icon = icon_cache
        .lookup("system-log-out", POWER_ICON_SIZE)
        .and_then(icon_image_to_pixmap);

    let footer_right = vec![
        Box::new(Button::with_id_and_icon(
            "power-off",
            "Off",
            pal.error,
            FOOTER_POWER_BUTTON_SIZE,
            FOOTER_POWER_BUTTON_SIZE,
            power_off_icon,
        )) as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon(
            "power-restart",
            "Rst",
            pal.warning,
            FOOTER_POWER_BUTTON_SIZE,
            FOOTER_POWER_BUTTON_SIZE,
            power_restart_icon,
        )) as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon(
            "power-sleep",
            "Zzz",
            pal.accent,
            FOOTER_POWER_BUTTON_SIZE,
            FOOTER_POWER_BUTTON_SIZE,
            power_sleep_icon,
        )) as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon(
            "power-lock",
            "Lock",
            pal.accent_alt,
            FOOTER_POWER_BUTTON_SIZE,
            FOOTER_POWER_BUTTON_SIZE,
            power_lock_icon,
        )) as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon(
            "power-logout",
            "Out",
            pal.success,
            FOOTER_POWER_BUTTON_SIZE,
            FOOTER_POWER_BUTTON_SIZE,
            power_logout_icon,
        )) as Box<dyn Widget>,
    ];

    let footer = Container::footer_row(
        width,
        FOOTER_HEIGHT as i32,
        FOOTER_PADDING_X,
        FOOTER_CLUSTER_GAP,
        footer_left,
        footer_right,
    );

    Box::new(Container::column(
        0,
        vec![
            search_bar,
            Box::new(chip_bar) as Box<dyn Widget>,
            Box::new(grid) as Box<dyn Widget>,
            Box::new(footer) as Box<dyn Widget>,
        ],
    ))
}

pub(crate) fn draw_app_view(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    apps: &[DesktopApp],
    category: AppCategory,
    icon_cache: &IconCache,
    state_fn: &dyn Fn(&[usize]) -> WidgetState,
) {
    let expected_len = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if canvas.len() != expected_len {
        return;
    }

    let Some(mut pixmap) = Pixmap::new(width, height) else {
        return;
    };

    let theme = Theme::TOKYO_NIGHT_METRO;
    pixmap.fill(to_tiny_skia_color(theme.palette.background));

    let root = build_app_view_widget_tree(width, height, apps, category, icon_cache);

    if let Ok(layout) =
        meridian_ui::compute_layout(&*root, meridian_ui::PixelSize { width, height })
    {
        let mut pixmap_canvas = pixmap.as_mut();
        let _ = meridian_ui::render(&*root, &layout, &mut pixmap_canvas, &theme, state_fn);
    }

    blit_rgba_to_argb(pixmap.data(), canvas);
}

fn blit_rgba_to_argb(src: &[u8], dst: &mut [u8]) {
    if src.len() != dst.len() || !src.len().is_multiple_of(4) {
        return;
    }

    for (rgba, argb) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        argb[0] = rgba[2];
        argb[1] = rgba[1];
        argb[2] = rgba[0];
        argb[3] = rgba[3];
    }
}

fn to_tiny_skia_color(color: meridian_ui::style::Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(color.r, color.g, color.b, color.a)
}

#[cfg(test)]
mod tests {
    use super::{build_app_view_widget_tree, AppCard, AppCategory};
    use crate::icons::IconCache;
    use meridian_ui::style::Color;
    use meridian_ui::Widget;

    #[test]
    fn app_category_chip_id_mapping() {
        assert_eq!(AppCategory::Internet.chip_id(), "cat-internet");
        assert_eq!(AppCategory::Kreativ.chip_id(), "cat-kreativ");
        assert_eq!(AppCategory::Buero.chip_id(), "cat-buero");
        assert_eq!(AppCategory::Entwicklung.chip_id(), "cat-entwicklung");
        assert_eq!(AppCategory::System.chip_id(), "cat-system");
        assert_eq!(AppCategory::Spiele.chip_id(), "cat-spiele");
        assert_eq!(AppCategory::Alle.chip_id(), "cat-alle");
    }

    #[test]
    fn app_card_launch_exec() {
        let card = AppCard {
            label: "Firefox".into(),
            exec: "firefox".into(),
            icon: None,
            accent: Color::rgb(0, 0, 0),
        };
        assert_eq!(card.launch_exec(), Some("firefox"));
    }

    #[test]
    fn build_app_view_widget_tree_empty_apps() {
        let icon_cache = IconCache::new();
        let tree = build_app_view_widget_tree(880, 620, &[], AppCategory::Alle, &icon_cache);
        let children = tree.children();
        assert_eq!(children.len(), 4, "root column should have 4 children");
    }

    #[test]
    fn app_category_labels_match_expected() {
        assert_eq!(AppCategory::Internet.label(), "Internet");
        assert_eq!(AppCategory::Kreativ.label(), "Kreativ");
        assert_eq!(AppCategory::Buero.label(), "Büro");
        assert_eq!(AppCategory::Entwicklung.label(), "Entwicklung");
        assert_eq!(AppCategory::System.label(), "System");
        assert_eq!(AppCategory::Spiele.label(), "Spiele");
        assert_eq!(AppCategory::Alle.label(), "Alle");
    }
}
