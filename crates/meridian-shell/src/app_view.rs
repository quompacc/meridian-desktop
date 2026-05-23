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
            Self::Internet => &["network", "webbrowser", "email", "instantmessaging", "chat"],
            Self::Kreativ => &[
                "graphics",
                "photography",
                "audio",
                "video",
                "music",
                "audiovideo",
            ],
            Self::Buero => &[
                "office",
                "wordprocessor",
                "spreadsheet",
                "presentation",
                "viewer",
            ],
            Self::Entwicklung => &["development", "ide", "debugger", "revisioncontrol"],
            Self::System => &[
                "system",
                "settings",
                "security",
                "filemanager",
                "filesystem",
            ],
            Self::Spiele => &["game", "actiongame", "arcadegame", "boardgame", "logicgame"],
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
const DIVIDER_HEIGHT: i32 = 2;

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

struct Divider {
    width: i32,
    color: Color,
}

impl Widget for Divider {
    fn style(&self) -> meridian_ui::WidgetStyle {
        meridian_ui::WidgetStyle {
            size: meridian_ui::UiSize {
                width: meridian_ui::ui_length(self.width as f32),
                height: meridian_ui::ui_length(DIVIDER_HEIGHT as f32),
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

const SEARCH_BAR_HEIGHT: u32 = 44;

struct SearchBar {
    width: i32,
    query: Box<str>,
}

impl Widget for SearchBar {
    fn style(&self) -> meridian_ui::WidgetStyle {
        meridian_ui::WidgetStyle {
            size: meridian_ui::UiSize {
                width: meridian_ui::ui_length(self.width as f32),
                height: meridian_ui::ui_length(SEARCH_BAR_HEIGHT as f32),
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

        if let Some(path) = rounded_rect_path(area, 4) {
            paint_fill(canvas, &path, body_color);
        }

        let text_x = area.x + 12;
        let text_baseline = area.y + area.height - 10;
        let font_size: f32 = 13.0;

        if self.query.is_empty() {
            let dimmed = Color::rgba(
                theme.palette.text.r,
                theme.palette.text.g,
                theme.palette.text.b,
                80,
            );
            paint_text(
                canvas,
                "Apps suchen...",
                text_x,
                text_baseline,
                font_size,
                dimmed,
            );
        } else {
            paint_text(
                canvas,
                &self.query,
                text_x,
                text_baseline,
                font_size,
                theme.palette.text,
            );
        }
    }
}
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
    search_query: &str,
) -> Box<dyn Widget> {
    let pal = Palette::TOKYO_NIGHT_METRO;

    let search_bar: Box<dyn Widget> = Box::new(SearchBar {
        width: width as i32,
        query: search_query.into(),
    });

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
        .filter(|app| {
            !app.terminal
                && app
                    .icon_name
                    .as_deref()
                    .and_then(|name| icon_cache.lookup(name, 24))
                    .is_some()
                && category.matches(app)
                && (search_query.is_empty()
                    || app
                        .name
                        .to_lowercase()
                        .contains(&search_query.to_lowercase()))
        })
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

    const DIVIDER_COUNT: u32 = 2;
    let grid_height = height.saturating_sub(
        SEARCH_BAR_HEIGHT
            + CHIPS_BAR_HEIGHT
            + FOOTER_HEIGHT
            + DIVIDER_COUNT * DIVIDER_HEIGHT as u32,
    );

    let grid = Container::centered_viewport(
        width,
        grid_height,
        vec![Box::new(Container::column(8, row_widgets)) as Box<dyn Widget>],
    );

    let settings_icon = icon_cache
        .lookup("preferences-system-symbolic", POWER_ICON_SIZE)
        .and_then(icon_image_to_pixmap);

    let footer_left = vec![
        Box::new(Button::with_id(
            "show-tile-view",
            "← Apps",
            pal.accent,
            FOOTER_SWITCH_WIDTH,
            FOOTER_SWITCH_HEIGHT,
        )) as Box<dyn Widget>,
        Box::new(Button::with_id_and_icon(
            "launcher-settings",
            "Settings",
            pal.accent_alt,
            FOOTER_SWITCH_WIDTH,
            FOOTER_SWITCH_HEIGHT,
            settings_icon,
        )) as Box<dyn Widget>,
    ];

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

    let divider_color = Color::rgba(active_accent.r, active_accent.g, active_accent.b, 180);
    let make_divider = || -> Box<dyn Widget> {
        Box::new(Divider {
            width: width as i32,
            color: divider_color,
        })
    };

    Box::new(Container::column(
        0,
        vec![
            search_bar,
            Box::new(chip_bar) as Box<dyn Widget>,
            make_divider(),
            Box::new(grid) as Box<dyn Widget>,
            make_divider(),
            Box::new(footer) as Box<dyn Widget>,
        ],
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_app_view(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    apps: &[DesktopApp],
    category: AppCategory,
    icon_cache: &IconCache,
    state_fn: &dyn Fn(&[usize]) -> WidgetState,
    search_query: &str,
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

    let root = build_app_view_widget_tree(width, height, apps, category, icon_cache, search_query);

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
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{build_app_view_widget_tree, AppCard, AppCategory, SearchBar};
    use crate::icons::{IconCache, IconLoader};
    use crate::launcher::DesktopApp;
    use meridian_ui::style::Color;
    use meridian_ui::Widget;

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(label: &str) -> Self {
            let mut path = std::env::temp_dir();
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            path.push(format!(
                "meridian-shell-app-view-{label}-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn path(&self) -> &PathBuf {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_theme_index(path: &std::path::Path, directories: &[&str]) {
        let mut body = format!(
            "[Icon Theme]\nName=Theme\nInherits=\nDirectories={}\n\n",
            directories.join(",")
        );
        for directory in directories {
            let size: u32 = directory
                .split('x')
                .next()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0);
            body.push_str(&format!("[{directory}]\nType=Fixed\nSize={size}\n\n"));
        }
        fs::write(path, body).expect("write index.theme");
    }

    fn write_png(path: &std::path::Path) {
        use png::{BitDepth, ColorType, Encoder};
        let file = fs::File::create(path).expect("create png");
        let mut encoder = Encoder::new(file, 24, 24);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        let mut writer = encoder.write_header().expect("write header");
        let data = vec![255u8; 24 * 24 * 4];
        writer.write_image_data(&data).expect("write data");
    }

    fn test_icon_cache() -> (TempDir, IconCache) {
        let temp = TempDir::new("app-view");
        let icons_root = temp.path().join("icons");
        let theme_root = icons_root.join("Adwaita");
        let apps = theme_root.join("22x22/apps");
        fs::create_dir_all(&apps).expect("create apps dir");
        write_theme_index(&theme_root.join("index.theme"), &["22x22/apps"]);
        write_png(&apps.join("firefox.png"));
        write_png(&apps.join("utilities-terminal.png"));

        let loader = IconLoader::new_for_tests("Adwaita", vec![icons_root], vec![]);
        let mut cache = IconCache::new_for_tests(loader);
        cache.warm(&["firefox"], 24);
        cache.warm(&["utilities-terminal"], 24);
        (temp, cache)
    }

    fn firefox_app() -> DesktopApp {
        let mut app = DesktopApp::new("Firefox".into(), vec!["firefox".into()], false);
        app.icon_name = Some("firefox".into());
        app.categories = vec!["network".into(), "webbrowser".into()];
        app
    }

    #[test]
    fn search_bar_style_returns_correct_size() {
        let bar = SearchBar {
            width: 880,
            query: "".into(),
        };
        let style = bar.style();
        assert_eq!(style.size.width, meridian_ui::ui_length(880.0));
        assert_eq!(
            style.size.height,
            meridian_ui::ui_length(super::SEARCH_BAR_HEIGHT as f32)
        );
    }

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
        let tree = build_app_view_widget_tree(880, 620, &[], AppCategory::Alle, &icon_cache, "");
        let children = tree.children();
        assert_eq!(
            children.len(),
            6,
            "root column should have 6 children (search, chips, divider, grid, divider, footer)"
        );
    }

    #[test]
    fn app_view_search_filters_by_query_match() {
        let (_temp, icon_cache) = test_icon_cache();
        let app = firefox_app();
        let tree = build_app_view_widget_tree(
            880,
            620,
            &[app],
            AppCategory::Internet,
            &icon_cache,
            "fire",
        );
        let children = tree.children();
        assert_eq!(children.len(), 6);
        // child[3] is the grid container
        let grid = &children[3];
        let grid_children = grid.children();
        // grid has 1 child: the column container
        assert_eq!(grid_children.len(), 1);
        let column = &grid_children[0];
        // column has rows; with 1 matching app there should be at least 1 row
        assert!(
            !column.children().is_empty(),
            "should have at least one row with an AppCard"
        );
    }

    #[test]
    fn app_view_search_excludes_non_matching_query() {
        let (_temp, icon_cache) = test_icon_cache();
        let app = firefox_app();
        let tree = build_app_view_widget_tree(
            880,
            620,
            &[app],
            AppCategory::Internet,
            &icon_cache,
            "zzznomatch",
        );
        let children = tree.children();
        assert_eq!(children.len(), 6);
        // child[3] is the grid container
        let grid = &children[3];
        let grid_children = grid.children();
        assert_eq!(grid_children.len(), 1);
        let column = &grid_children[0];
        assert!(
            column.children().is_empty(),
            "should have no rows when no app matches search query"
        );
    }
}
