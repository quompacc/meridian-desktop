use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use meridian_config::{Color, ThemeConfig};
use meridian_ipc::ShellCommand;
use tracing::{debug, info, warn};

use crate::{
    icons::IconCache,
    ui::{
        primitives::{
            draw_active_indicator, draw_initial_badge, draw_list_item, draw_panel_button,
            draw_sidebar_item, fill_surface_with_radius, subtle_border, ActiveIndicatorEdge,
            InteractiveState, SurfaceKind,
        },
        tokens,
    },
    ClickAction, ClickZone, Painter, Rect, TextRenderer,
};

const PINNED_GRID_COLS: usize = 2;
const MAX_RESULTS: usize = 9;
const MAX_PINNED_RESULTS: usize = 4;
const SIDEBAR_CATEGORY_CLICK_BASE: u8 = 100;
const PINNED_CANDIDATES: &[&str] = &[
    "terminal",
    "foot",
    "alacritty",
    "kitty",
    "wezterm",
    "ghostty",
    "konsole",
    "kgx",
    "firefox",
    "chromium",
    "google chrome",
    "brave",
    "dolphin",
    "nautilus",
    "thunar",
    "nemo",
    "system settings",
    "systemsettings",
];
const XDG_DATA_DIRS_DEFAULT: &str = "/usr/local/share:/usr/share";
const MERIDIAN_DESKTOP_ENV: &str = "Meridian";
const SELECTED_EXEC_HINT_COLOR: Color = Color::rgb(0x3a, 0x3a, 0x44);
const FOOTER_TOP_GAP: i32 = 6;
const FOOTER_BOTTOM_MARGIN: i32 = 10;
const FOOTER_BAR_V_PADDING: i32 = 3;
const FOOTER_SECTION_GAP: i32 = 12;
const FOOTER_LEFT_MIN_W: i32 = 120;
const FOOTER_MODE_PILL_W: i32 = 82;
const FOOTER_MODE_PILL_H: i32 = 24;
const FOOTER_ACTION_BUTTON_H: i32 = 28;
const FOOTER_ACTION_BUTTON_MIN_W: i32 = 150;
const FOOTER_ACTION_BUTTON_MAX_W: i32 = 220;
const TILE_START_SWITCH_BUTTON_W: i32 = 140;
const ALL_APPS_BACK_BUTTON_W: i32 = 120;
const TILE_GRID_COLS: u8 = 6;
const TILE_SLOT_MIN_PX: i32 = 64;
const TILE_GAP: i32 = 8;
const TILE_LABEL_H: i32 = 22;
const TILE_ICON_SIZE_SMALL: u32 = 96;
const TILE_ICON_SIZE_MEDIUM: u32 = 192;
const TILE_ICON_SIZE_WIDE: u32 = 192;
const SECTION_HEADER_H: i32 = 32;
const SECTION_HEADER_TO_TILES_GAP: i32 = 6;
const SECTION_GAP: i32 = 12;
const SECTION_HEADER_TEXT_X_INSET: i32 = 6;
const SECTION_HEADER_BASELINE_OFFSET: i32 = 22;
const SECTION_HEADER_UNDERLINE_H: i32 = 3;
const SIDEBAR_ITEM_X_INSET: i32 = 8;
const SIDEBAR_ITEM_W_INSET: i32 = 16;
const SIDEBAR_ITEM_H: i32 = 24;
const SIDEBAR_ITEM_GAP: i32 = 4;
const SIDEBAR_TEXT_X_OFFSET: i32 = 10;
const SIDEBAR_TEXT_BASELINE_OFFSET: i32 = 16;
const SIDEBAR_TOP_PADDING: i32 = 10;
const SIDEBAR_SECTION_GAP: i32 = 12;
const SIDEBAR_DIVIDER_X_INSET: i32 = 12;
const SIDEBAR_DIVIDER_MARGIN_TOP: i32 = 10;
const SIDEBAR_DIVIDER_TO_LIST_GAP: i32 = 10;
const HEADER_TITLE_BASELINE_OFFSET: i32 = 16;
const HEADER_COUNT_BASELINE_OFFSET: i32 = tokens::launcher::HEADER_H + 1;
const HEADER_COUNT_WIDTH: i32 = 110;
const HEADER_COUNT_RIGHT_INSET: i32 = 0;
const HEADER_TITLE_TO_COUNT_GAP: i32 = 10;
const SEARCH_TEXT_BASELINE_OFFSET: i32 = 28;
const PINNED_APP_TITLE_BASELINE_OFFSET: i32 = 20;
const APP_ROW_TITLE_BASELINE_OFFSET: i32 = 16;
const APP_ROW_SUBTITLE_BASELINE_OFFSET: i32 = 30;
const LAUNCHER_ICON_SIZE: u32 = 24;
const LAUNCHER_ICON_SLOT: i32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidebarCategory {
    Favorites = 0,
    AllApps = 1,
    Development = 2,
    Internet = 3,
    System = 4,
    Utilities = 5,
    Graphics = 6,
    Games = 7,
}

impl SidebarCategory {
    fn label(self) -> &'static str {
        match self {
            Self::Favorites => "Favorites",
            Self::AllApps => "All apps",
            Self::Development => "Development",
            Self::Internet => "Internet",
            Self::System => "System",
            Self::Utilities => "Utilities",
            Self::Graphics => "Graphics",
            Self::Games => "Games",
        }
    }

    fn to_click_id(self) -> u8 {
        SIDEBAR_CATEGORY_CLICK_BASE + self as u8
    }

    fn from_click_id(raw: u8) -> Option<Self> {
        let offset = raw.checked_sub(SIDEBAR_CATEGORY_CLICK_BASE)?;
        match offset {
            0 => Some(Self::Favorites),
            1 => Some(Self::AllApps),
            2 => Some(Self::Development),
            3 => Some(Self::Internet),
            4 => Some(Self::System),
            5 => Some(Self::Utilities),
            6 => Some(Self::Graphics),
            7 => Some(Self::Games),
            _ => None,
        }
    }

    fn category_tokens(self) -> &'static [&'static str] {
        match self {
            Self::Favorites | Self::AllApps => &[],
            Self::Development => &[
                "development",
                "ide",
                "building",
                "debugger",
                "profiling",
                "revisioncontrol",
                "translation",
            ],
            Self::Internet => &[
                "network",
                "webbrowser",
                "email",
                "chat",
                "instantmessaging",
                "ircclient",
                "filetransfer",
                "remoteaccess",
            ],
            Self::System => &[
                "system",
                "settings",
                "desktopsettings",
                "hardwaresettings",
                "packagemanager",
                "security",
            ],
            Self::Utilities => &[
                "utility",
                "texteditor",
                "archiving",
                "calculator",
                "clock",
                "filetools",
            ],
            Self::Graphics => &[
                "graphics",
                "2dgraphics",
                "rastergraphics",
                "vectorgraphics",
                "3dgraphics",
                "photography",
                "scanning",
                "viewer",
            ],
            Self::Games => &[
                "game",
                "actiongame",
                "arcadegame",
                "boardgame",
                "cardgame",
                "kidsgame",
                "logicgame",
                "simulation",
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LauncherAction {
    ExitMeridian,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LauncherMode {
    Apps,
}

impl LauncherMode {
    fn label(self) -> &'static str {
        match self {
            Self::Apps => "Apps",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LauncherView {
    TileStart,
    AllApps,
}

#[allow(clippy::derivable_impls)]
impl Default for LauncherView {
    fn default() -> Self {
        Self::TileStart
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileSize {
    Small,
    Medium,
    Wide,
}

impl TileSize {
    pub fn grid_units(self) -> (u8, u8) {
        match self {
            TileSize::Small => (1, 1),
            TileSize::Medium => (2, 2),
            TileSize::Wide => (4, 2),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppTile {
    pub app_index: usize,
    pub size: TileSize,
    pub col: u8,
    /// Row index relative to the section this tile belongs to (0 = first row).
    pub row: u8,
}

#[derive(Debug, Clone)]
pub struct AppSection {
    pub letter: char,
    pub tiles: Vec<AppTile>,
    /// Number of grid rows occupied by this section's tiles.
    pub rows: u8,
}

impl LauncherAction {
    fn label(self) -> &'static str {
        match self {
            Self::ExitMeridian => "Exit Meridian",
        }
    }

    fn confirm_label(self) -> &'static str {
        match self {
            Self::ExitMeridian => "Confirm Exit",
        }
    }
}

fn app_matches_sidebar_category(app: &DesktopApp, category: SidebarCategory) -> bool {
    let tokens = category.category_tokens();
    !tokens.is_empty()
        && app
            .categories
            .iter()
            .any(|category| tokens.iter().any(|token| category == token))
}

#[derive(Debug, Clone)]
pub struct DesktopApp {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    pub terminal: bool,
    pub categories: Vec<String>,
    pub icon_name: Option<String>,
    name_key: String,
    exec_key: String,
}

impl DesktopApp {
    pub fn load_system() -> Vec<Self> {
        Self::load_from_dirs(desktop_app_dirs())
    }

    fn new(name: String, exec_argv: Vec<String>, terminal: bool) -> Self {
        let name = name.trim().to_string();
        let program = exec_argv.first().cloned().unwrap_or_default();
        let args = exec_argv.iter().skip(1).cloned().collect::<Vec<_>>();
        let exec = argv_to_display(&program, &args);
        Self {
            name_key: name.to_lowercase(),
            exec_key: exec.to_lowercase(),
            name,
            program,
            args,
            terminal,
            categories: Vec::new(),
            icon_name: None,
        }
    }

    fn load_from_dirs(dirs: Vec<PathBuf>) -> Vec<Self> {
        let mut apps = Vec::new();
        let mut seen = HashSet::new();

        for dir in dirs {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            let mut paths = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| is_desktop_file(path))
                .collect::<Vec<_>>();
            paths.sort();

            for path in paths {
                let Some(app) = Self::from_file(&path) else {
                    continue;
                };
                if seen.insert((app.name_key.clone(), app.exec_key.clone())) {
                    apps.push(app);
                }
            }
        }

        apps.sort_by(cmp_apps);
        apps
    }

    fn from_file(path: &Path) -> Option<Self> {
        let raw = fs::read_to_string(path).ok()?;
        match Self::from_desktop_entry_str_with_reason(&raw) {
            Ok(app) => Some(app),
            Err(reason) => {
                debug!(path=?path, reason, "launcher ignored desktop entry");
                None
            }
        }
    }

    fn from_desktop_entry_str_with_reason(raw: &str) -> Result<Self, &'static str> {
        let mut in_desktop_entry = false;
        let mut name = None;
        let mut exec_argv = None;
        let mut try_exec = None;
        let mut only_show_in = None;
        let mut not_show_in = None;
        let mut terminal = false;
        let mut hidden = false;
        let mut no_display = false;
        let mut desktop_type = None;
        let mut categories = None;
        let mut icon_name = None;

        for line in raw.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                in_desktop_entry = line == "[Desktop Entry]";
                continue;
            }
            if !in_desktop_entry || line.starts_with('#') || line.is_empty() {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();

            match key {
                "Name" if !value.is_empty() => {
                    name.get_or_insert_with(|| value.to_string());
                }
                "Exec" => {
                    let argv = parse_exec_argv(value);
                    if !argv.is_empty() {
                        exec_argv.get_or_insert(argv);
                    }
                }
                "Terminal" => terminal = value.eq_ignore_ascii_case("true"),
                "Hidden" => hidden = value.eq_ignore_ascii_case("true"),
                "NoDisplay" => no_display = value.eq_ignore_ascii_case("true"),
                "TryExec" if !value.is_empty() => {
                    try_exec.get_or_insert_with(|| value.to_string());
                }
                "OnlyShowIn" if !value.is_empty() => {
                    only_show_in.get_or_insert_with(|| value.to_string());
                }
                "NotShowIn" if !value.is_empty() => {
                    not_show_in.get_or_insert_with(|| value.to_string());
                }
                "Type" => desktop_type = Some(value.to_string()),
                "Categories" if !value.is_empty() => {
                    categories.get_or_insert_with(|| value.to_string());
                }
                "Icon" => {
                    icon_name = normalize_icon_name(value);
                }
                _ => {}
            };
        }

        if desktop_type.as_deref() != Some("Application") {
            return Err("unsupported-type");
        }

        if hidden || no_display {
            return Err("hidden-or-nodisplay");
        }

        if let Some(only_show_in) = only_show_in {
            if !desktop_env_list_contains(&only_show_in, MERIDIAN_DESKTOP_ENV) {
                return Err("onlyshowin-excludes-meridian");
            }
        }

        if let Some(not_show_in) = not_show_in {
            if desktop_env_list_contains(&not_show_in, MERIDIAN_DESKTOP_ENV) {
                return Err("notshowin-includes-meridian");
            }
        }

        if let Some(try_exec) = try_exec {
            if !is_executable_available(try_exec.trim()) {
                return Err("tryexec-unavailable");
            }
        }

        let name = name.ok_or("missing-name")?;
        let exec_argv = exec_argv.ok_or("missing-exec")?;
        let mut app = Self::new(name, exec_argv, terminal);
        if let Some(raw_categories) = categories {
            app.categories = parse_categories(&raw_categories);
        }
        app.icon_name = icon_name;
        if app.name.is_empty() || app.program.is_empty() {
            return Err("empty-name-or-exec");
        }

        Ok(app)
    }

    fn matches_query(&self, query: &str) -> bool {
        query.is_empty() || self.name_key.contains(query) || self.exec_key.contains(query)
    }
}

fn parse_categories(raw: &str) -> Vec<String> {
    raw.split(';')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn normalize_icon_name(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    if value.starts_with('/') {
        return Some(value.to_string());
    }

    let lowered = value.to_ascii_lowercase();
    if lowered.ends_with(".png") || lowered.ends_with(".svg") || lowered.ends_with(".xpm") {
        return value.rsplit_once('.').map(|(base, _)| base.to_string());
    }

    Some(value.to_string())
}

fn parse_exec_argv(exec: &str) -> Vec<String> {
    tokenize_exec(exec)
        .into_iter()
        .filter_map(|token| {
            let cleaned = strip_field_codes(&token).trim().to_string();
            (!cleaned.is_empty()).then_some(cleaned)
        })
        .collect()
}

fn tokenize_exec(exec: &str) -> Vec<String> {
    #[derive(Copy, Clone, Eq, PartialEq)]
    enum Quote {
        Single,
        Double,
    }

    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut chars = exec.chars().peekable();

    while let Some(ch) = chars.next() {
        match quote {
            Some(Quote::Single) => {
                if ch == '\'' {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            Some(Quote::Double) => {
                if ch == '"' {
                    quote = None;
                } else if ch == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else {
                    current.push(ch);
                }
            }
            None => {
                if ch.is_whitespace() {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                } else if ch == '\'' {
                    quote = Some(Quote::Single);
                } else if ch == '"' {
                    quote = Some(Quote::Double);
                } else if ch == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else {
                    current.push(ch);
                }
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn strip_field_codes(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            output.push(ch);
            continue;
        }

        match chars.peek().copied() {
            Some('%') => {
                output.push('%');
                let _ = chars.next();
            }
            Some(next) if next.is_ascii_alphabetic() => {
                let _ = chars.next();
            }
            Some(next) => {
                output.push('%');
                output.push(next);
                let _ = chars.next();
            }
            None => output.push('%'),
        }
    }
    output
}

fn argv_to_display(program: &str, args: &[String]) -> String {
    if program.is_empty() {
        return String::new();
    }

    let mut display = String::from(program);
    for arg in args {
        display.push(' ');
        display.push_str(arg);
    }
    display
}

fn desktop_env_list_contains(value: &str, needle: &str) -> bool {
    value
        .split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .any(|entry| entry.eq_ignore_ascii_case(needle))
}

fn cmp_apps(left: &DesktopApp, right: &DesktopApp) -> Ordering {
    left.name_key
        .cmp(&right.name_key)
        .then_with(|| left.exec_key.cmp(&right.exec_key))
        .then_with(|| left.terminal.cmp(&right.terminal))
}

fn is_desktop_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("desktop"))
}

fn desktop_app_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen = HashSet::new();

    let local = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    if let Some(local) = local {
        push_unique_dir(&mut dirs, &mut seen, local.join("applications"));
    }

    let data_dirs = env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| XDG_DATA_DIRS_DEFAULT.to_string());
    for base in data_dirs
        .split(':')
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        push_unique_dir(
            &mut dirs,
            &mut seen,
            PathBuf::from(base).join("applications"),
        );
    }

    dirs
}

fn push_unique_dir(dirs: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, dir: PathBuf) {
    if seen.insert(dir.clone()) {
        dirs.push(dir);
    }
}

fn is_executable_available(binary_or_path: &str) -> bool {
    if binary_or_path.is_empty() {
        return false;
    }
    let candidate = Path::new(binary_or_path);
    if candidate.is_absolute() {
        return is_executable_file(candidate);
    }

    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path)
        .map(|entry| entry.join(binary_or_path))
        .any(|candidate| is_executable_file(&candidate))
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn terminal_program() -> Option<String> {
    env::var("TERMINAL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            [
                "foot",
                "alacritty",
                "kitty",
                "wezterm",
                "ghostty",
                "kgx",
                "konsole",
                "xterm",
            ]
            .into_iter()
            .find(|candidate| is_executable_available(candidate))
            .map(str::to_string)
        })
}

fn is_firefox_program(program: &str) -> bool {
    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("firefox") || name.eq_ignore_ascii_case("firefox-esr")
        })
}

pub struct LauncherState {
    pub open: bool,
    pub query: String,
    pub selected_index: usize,
    pub clicks: Vec<ClickZone>,
    pub apps: Vec<DesktopApp>,
    pub app_sections: Vec<AppSection>,
    pub hover_app_index: Option<usize>,
    pub tile_scroll_y: i32,
    pub tile_content_h_cache: i32,
    pub tile_viewport_h_cache: i32,
    pub sidebar_category: SidebarCategory,
    pending_action_confirmation: Option<LauncherAction>,
    view: LauncherView,
}

struct VisibleApps {
    apps: Vec<DesktopApp>,
    total_results: usize,
    pinned_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectedLauncherEntry {
    App(usize),
    Action(LauncherAction),
}

fn search_match_rank(app: &DesktopApp, query: &str) -> u8 {
    let exec_base = app_exec_basename(&app.program);
    if app.name_key == query {
        0
    } else if app.name_key.starts_with(query) {
        1
    } else if app.name_key.contains(query) {
        2
    } else if exec_base.starts_with(query) {
        3
    } else if exec_base.contains(query) {
        4
    } else if app.exec_key.contains(query) || app.program.to_ascii_lowercase().contains(query) {
        5
    } else {
        6
    }
}

fn random_tile_size_for_app(app: &DesktopApp) -> TileSize {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    app.name_key.hash(&mut hasher);
    let bucket = hasher.finish() % 100;
    // ~15% Wide, ~35% Medium, ~50% Small — genug Variation in den
    // alphabetisch ersten Viewport-Reihen, ohne dass Wides die Spalten
    // dominieren (Wide belegt 4 von 6 Cols).
    if bucket < 15 {
        TileSize::Wide
    } else if bucket < 50 {
        TileSize::Medium
    } else {
        TileSize::Small
    }
}

fn section_letter_for(name_key: &str) -> char {
    name_key
        .chars()
        .next()
        .map(|c| {
            if c.is_ascii_alphabetic() {
                c.to_ascii_uppercase()
            } else {
                '#'
            }
        })
        .unwrap_or('#')
}

fn pack_one_section(letter: char, group: &[(usize, &DesktopApp)]) -> AppSection {
    let cols = TILE_GRID_COLS as usize;
    let mut rows = 1usize;
    let mut occupied = vec![false; cols * rows];
    let mut tiles = Vec::new();

    for (app_index, app) in group {
        let size = random_tile_size_for_app(app);
        let (tile_w, tile_h) = size.grid_units();
        let tile_w = tile_w as usize;
        let tile_h = tile_h as usize;
        if tile_w > cols {
            continue;
        }

        let mut found = None;
        while found.is_none() {
            for row in 0..rows {
                for col in 0..cols {
                    if col + tile_w > cols || row + tile_h > rows {
                        continue;
                    }
                    let mut fits = true;
                    for yy in row..(row + tile_h) {
                        for xx in col..(col + tile_w) {
                            if occupied[yy * cols + xx] {
                                fits = false;
                                break;
                            }
                        }
                        if !fits {
                            break;
                        }
                    }
                    if fits {
                        found = Some((col, row));
                        break;
                    }
                }
                if found.is_some() {
                    break;
                }
            }

            if found.is_none() {
                rows += 1;
                occupied.extend(std::iter::repeat_n(false, cols));
            }
        }

        let Some((col, row)) = found else { continue };
        for yy in row..(row + tile_h) {
            for xx in col..(col + tile_w) {
                occupied[yy * cols + xx] = true;
            }
        }
        tiles.push(AppTile {
            app_index: *app_index,
            size,
            col: col as u8,
            row: row as u8,
        });
    }

    // Compute actual rows used (last occupied row + 1) so empty trailing
    // rows added by the dynamic grow loop do not inflate section height.
    let used_rows = tiles
        .iter()
        .map(|t| t.row as usize + t.size.grid_units().1 as usize)
        .max()
        .unwrap_or(0);

    AppSection {
        letter,
        tiles,
        rows: used_rows as u8,
    }
}

fn pack_app_sections(apps: &[DesktopApp]) -> Vec<AppSection> {
    let mut sections: Vec<AppSection> = Vec::new();
    let mut current_letter: Option<char> = None;
    let mut group: Vec<(usize, &DesktopApp)> = Vec::new();

    for (app_index, app) in apps.iter().enumerate() {
        let letter = section_letter_for(&app.name_key);
        if Some(letter) != current_letter {
            if let Some(c) = current_letter {
                sections.push(pack_one_section(c, &group));
            }
            current_letter = Some(letter);
            group.clear();
        }
        group.push((app_index, app));
    }
    if let Some(c) = current_letter {
        sections.push(pack_one_section(c, &group));
    }

    sections
}

#[derive(Debug, Clone, Copy)]
struct TileGridGeometry {
    origin_x: i32,
    slot_w: i32,
    slot_h: i32,
    gap: i32,
    cols: u8,
    rows: u8,
}

fn compute_tile_grid_geometry(area: Rect) -> TileGridGeometry {
    let cols = TILE_GRID_COLS;
    let gap = TILE_GAP;
    let usable_w = (area.w - gap * (cols as i32 - 1)).max(0);
    let slot = (usable_w / cols as i32).max(TILE_SLOT_MIN_PX);
    let grid_w = slot * cols as i32 + gap * (cols as i32 - 1);
    let origin_x = area.x + (area.w - grid_w).max(0) / 2;
    let rows = if slot + gap > 0 {
        (((area.h + gap) / (slot + gap)) as u8).max(1)
    } else {
        1
    };
    TileGridGeometry {
        origin_x,
        slot_w: slot,
        slot_h: slot,
        gap,
        cols,
        rows,
    }
}

impl LauncherState {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::new_with_apps(DesktopApp::load_system())
    }

    pub fn new_with_apps(apps: Vec<DesktopApp>) -> Self {
        let app_sections = pack_app_sections(&apps);
        Self {
            open: false,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::Favorites,
            clicks: Vec::new(),
            apps,
            app_sections,
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
        }
    }

    pub fn toggle(&mut self) -> bool {
        self.open = !self.open;
        if self.open {
            self.apps = DesktopApp::load_system();
            self.app_sections = pack_app_sections(&self.apps);
        }
        self.query.clear();
        self.selected_index = 0;
        self.sidebar_category = SidebarCategory::Favorites;
        self.pending_action_confirmation = None;
        self.hover_app_index = None;
        self.tile_scroll_y = 0;
        self.tile_content_h_cache = 0;
        self.tile_viewport_h_cache = 0;
        self.view = LauncherView::TileStart;
        self.open
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.selected_index = 0;
        self.sidebar_category = SidebarCategory::Favorites;
        self.pending_action_confirmation = None;
        self.hover_app_index = None;
        self.tile_scroll_y = 0;
        self.tile_content_h_cache = 0;
        self.tile_viewport_h_cache = 0;
        self.view = LauncherView::TileStart;
    }

    pub fn filtered_apps(&self) -> Vec<DesktopApp> {
        self.visible_apps().apps
    }

    pub fn pending_action_confirmation(&self) -> Option<LauncherAction> {
        self.pending_action_confirmation
    }

    pub fn view(&self) -> LauncherView {
        self.view
    }

    pub fn set_view(&mut self, view: LauncherView) -> bool {
        if self.view == view {
            return false;
        }
        self.view = view;
        self.selected_index = 0;
        self.pending_action_confirmation = None;
        self.hover_app_index = None;
        true
    }

    pub fn scroll_tile_area(&mut self, delta_y: i32, viewport_h: i32, content_h: i32) -> bool {
        let max = (content_h - viewport_h).max(0);
        let next = (self.tile_scroll_y + delta_y).clamp(0, max);
        if next != self.tile_scroll_y {
            self.tile_scroll_y = next;
            return true;
        }
        false
    }

    pub fn current_mode(&self) -> LauncherMode {
        LauncherMode::Apps
    }

    fn visible_actions(&self) -> Vec<LauncherAction> {
        match self.current_mode() {
            LauncherMode::Apps => vec![LauncherAction::ExitMeridian],
        }
    }

    fn visible_apps(&self) -> VisibleApps {
        let query = self.query.to_lowercase();
        let app_limit = MAX_RESULTS.saturating_sub(self.visible_actions().len());
        let matching = self
            .apps
            .iter()
            .filter(|app| app.matches_query(&query))
            .cloned()
            .collect::<Vec<_>>();

        if !query.is_empty() {
            let mut ranked = matching;
            ranked.sort_by(|left, right| {
                search_match_rank(left, &query)
                    .cmp(&search_match_rank(right, &query))
                    .then_with(|| left.name_key.cmp(&right.name_key))
                    .then_with(|| left.exec_key.cmp(&right.exec_key))
            });
            return VisibleApps {
                total_results: ranked.len(),
                apps: ranked.into_iter().take(app_limit).collect(),
                pinned_count: 0,
            };
        }

        if self.sidebar_category == SidebarCategory::AllApps {
            return VisibleApps {
                total_results: matching.len(),
                apps: matching.into_iter().take(app_limit).collect(),
                pinned_count: 0,
            };
        }

        if self.sidebar_category != SidebarCategory::Favorites {
            let filtered = matching
                .into_iter()
                .filter(|app| app_matches_sidebar_category(app, self.sidebar_category))
                .collect::<Vec<_>>();
            return VisibleApps {
                total_results: filtered.len(),
                apps: filtered.into_iter().take(app_limit).collect(),
                pinned_count: 0,
            };
        }

        let mut pinned = Vec::new();
        let mut pinned_keys = HashSet::new();
        for app in &matching {
            if !is_pinned_candidate(app) || pinned.len() >= MAX_PINNED_RESULTS {
                continue;
            }
            let key = (app.name_key.clone(), app.exec_key.clone());
            if pinned_keys.insert(key) {
                pinned.push(app.clone());
            }
        }

        if pinned.is_empty() {
            return VisibleApps {
                total_results: matching.len(),
                apps: matching.into_iter().take(app_limit).collect(),
                pinned_count: 0,
            };
        }

        VisibleApps {
            total_results: pinned.len(),
            pinned_count: pinned.len(),
            apps: pinned,
        }
    }

    fn filtered_visible_count(&self) -> usize {
        self.visible_apps().apps.len() + self.visible_actions().len()
    }

    fn selected_index_clamped(&self, visible_len: usize) -> usize {
        if visible_len == 0 {
            0
        } else {
            self.selected_index.min(visible_len - 1)
        }
    }

    fn selected_entry(&self) -> Option<SelectedLauncherEntry> {
        let visible = self.visible_apps();
        let actions = self.visible_actions();
        let visible_len = visible.apps.len() + actions.len();
        if visible_len == 0 {
            return None;
        }

        let selected = self.selected_index_clamped(visible_len);
        if selected < visible.apps.len() {
            return Some(SelectedLauncherEntry::App(selected));
        }

        let action_idx = selected - visible.apps.len();
        actions
            .get(action_idx)
            .copied()
            .map(SelectedLauncherEntry::Action)
    }

    pub fn set_selected_index(&mut self, index: usize) -> bool {
        let visible_len = self.filtered_visible_count();
        let next = if visible_len == 0 {
            0
        } else {
            index.min(visible_len - 1)
        };
        if self.selected_index != next {
            self.selected_index = next;
            return true;
        }
        false
    }

    pub fn update_hover_selection(&mut self, x: f64, y: f64) -> bool {
        let selected_changed = if self.view == LauncherView::AllApps {
            let hovered_index = self
                .clicks
                .iter()
                .find(|zone| zone.rect.contains(x, y))
                .and_then(|zone| match zone.action {
                    ClickAction::LaunchApp(index) => Some(index),
                    ClickAction::LauncherAction { index, .. } => Some(index),
                    ClickAction::SwitchWorkspace(_) => None,
                    ClickAction::FocusWindow(_) => None,
                    ClickAction::LaunchPinnedApp(_) => None,
                    ClickAction::SelectLauncherCategory(_) => None,
                    ClickAction::SetLauncherView(_) => None,
                    ClickAction::ToggleLauncher => None,
                    ClickAction::Clock => None,
                    ClickAction::ToggleWorkspacePopup => None,
                    ClickAction::ToggleNetworkPopup => None,
                });

            match hovered_index {
                Some(index) => self.set_selected_index(index),
                None => false,
            }
        } else {
            false
        };

        let hover_changed = self.update_app_hover(x, y);
        selected_changed || hover_changed
    }

    pub fn update_app_hover(&mut self, x: f64, y: f64) -> bool {
        let target = if self.view == LauncherView::TileStart {
            self.clicks
                .iter()
                .find(|zone| zone.rect.contains(x, y))
                .and_then(|zone| match zone.action {
                    ClickAction::LaunchApp(idx) => Some(idx),
                    _ => None,
                })
        } else {
            None
        };
        if self.hover_app_index != target {
            self.hover_app_index = target;
            return true;
        }
        false
    }

    fn launch_desktop_app(app: DesktopApp, ipc: &mut crate::IpcClient) {
        if app.program.trim().is_empty() {
            warn!("ignoring launch request for {}: empty argv", app.name);
            return;
        }

        info!(
            "requesting launch: {} (program={} args={:?})",
            app.name, app.program, app.args
        );
        let command = ShellCommand::LaunchApp {
            program: app.program.clone(),
            args: app.args.clone(),
            terminal: app.terminal,
        };
        if !ipc.send(&command) {
            warn!(
                "IPC unavailable, launching locally: program={} args={:?}",
                app.program, app.args
            );
            let mut local = if app.terminal {
                let Some(terminal_program) = terminal_program() else {
                    warn!(
                        "cannot launch terminal app {:?}: no terminal emulator found",
                        app.name
                    );
                    return;
                };
                let mut cmd = Command::new(terminal_program);
                cmd.arg("-e").arg(&app.program);
                cmd
            } else {
                Command::new(&app.program)
            };

            if let Ok(wayland_display) = std::env::var("WAYLAND_DISPLAY") {
                local.env("WAYLAND_DISPLAY", wayland_display);
            }
            if let Ok(xdg_runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
                local.env("XDG_RUNTIME_DIR", xdg_runtime_dir);
            }
            local
                .env("XDG_SESSION_TYPE", "wayland")
                .env("XDG_CURRENT_DESKTOP", "Meridian")
                .env("XDG_SESSION_DESKTOP", "meridian")
                .env("DESKTOP_SESSION", "meridian");
            if is_firefox_program(&app.program) && env::var_os("MOZ_ENABLE_WAYLAND").is_none() {
                local.env("MOZ_ENABLE_WAYLAND", "1");
            }

            match local.args(&app.args).spawn() {
                Ok(child) => info!("local launch pid: {}", child.id()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    warn!("failed to launch {}: program not found", app.name)
                }
                Err(err) => warn!("failed to launch {}: {}", app.name, err),
            }
        }
    }

    pub fn launch_app(&mut self, index: usize, ipc: &mut crate::IpcClient) {
        self.pending_action_confirmation = None;
        let apps = self.visible_apps().apps;
        if let Some(app) = apps.get(index).cloned() {
            Self::launch_desktop_app(app, ipc);
        }
    }

    pub fn launch_app_by_app_index(&mut self, app_index: usize, ipc: &mut crate::IpcClient) {
        self.pending_action_confirmation = None;
        if let Some(app) = self.apps.get(app_index).cloned() {
            Self::launch_desktop_app(app, ipc);
        }
    }

    fn activate_action(&mut self, action: LauncherAction) -> LauncherActionActivationResult {
        if self.pending_action_confirmation == Some(action) {
            return LauncherActionActivationResult::Confirmed;
        }
        self.pending_action_confirmation = Some(action);
        LauncherActionActivationResult::Armed
    }

    pub fn trigger_action(
        &mut self,
        action: LauncherAction,
        ipc: &mut crate::IpcClient,
    ) -> LauncherActionTriggerResult {
        match action {
            LauncherAction::ExitMeridian => match self.activate_action(action) {
                LauncherActionActivationResult::Armed => LauncherActionTriggerResult::Armed,
                LauncherActionActivationResult::Confirmed => {
                    info!("requesting compositor exit from launcher action");
                    if !ipc.send(&ShellCommand::Quit) {
                        warn!("failed to send compositor exit request: IPC unavailable");
                        self.pending_action_confirmation = Some(action);
                        return LauncherActionTriggerResult::Failed;
                    }
                    self.pending_action_confirmation = None;
                    LauncherActionTriggerResult::Sent
                }
            },
        }
    }

    pub fn handle_key(
        &mut self,
        ch: Option<char>,
        is_backspace: bool,
        is_enter: bool,
        is_escape: bool,
        is_up: bool,
        is_down: bool,
    ) -> LauncherInputResult {
        if !self.open {
            return LauncherInputResult::None;
        }

        if is_escape {
            if self.view() == LauncherView::AllApps {
                self.set_view(LauncherView::TileStart);
                return LauncherInputResult::Redraw;
            }
            self.close();
            return LauncherInputResult::Close;
        }

        if is_enter {
            return self
                .selected_entry()
                .map(|entry| match entry {
                    SelectedLauncherEntry::App(index) => LauncherInputResult::Launch(index),
                    SelectedLauncherEntry::Action(action) => LauncherInputResult::Action(action),
                })
                .unwrap_or(LauncherInputResult::None);
        }

        if is_up {
            if self.selected_index > 0 {
                self.selected_index -= 1;
                return LauncherInputResult::Redraw;
            }
            return LauncherInputResult::None;
        }

        if is_down {
            let visible_len = self.filtered_visible_count();
            if visible_len == 0 {
                return LauncherInputResult::None;
            }
            let max_idx = visible_len.saturating_sub(1);
            if self.selected_index < max_idx {
                self.selected_index += 1;
                return LauncherInputResult::Redraw;
            }
            return LauncherInputResult::None;
        }

        if is_backspace {
            self.pending_action_confirmation = None;
            self.query.pop();
            self.selected_index = 0;
            return LauncherInputResult::Redraw;
        }

        if let Some(ch) = ch {
            if !ch.is_control() {
                self.pending_action_confirmation = None;
                self.query.push(ch);
                self.selected_index = 0;
                return LauncherInputResult::Redraw;
            }
        }

        LauncherInputResult::None
    }

    pub fn set_sidebar_category_from_click(&mut self, raw: u8) -> bool {
        let Some(category) = SidebarCategory::from_click_id(raw) else {
            return false;
        };
        if self.sidebar_category != category {
            self.sidebar_category = category;
            self.selected_index = 0;
            self.pending_action_confirmation = None;
            return true;
        }
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LauncherActionActivationResult {
    Armed,
    Confirmed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherActionTriggerResult {
    Armed,
    Sent,
    Failed,
}

#[derive(Debug)]
pub enum LauncherInputResult {
    None,
    Redraw,
    Close,
    Launch(usize),
    Action(LauncherAction),
}

#[derive(Debug, Clone, Copy)]
struct LauncherLayout {
    card: Rect,
    layout: Rect,
    sidebar: Rect,
    content: Rect,
    header: Rect,
    search: Rect,
    results: Rect,
    footer: Rect,
    footer_left: Rect,
    footer_right: Rect,
}

fn compute_launcher_layout(width: u32, height: u32, footer_rows: usize) -> LauncherLayout {
    let card = Rect {
        x: tokens::launcher::OUTER_PADDING / 2,
        y: tokens::launcher::OUTER_PADDING / 2,
        w: width as i32 - tokens::launcher::OUTER_PADDING,
        h: height as i32 - tokens::launcher::OUTER_PADDING,
    };

    let layout = Rect {
        x: card.x + tokens::launcher::OUTER_PADDING / 2,
        y: card.y + tokens::launcher::OUTER_PADDING / 2,
        w: card.w - tokens::launcher::OUTER_PADDING,
        h: card.h - tokens::launcher::OUTER_PADDING,
    };

    let sidebar = Rect {
        x: layout.x,
        y: layout.y,
        w: tokens::launcher::SIDEBAR_W,
        h: layout.h,
    };

    let content_x = sidebar.x + sidebar.w + tokens::launcher::OUTER_PADDING;
    let content = Rect {
        x: content_x,
        y: layout.y,
        w: (layout.x + layout.w) - content_x,
        h: layout.h,
    };

    let header = Rect {
        x: content.x,
        y: content.y,
        w: content.w,
        h: tokens::launcher::HEADER_H,
    };

    let search = Rect {
        x: content.x,
        y: content.y + tokens::launcher::HEADER_H + 6,
        w: content.w,
        h: tokens::launcher::SEARCH_H,
    };

    let footer_rows = footer_rows as i32;
    let footer_h = if footer_rows == 0 {
        0
    } else {
        footer_rows * FOOTER_ACTION_BUTTON_H
            + footer_rows.saturating_sub(1) * tokens::launcher::ROW_GAP
            + FOOTER_BAR_V_PADDING * 2
    };
    let footer = Rect {
        x: content.x,
        y: layout.y + layout.h - footer_h - FOOTER_BOTTOM_MARGIN,
        w: content.w,
        h: footer_h,
    };

    let available_split_w = (footer.w - FOOTER_SECTION_GAP).max(0);
    let left_w = ((available_split_w / 3).max(FOOTER_LEFT_MIN_W)).min(available_split_w);
    let right_w = (available_split_w - left_w).max(0);
    let footer_left = Rect {
        x: footer.x,
        y: footer.y,
        w: left_w,
        h: footer.h,
    };
    let footer_right = Rect {
        x: footer_left.x + footer_left.w + FOOTER_SECTION_GAP,
        y: footer.y,
        w: right_w,
        h: footer.h,
    };

    let results_y = search.y + search.h + tokens::launcher::LIST_TOP_GAP + 6;
    let results = Rect {
        x: content.x,
        y: results_y,
        w: content.w,
        h: (footer.y - FOOTER_TOP_GAP - results_y).max(0),
    };

    LauncherLayout {
        card,
        layout,
        sidebar,
        content,
        header,
        search,
        results,
        footer,
        footer_left,
        footer_right,
    }
}

fn result_count_label(total: usize, buf: &mut [u8; 32]) -> &str {
    if total == 1 {
        return "1 result";
    }

    let mut n = total;
    let mut digits = [0u8; 20];
    let mut len = 0usize;
    loop {
        digits[len] = b'0' + (n % 10) as u8;
        len += 1;
        n /= 10;
        if n == 0 {
            break;
        }
    }

    let mut out_len = 0usize;
    for idx in (0..len).rev() {
        buf[out_len] = digits[idx];
        out_len += 1;
    }
    const SUFFIX: &[u8] = b" results";
    for byte in SUFFIX {
        buf[out_len] = *byte;
        out_len += 1;
    }

    std::str::from_utf8(&buf[..out_len]).unwrap_or("results")
}

pub fn draw_launcher(
    launcher_state: &mut LauncherState,
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    icon_cache: &IconCache,
    width: u32,
    height: u32,
) {
    if !launcher_state.open {
        return;
    }

    launcher_state.clicks.clear();
    match launcher_state.view() {
        LauncherView::TileStart => draw_tile_start_view(
            launcher_state,
            painter,
            font,
            theme,
            icon_cache,
            width,
            height,
        ),
        LauncherView::AllApps => draw_all_apps_view(
            launcher_state,
            painter,
            font,
            theme,
            icon_cache,
            width,
            height,
        ),
    }
}

fn draw_all_apps_view(
    launcher_state: &mut LauncherState,
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    icon_cache: &IconCache,
    width: u32,
    height: u32,
) {
    let colors = &theme.colors;
    painter.clear(colors.surface_alt);
    let actions = launcher_state.visible_actions();
    let layout = compute_launcher_layout(width, height, actions.len());
    painter.roundish_rect_with_radius(
        layout.card,
        colors.surface_alt,
        tokens::launcher::CARD_RADIUS,
    );
    subtle_border(painter, layout.card, theme);

    fill_surface_with_radius(
        painter,
        layout.sidebar,
        theme,
        SurfaceKind::Surface,
        tokens::launcher::SIDEBAR_RADIUS,
    );

    let separator_x = layout.content.x - (tokens::launcher::OUTER_PADDING / 2);
    painter.rect(
        Rect {
            x: separator_x,
            y: layout.layout.y + 6,
            w: 1,
            h: (layout.layout.h - 12).max(0),
        },
        colors.border,
    );

    let visible = launcher_state.visible_apps();
    let apps = visible.apps;
    let results_total = visible.total_results + actions.len();
    let pinned_count = visible.pinned_count;
    let selected_idx = launcher_state.selected_index_clamped(apps.len() + actions.len());

    painter.text_clipped(
        font,
        "Launcher",
        layout.header.x,
        layout.header.y + HEADER_TITLE_BASELINE_OFFSET,
        (layout.header.w
            - HEADER_COUNT_WIDTH
            - HEADER_COUNT_RIGHT_INSET
            - HEADER_TITLE_TO_COUNT_GAP)
            .max(0),
        colors.text,
    );
    let mut count_text_buf = [0u8; 32];
    let count_text = result_count_label(results_total, &mut count_text_buf);
    painter.text_clipped(
        font,
        count_text,
        layout.header.x + layout.header.w - HEADER_COUNT_WIDTH - HEADER_COUNT_RIGHT_INSET,
        layout.header.y + HEADER_COUNT_BASELINE_OFFSET,
        HEADER_COUNT_WIDTH,
        colors.border,
    );

    fill_surface_with_radius(
        painter,
        layout.search,
        theme,
        SurfaceKind::Surface,
        tokens::launcher::SEARCH_RADIUS,
    );
    subtle_border(painter, layout.search, theme);
    draw_active_indicator(painter, layout.search, ActiveIndicatorEdge::Bottom, theme);
    let query_text = if launcher_state.query.is_empty() {
        "Search apps by name or executable"
    } else {
        &launcher_state.query
    };
    let query_color = if launcher_state.query.is_empty() {
        colors.border
    } else {
        colors.text
    };
    painter.text_clipped(
        font,
        query_text,
        layout.search.x + tokens::launcher::INNER_PADDING,
        layout.search.y + SEARCH_TEXT_BASELINE_OFFSET,
        layout.search.w - tokens::launcher::INNER_PADDING * 2,
        query_color,
    );

    let mut y = layout.results.y;

    let mut sidebar_item_y = layout.sidebar.y + SIDEBAR_TOP_PADDING;
    let mut all_apps_label_bottom = sidebar_item_y;
    for category in [SidebarCategory::Favorites, SidebarCategory::AllApps] {
        let label_rect = Rect {
            x: layout.sidebar.x + SIDEBAR_ITEM_X_INSET,
            y: sidebar_item_y,
            w: layout.sidebar.w - SIDEBAR_ITEM_W_INSET,
            h: SIDEBAR_ITEM_H,
        };
        let is_active =
            launcher_state.query.is_empty() && launcher_state.sidebar_category == category;
        let label_state = if is_active {
            InteractiveState::Selected
        } else {
            InteractiveState::Default
        };
        let default_color = if category == SidebarCategory::Favorites {
            colors.text
        } else {
            colors.border
        };
        let label_color = match label_state {
            InteractiveState::Default => default_color,
            InteractiveState::Selected => {
                draw_sidebar_item(painter, label_rect, theme, label_state)
            }
        };
        painter.text_clipped(
            font,
            category.label(),
            label_rect.x + SIDEBAR_TEXT_X_OFFSET,
            label_rect.y + SIDEBAR_TEXT_BASELINE_OFFSET,
            label_rect.w - SIDEBAR_TEXT_X_OFFSET * 2,
            label_color,
        );
        launcher_state.clicks.push(ClickZone {
            rect: label_rect,
            action: ClickAction::SelectLauncherCategory(category.to_click_id()),
        });
        sidebar_item_y = label_rect.y + label_rect.h + SIDEBAR_ITEM_GAP;
        all_apps_label_bottom = label_rect.y + label_rect.h;
    }

    let categories_top = all_apps_label_bottom + SIDEBAR_SECTION_GAP;
    painter.rect(
        Rect {
            x: layout.sidebar.x + SIDEBAR_DIVIDER_X_INSET,
            y: categories_top + SIDEBAR_DIVIDER_MARGIN_TOP,
            w: layout.sidebar.w - SIDEBAR_DIVIDER_X_INSET * 2,
            h: 1,
        },
        colors.border,
    );
    let mut category_row_y =
        categories_top + SIDEBAR_DIVIDER_MARGIN_TOP + SIDEBAR_DIVIDER_TO_LIST_GAP;
    for category in [
        SidebarCategory::Development,
        SidebarCategory::Internet,
        SidebarCategory::System,
        SidebarCategory::Utilities,
        SidebarCategory::Graphics,
        SidebarCategory::Games,
    ] {
        let label_rect = Rect {
            x: layout.sidebar.x + SIDEBAR_ITEM_X_INSET,
            y: category_row_y,
            w: layout.sidebar.w - SIDEBAR_ITEM_W_INSET,
            h: SIDEBAR_ITEM_H,
        };
        let is_active =
            launcher_state.query.is_empty() && launcher_state.sidebar_category == category;
        let label_state = if is_active {
            InteractiveState::Selected
        } else {
            InteractiveState::Default
        };
        let label_color = match label_state {
            InteractiveState::Default => colors.border,
            InteractiveState::Selected => {
                draw_sidebar_item(painter, label_rect, theme, label_state)
            }
        };
        painter.text_clipped(
            font,
            category.label(),
            label_rect.x + SIDEBAR_TEXT_X_OFFSET,
            label_rect.y + SIDEBAR_TEXT_BASELINE_OFFSET,
            label_rect.w - SIDEBAR_TEXT_X_OFFSET * 2,
            label_color,
        );
        launcher_state.clicks.push(ClickZone {
            rect: label_rect,
            action: ClickAction::SelectLauncherCategory(category.to_click_id()),
        });
        category_row_y += SIDEBAR_ITEM_H + SIDEBAR_ITEM_GAP;
    }

    if apps.is_empty() && actions.is_empty() {
        let empty = if launcher_state.query.is_empty() {
            "No applications found"
        } else {
            "No results. Refine your search"
        };
        let empty_rect = Rect {
            x: layout.content.x,
            y,
            w: layout.content.w,
            h: tokens::launcher::APP_ROW_H,
        };
        fill_surface_with_radius(
            painter,
            empty_rect,
            theme,
            SurfaceKind::Surface,
            tokens::launcher::LIST_ROW_RADIUS,
        );
        painter.text_clipped(
            font,
            empty,
            empty_rect.x + tokens::launcher::INNER_PADDING,
            empty_rect.y + 24,
            empty_rect.w - tokens::launcher::INNER_PADDING * 2,
            colors.border,
        );
        return;
    }

    let show_pinned_grid = launcher_state.query.is_empty()
        && launcher_state.sidebar_category == SidebarCategory::Favorites
        && pinned_count > 0;
    if show_pinned_grid {
        painter.text_clipped(
            font,
            "Pinned",
            layout.content.x,
            y + 13,
            layout.content.w,
            colors.border,
        );
        y += tokens::launcher::SECTION_LABEL_H + 2;

        let card_w =
            (layout.content.w - tokens::launcher::PINNED_GRID_COL_GAP) / PINNED_GRID_COLS as i32;
        for (index, app) in apps.iter().take(pinned_count).enumerate() {
            let row = index / PINNED_GRID_COLS;
            let col = index % PINNED_GRID_COLS;
            let rect = Rect {
                x: layout.content.x + col as i32 * (card_w + tokens::launcher::PINNED_GRID_COL_GAP),
                y: y + row as i32
                    * (tokens::launcher::PINNED_CARD_H + tokens::launcher::PINNED_GRID_ROW_GAP),
                w: card_w,
                h: tokens::launcher::PINNED_CARD_H,
            };
            let is_selected = index == selected_idx;
            let slot_x = rect.x + tokens::launcher::INNER_PADDING;
            let slot_y = rect.y + (rect.h - LAUNCHER_ICON_SLOT) / 2;
            let text_x = slot_x + LAUNCHER_ICON_SLOT + tokens::badge::CONTENT_GAP;
            let row_state = if is_selected {
                InteractiveState::Selected
            } else {
                InteractiveState::Default
            };
            let text_color = draw_list_item(painter, rect, theme, row_state, false);
            let icon_slot_rect = Rect {
                x: slot_x,
                y: slot_y,
                w: LAUNCHER_ICON_SLOT,
                h: LAUNCHER_ICON_SLOT,
            };
            draw_launcher_app_visual(
                painter,
                font,
                icon_cache,
                icon_slot_rect,
                app,
                theme,
                row_state,
            );
            painter.text_clipped(
                font,
                &app.name,
                text_x,
                rect.y + PINNED_APP_TITLE_BASELINE_OFFSET,
                rect.w - (text_x - rect.x) - tokens::launcher::INNER_PADDING,
                text_color,
            );
            launcher_state.clicks.push(ClickZone {
                rect,
                action: ClickAction::LaunchApp(index),
            });
        }

        let pinned_rows = pinned_count.div_ceil(PINNED_GRID_COLS) as i32;
        y += pinned_rows * tokens::launcher::PINNED_CARD_H
            + (pinned_rows.saturating_sub(1)) * tokens::launcher::PINNED_GRID_ROW_GAP;
        y += 10;
    }

    if !show_pinned_grid {
        for (index, app) in apps.iter().enumerate() {
            let is_selected = index == selected_idx;
            let rect = Rect {
                x: layout.content.x,
                y,
                w: layout.content.w,
                h: tokens::launcher::APP_ROW_H,
            };
            let row_state = if is_selected {
                InteractiveState::Selected
            } else {
                InteractiveState::Default
            };
            let text_color = draw_list_item(painter, rect, theme, row_state, false);
            let slot_x = rect.x + tokens::launcher::INNER_PADDING;
            let slot_y = rect.y + (rect.h - LAUNCHER_ICON_SLOT) / 2;
            let text_x = slot_x + LAUNCHER_ICON_SLOT + tokens::badge::CONTENT_GAP;
            let icon_slot_rect = Rect {
                x: slot_x,
                y: slot_y,
                w: LAUNCHER_ICON_SLOT,
                h: LAUNCHER_ICON_SLOT,
            };
            draw_launcher_app_visual(
                painter,
                font,
                icon_cache,
                icon_slot_rect,
                app,
                theme,
                row_state,
            );
            let exec_hint = Path::new(&app.program)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&app.program);
            painter.text_clipped(
                font,
                &app.name,
                text_x,
                rect.y + APP_ROW_TITLE_BASELINE_OFFSET,
                rect.w - (text_x - rect.x) - tokens::launcher::INNER_PADDING,
                text_color,
            );
            painter.text_clipped(
                font,
                exec_hint,
                text_x,
                rect.y + APP_ROW_SUBTITLE_BASELINE_OFFSET,
                rect.w - (text_x - rect.x) - tokens::launcher::INNER_PADDING,
                if is_selected {
                    SELECTED_EXEC_HINT_COLOR
                } else {
                    colors.border
                },
            );
            launcher_state.clicks.push(ClickZone {
                rect,
                action: ClickAction::LaunchApp(index),
            });
            y += tokens::launcher::APP_ROW_H + tokens::launcher::ROW_GAP;
        }
    }

    if !actions.is_empty() {
        let current_mode = launcher_state.current_mode();
        let footer_y = layout.footer.y + FOOTER_BAR_V_PADDING;
        let control_center_y = layout.footer.y + (layout.footer.h - FOOTER_MODE_PILL_H) / 2;
        fill_surface_with_radius(
            painter,
            layout.footer,
            theme,
            SurfaceKind::Background,
            tokens::launcher::LIST_ROW_RADIUS,
        );
        painter.rect(
            Rect {
                x: layout.footer.x,
                y: layout.footer.y - (FOOTER_TOP_GAP / 2).max(1),
                w: layout.footer.w,
                h: 1,
            },
            colors.border,
        );

        let mode_pill_rect = Rect {
            x: layout.footer_left.x + tokens::launcher::INNER_PADDING,
            y: control_center_y,
            w: FOOTER_MODE_PILL_W.min(
                (layout.footer_left.w - tokens::launcher::INNER_PADDING * 2)
                    .max(FOOTER_MODE_PILL_W / 2),
            ),
            h: FOOTER_MODE_PILL_H,
        };
        fill_surface_with_radius(
            painter,
            mode_pill_rect,
            theme,
            SurfaceKind::Accent,
            tokens::launcher::LIST_ROW_RADIUS,
        );
        painter.text_clipped(
            font,
            current_mode.label(),
            mode_pill_rect.x + 12,
            mode_pill_rect.y + 18,
            mode_pill_rect.w - 24,
            crate::ui::primitives::active_accent_foreground(),
        );

        let mut action_y = footer_y;
        let mut primary_action_rect = None;
        for (offset, action) in actions.iter().enumerate() {
            let index = apps.len() + offset;
            let is_selected = index == selected_idx;
            let awaiting_confirmation = launcher_state.pending_action_confirmation == Some(*action);
            let action_button_available =
                (layout.footer_right.w - tokens::launcher::INNER_PADDING * 2).max(0);
            let action_button_w = if action_button_available < FOOTER_ACTION_BUTTON_MIN_W {
                action_button_available
            } else {
                action_button_available
                    .clamp(FOOTER_ACTION_BUTTON_MIN_W, FOOTER_ACTION_BUTTON_MAX_W)
            };
            let rect = Rect {
                x: layout.footer_right.x + layout.footer_right.w - action_button_w,
                y: action_y,
                w: action_button_w,
                h: FOOTER_ACTION_BUTTON_H,
            };
            if primary_action_rect.is_none() {
                primary_action_rect = Some(rect);
            }
            let row_state = if is_selected {
                InteractiveState::Selected
            } else {
                InteractiveState::Default
            };
            let text_color = draw_panel_button(painter, rect, theme, row_state, false);
            let label = if awaiting_confirmation {
                action.confirm_label()
            } else {
                action.label()
            };
            painter.text_clipped(
                font,
                label,
                rect.x + tokens::launcher::INNER_PADDING,
                rect.y + 18,
                rect.w - tokens::launcher::INNER_PADDING * 2,
                text_color,
            );
            launcher_state.clicks.push(ClickZone {
                rect,
                action: ClickAction::LauncherAction {
                    action: *action,
                    index,
                },
            });
            action_y += tokens::launcher::APP_ROW_H + tokens::launcher::ROW_GAP;
        }

        if let Some(action_rect) = primary_action_rect {
            let right_min_x = layout.footer_right.x + tokens::launcher::INNER_PADDING;
            let mut back_rect = Rect {
                x: action_rect.x - FOOTER_SECTION_GAP - ALL_APPS_BACK_BUTTON_W,
                y: footer_y,
                w: ALL_APPS_BACK_BUTTON_W,
                h: FOOTER_ACTION_BUTTON_H,
            };
            if back_rect.x < right_min_x {
                let fallback_w =
                    (layout.footer_left.w - tokens::launcher::INNER_PADDING * 2).max(0);
                if fallback_w > 0 {
                    back_rect = Rect {
                        x: layout.footer_left.x + tokens::launcher::INNER_PADDING,
                        y: footer_y,
                        w: fallback_w,
                        h: FOOTER_ACTION_BUTTON_H,
                    };
                } else {
                    back_rect.w = 0;
                }
            }

            if back_rect.w > 0 {
                let text_color =
                    draw_panel_button(painter, back_rect, theme, InteractiveState::Default, false);
                painter.text_clipped(
                    font,
                    "< Tiles",
                    back_rect.x + tokens::launcher::INNER_PADDING,
                    back_rect.y + 18,
                    back_rect.w - tokens::launcher::INNER_PADDING * 2,
                    text_color,
                );
                launcher_state.clicks.push(ClickZone {
                    rect: back_rect,
                    action: ClickAction::SetLauncherView(LauncherView::TileStart),
                });
            }
        }
    }
}

fn draw_tile_start_view(
    launcher_state: &mut LauncherState,
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    icon_cache: &IconCache,
    width: u32,
    height: u32,
) {
    let colors = &theme.colors;
    painter.clear(colors.surface_alt);

    let results_total = launcher_state.apps.len() + 1;

    let card = Rect {
        x: tokens::launcher::OUTER_PADDING / 2,
        y: tokens::launcher::OUTER_PADDING / 2,
        w: width as i32 - tokens::launcher::OUTER_PADDING,
        h: height as i32 - tokens::launcher::OUTER_PADDING,
    };
    let content = Rect {
        x: card.x + tokens::launcher::OUTER_PADDING / 2,
        y: card.y + tokens::launcher::OUTER_PADDING / 2,
        w: card.w - tokens::launcher::OUTER_PADDING,
        h: card.h - tokens::launcher::OUTER_PADDING,
    };
    let header = Rect {
        x: content.x,
        y: content.y,
        w: content.w,
        h: tokens::launcher::HEADER_H,
    };
    let search = Rect {
        x: content.x,
        y: content.y + tokens::launcher::HEADER_H + 6,
        w: content.w,
        h: tokens::launcher::SEARCH_H,
    };
    let footer_h = FOOTER_ACTION_BUTTON_H + FOOTER_BAR_V_PADDING * 2;
    let footer = Rect {
        x: content.x,
        y: content.y + content.h - footer_h - FOOTER_BOTTOM_MARGIN,
        w: content.w,
        h: footer_h,
    };
    let tile_area = Rect {
        x: content.x,
        y: search.y + search.h + tokens::launcher::LIST_TOP_GAP + 6,
        w: content.w,
        h: (footer.y - FOOTER_TOP_GAP - (search.y + search.h + tokens::launcher::LIST_TOP_GAP + 6))
            .max(0),
    };

    painter.rect(card, colors.surface_alt);
    subtle_border(painter, card, theme);

    painter.rect(tile_area, colors.surface_alt);
    let geo = compute_tile_grid_geometry(tile_area);
    debug_assert!(geo.rows >= 1);
    launcher_state.tile_viewport_h_cache = tile_area.h;

    // Total content height = sum over sections of (header + header/tiles gap +
    // section body) plus a SECTION_GAP between sections.
    let mut content_h = 0i32;
    for (idx, section) in launcher_state.app_sections.iter().enumerate() {
        if idx > 0 {
            content_h += SECTION_GAP;
        }
        content_h += SECTION_HEADER_H + SECTION_HEADER_TO_TILES_GAP;
        if section.rows > 0 {
            content_h += section.rows as i32 * geo.slot_h + (section.rows as i32 - 1) * geo.gap;
        }
    }
    launcher_state.tile_content_h_cache = content_h;
    let max_scroll = (content_h - tile_area.h).max(0);
    launcher_state.tile_scroll_y = launcher_state.tile_scroll_y.clamp(0, max_scroll);

    // Walk sections top-to-bottom and emit header + tiles, applying scroll.
    let mut section_top = tile_area.y - launcher_state.tile_scroll_y;
    for (idx, section) in launcher_state.app_sections.iter().enumerate() {
        if idx > 0 {
            section_top += SECTION_GAP;
        }
        let header_rect = Rect {
            x: tile_area.x,
            y: section_top,
            w: tile_area.w,
            h: SECTION_HEADER_H,
        };
        let body_top = section_top + SECTION_HEADER_H + SECTION_HEADER_TO_TILES_GAP;
        let body_h = if section.rows > 0 {
            section.rows as i32 * geo.slot_h + (section.rows as i32 - 1) * geo.gap
        } else {
            0
        };

        // Render header only if any part of it is on-screen inside tile_area.
        if header_rect.y + header_rect.h > tile_area.y && header_rect.y < tile_area.y + tile_area.h
        {
            let mut letter_buf = [0u8; 4];
            let letter_str = section.letter.encode_utf8(&mut letter_buf);
            painter.text_clipped(
                font,
                letter_str,
                header_rect.x + SECTION_HEADER_TEXT_X_INSET,
                header_rect.y + SECTION_HEADER_BASELINE_OFFSET,
                header_rect.w - SECTION_HEADER_TEXT_X_INSET * 2,
                theme.colors.text,
            );
            painter.rect(
                Rect {
                    x: header_rect.x,
                    y: header_rect.y + header_rect.h - SECTION_HEADER_UNDERLINE_H,
                    w: header_rect.w,
                    h: SECTION_HEADER_UNDERLINE_H,
                },
                theme.colors.accent,
            );
        }

        // Render tiles of this section (skip if body is fully off-screen).
        if body_top + body_h > tile_area.y && body_top < tile_area.y + tile_area.h {
            for tile in &section.tiles {
                if tile.col >= geo.cols {
                    continue;
                }
                let tile_x = geo.origin_x + tile.col as i32 * (geo.slot_w + geo.gap);
                let tile_y = body_top + tile.row as i32 * (geo.slot_h + geo.gap);
                let (cw, rh) = tile.size.grid_units();
                let tile_w = geo.slot_w * cw as i32 + geo.gap * (cw as i32 - 1);
                let tile_h = geo.slot_h * rh as i32 + geo.gap * (rh as i32 - 1);
                let rect = Rect {
                    x: tile_x,
                    y: tile_y,
                    w: tile_w,
                    h: tile_h,
                };
                if rect.y + rect.h <= tile_area.y || rect.y >= tile_area.y + tile_area.h {
                    continue;
                }

                let is_hovered = launcher_state.hover_app_index == Some(tile.app_index);
                painter.rect(rect, theme.colors.surface);
                subtle_border(painter, rect, theme);
                let indicator_color = if is_hovered {
                    theme.colors.accent_alt
                } else {
                    theme.colors.accent
                };
                painter.rect(
                    Rect {
                        x: rect.x,
                        y: rect.y,
                        w: rect.w,
                        h: 3,
                    },
                    indicator_color,
                );

                let Some(app) = launcher_state.apps.get(tile.app_index) else {
                    continue;
                };
                let icon_size = match tile.size {
                    TileSize::Small => TILE_ICON_SIZE_SMALL,
                    TileSize::Medium => TILE_ICON_SIZE_MEDIUM,
                    TileSize::Wide => TILE_ICON_SIZE_WIDE,
                };
                let icon_size_i32 = icon_size as i32;
                let icon_y = rect.y + (rect.h - TILE_LABEL_H - icon_size_i32).max(0) / 2;
                let icon_x = rect.x + (rect.w - icon_size_i32) / 2;
                let icon_rect = Rect {
                    x: icon_x,
                    y: icon_y,
                    w: icon_size_i32,
                    h: icon_size_i32,
                };
                if let Some(image) = app
                    .icon_name
                    .as_deref()
                    .and_then(|name| icon_cache.lookup(name, icon_size))
                {
                    painter.draw_image(icon_rect, image);
                } else {
                    let initial = app_initial_char(&app.name);
                    let mut initial_buf = [0u8; 4];
                    let label = initial.encode_utf8(&mut initial_buf);
                    painter.text_centered(font, label, icon_rect, theme.colors.text);
                }

                let label_rect = Rect {
                    x: rect.x,
                    y: rect.y + rect.h - TILE_LABEL_H,
                    w: rect.w,
                    h: TILE_LABEL_H,
                };
                painter.text_centered(font, &app.name, label_rect, theme.colors.text);
                launcher_state.clicks.push(ClickZone {
                    rect,
                    action: ClickAction::LaunchApp(tile.app_index),
                });
            }
        }

        section_top = body_top + body_h;
    }

    if content_h > tile_area.h {
        let track = Rect {
            x: tile_area.x + tile_area.w - 6,
            y: tile_area.y,
            w: 6,
            h: tile_area.h,
        };
        painter.rect(track, theme.colors.border);
        let handle_h =
            ((tile_area.h as f32 * tile_area.h as f32) / content_h as f32).max(20.0) as i32;
        let scroll_range = (content_h - tile_area.h).max(1);
        let handle_y = tile_area.y
            + ((tile_area.h - handle_h) as f32
                * (launcher_state.tile_scroll_y as f32 / scroll_range as f32)) as i32;
        painter.rect(
            Rect {
                x: track.x,
                y: handle_y,
                w: track.w,
                h: handle_h,
            },
            theme.colors.accent,
        );
    }

    // Cover bands above and below tile_area, spanning the full canvas
    // width: tiles that scrolled into the header/search/footer region (or
    // into the OUTER_PADDING ring between the card border and the canvas
    // edge) get painted over with surface_alt before the header/search/
    // footer redraw on top.
    if tile_area.y > 0 {
        painter.rect(
            Rect {
                x: 0,
                y: 0,
                w: width as i32,
                h: tile_area.y,
            },
            colors.surface_alt,
        );
    }
    let bottom_cover_y = tile_area.y + tile_area.h;
    if bottom_cover_y < height as i32 {
        painter.rect(
            Rect {
                x: 0,
                y: bottom_cover_y,
                w: width as i32,
                h: height as i32 - bottom_cover_y,
            },
            colors.surface_alt,
        );
    }
    // Cover bands erase the card border on the top/bottom edges; redraw it.
    subtle_border(painter, card, theme);

    painter.text_clipped(
        font,
        "Launcher",
        header.x,
        header.y + HEADER_TITLE_BASELINE_OFFSET,
        (header.w - HEADER_COUNT_WIDTH - HEADER_COUNT_RIGHT_INSET - HEADER_TITLE_TO_COUNT_GAP)
            .max(0),
        colors.text,
    );
    let mut count_text_buf = [0u8; 32];
    let count_text = result_count_label(results_total, &mut count_text_buf);
    painter.text_clipped(
        font,
        count_text,
        header.x + header.w - HEADER_COUNT_WIDTH - HEADER_COUNT_RIGHT_INSET,
        header.y + HEADER_COUNT_BASELINE_OFFSET,
        HEADER_COUNT_WIDTH,
        colors.border,
    );

    painter.rect(search, colors.surface);
    subtle_border(painter, search, theme);
    draw_active_indicator(painter, search, ActiveIndicatorEdge::Bottom, theme);
    let query_text = if launcher_state.query.is_empty() {
        "Search apps by name or executable"
    } else {
        &launcher_state.query
    };
    let query_color = if launcher_state.query.is_empty() {
        colors.border
    } else {
        colors.text
    };
    painter.text_clipped(
        font,
        query_text,
        search.x + tokens::launcher::INNER_PADDING,
        search.y + SEARCH_TEXT_BASELINE_OFFSET,
        search.w - tokens::launcher::INNER_PADDING * 2,
        query_color,
    );

    fill_surface_with_radius(
        painter,
        footer,
        theme,
        SurfaceKind::Background,
        tokens::launcher::LIST_ROW_RADIUS,
    );
    painter.rect(
        Rect {
            x: footer.x,
            y: footer.y - (FOOTER_TOP_GAP / 2).max(1),
            w: footer.w,
            h: 1,
        },
        colors.border,
    );

    let button_available = (footer.w - tokens::launcher::INNER_PADDING * 2).max(0);
    let button_w = TILE_START_SWITCH_BUTTON_W.min(button_available);
    if button_w > 0 {
        let button_rect = Rect {
            x: footer.x + footer.w - tokens::launcher::INNER_PADDING - button_w,
            y: footer.y + FOOTER_BAR_V_PADDING,
            w: button_w,
            h: FOOTER_ACTION_BUTTON_H,
        };
        let text_color = draw_panel_button(
            painter,
            button_rect,
            theme,
            InteractiveState::Default,
            false,
        );
        painter.text_clipped(
            font,
            "All apps >",
            button_rect.x + tokens::launcher::INNER_PADDING,
            button_rect.y + 18,
            button_rect.w - tokens::launcher::INNER_PADDING * 2,
            text_color,
        );
        launcher_state.clicks.push(ClickZone {
            rect: button_rect,
            action: ClickAction::SetLauncherView(LauncherView::AllApps),
        });
    }
}

fn draw_launcher_app_visual(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    icon_cache: &IconCache,
    slot_rect: Rect,
    app: &DesktopApp,
    theme: &ThemeConfig,
    row_state: InteractiveState,
) {
    if let Some(image) = app
        .icon_name
        .as_deref()
        .and_then(|name| icon_cache.lookup(name, LAUNCHER_ICON_SIZE))
    {
        painter.draw_image(slot_rect, image);
        return;
    }

    let initial = app_initial(&app.name);
    let badge_rect = Rect {
        x: slot_rect.x + (slot_rect.w - tokens::badge::SIZE) / 2,
        y: slot_rect.y + (slot_rect.h - tokens::badge::SIZE) / 2,
        w: tokens::badge::SIZE,
        h: tokens::badge::SIZE,
    };
    draw_initial_badge(painter, font, badge_rect, &initial, theme, row_state);
}

fn app_initial(name: &str) -> String {
    name.chars()
        .find(|ch| !ch.is_whitespace())
        .and_then(|ch| ch.to_uppercase().next())
        .map(|ch| ch.to_string())
        .unwrap_or_else(|| "?".to_string())
}

fn app_initial_char(name: &str) -> char {
    name.chars()
        .find(|ch| !ch.is_whitespace())
        .and_then(|ch| ch.to_uppercase().next())
        .unwrap_or('?')
}

fn app_exec_basename(program: &str) -> String {
    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program)
        .to_ascii_lowercase()
}

fn is_pinned_candidate(app: &DesktopApp) -> bool {
    let exec_base = app_exec_basename(&app.program);
    PINNED_CANDIDATES
        .iter()
        .any(|candidate| app.name_key == *candidate || exec_base == *candidate)
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        fs,
        path::PathBuf,
        sync::{
            atomic::{AtomicU64, Ordering as AtomicOrdering},
            Mutex, OnceLock,
        },
    };

    use super::{
        app_initial, compute_launcher_layout, compute_tile_grid_geometry, desktop_app_dirs,
        is_executable_available, pack_app_sections, parse_exec_argv, random_tile_size_for_app,
        section_letter_for, AppSection, AppTile, ClickAction, ClickZone, DesktopApp,
        LauncherAction, LauncherActionActivationResult, LauncherInputResult, LauncherState,
        LauncherView, Rect, SidebarCategory, TileSize, FOOTER_ACTION_BUTTON_H,
        FOOTER_BAR_V_PADDING, MAX_RESULTS, TILE_GRID_COLS, TILE_SLOT_MIN_PX, XDG_DATA_DIRS_DEFAULT,
    };
    use crate::{icons::IconCache, Painter};

    static TEST_ID: AtomicU64 = AtomicU64::new(1);
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct EnvVarGuard {
        key: &'static str,
        old: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let old = std::env::var_os(key);
            // SAFETY: tests mutate process env in a controlled scope and restore previous values in Drop.
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, old }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.old {
                Some(value) => {
                    // SAFETY: this restores the exact previously captured value for the same key.
                    unsafe {
                        std::env::set_var(self.key, value);
                    }
                }
                None => {
                    // SAFETY: key removal restores the pre-test absence captured in `old`.
                    unsafe {
                        std::env::remove_var(self.key);
                    }
                }
            }
        }
    }

    fn unique_test_dir(prefix: &str) -> PathBuf {
        let id = TEST_ID.fetch_add(1, AtomicOrdering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "meridian-launcher-tests-{}-{}-{}",
            prefix,
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    fn env_lock() -> &'static Mutex<()> {
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn write_desktop_entry(
        applications_dir: &std::path::Path,
        file_name: &str,
        name: &str,
        exec: &str,
    ) {
        fs::write(
            applications_dir.join(file_name),
            format!(
                r#"
[Desktop Entry]
Type=Application
Name={}
Exec={}
"#,
                name, exec
            ),
        )
        .expect("write desktop entry");
    }

    fn app_with_categories(name: &str, program: &str, categories: &[&str]) -> DesktopApp {
        let mut app = DesktopApp::new(name.to_string(), vec![program.to_string()], false);
        app.categories = categories.iter().map(|c| c.to_string()).collect();
        app
    }

    fn pinned_test_app(name: &str) -> DesktopApp {
        DesktopApp::new(name.to_string(), vec![name.to_lowercase()], false)
    }

    #[test]
    fn parses_valid_desktop_entry() {
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Calculator
Exec=gnome-calculator %U
Terminal=false
"#,
        )
        .expect("valid desktop entry");

        assert_eq!(app.name, "Calculator");
        assert_eq!(app.program, "gnome-calculator");
        assert!(app.args.is_empty());
        assert!(!app.terminal);
        assert!(app.categories.is_empty());
    }

    #[test]
    fn parses_categories_and_normalizes_to_lowercase_tokens() {
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Builder
Exec=builder
Categories=Development;IDE;
"#,
        )
        .expect("valid desktop entry with categories");

        assert_eq!(app.categories, vec!["development", "ide"]);
    }

    #[test]
    fn parses_categories_ignores_empty_tokens_and_whitespace() {
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Viewer
Exec=viewer
Categories=Utility; ;Graphics;;
"#,
        )
        .expect("valid desktop entry with mixed category separators");

        assert_eq!(app.categories, vec!["utility", "graphics"]);
    }

    #[test]
    fn missing_categories_defaults_to_empty_vec() {
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Terminal
Exec=foot
"#,
        )
        .expect("valid desktop entry without categories");

        assert!(app.categories.is_empty());
    }

    #[test]
    fn icon_field_theme_name_is_parsed() {
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Firefox
Exec=firefox
Icon=firefox
"#,
        )
        .expect("valid icon field");

        assert_eq!(app.icon_name.as_deref(), Some("firefox"));
    }

    #[test]
    fn icon_field_with_png_extension_is_stripped() {
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Firefox
Exec=firefox
Icon=firefox.png
"#,
        )
        .expect("valid png icon field");

        assert_eq!(app.icon_name.as_deref(), Some("firefox"));
    }

    #[test]
    fn icon_field_with_absolute_path_is_kept_verbatim() {
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Custom
Exec=custom
Icon=/usr/share/foo/icon.png
"#,
        )
        .expect("valid absolute icon path");

        assert_eq!(app.icon_name.as_deref(), Some("/usr/share/foo/icon.png"));
    }

    #[test]
    fn icon_field_empty_is_none() {
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Custom
Exec=custom
Icon=
"#,
        )
        .expect("valid empty icon field");

        assert_eq!(app.icon_name, None);
    }

    #[test]
    fn missing_icon_field_is_none() {
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Terminal
Exec=foot
"#,
        )
        .expect("valid desktop entry without icon");

        assert_eq!(app.icon_name, None);
    }

    #[test]
    fn launcher_state_new_with_apps_uses_provided_list() {
        let apps = vec![DesktopApp::new(
            "Provided".to_string(),
            vec!["provided-app".to_string()],
            false,
        )];
        let state = LauncherState::new_with_apps(apps);
        assert_eq!(state.apps.len(), 1);
        assert_eq!(state.apps[0].name, "Provided");
    }

    #[test]
    fn launcher_state_default_view_is_tile_start() {
        let state = LauncherState::new_with_apps(Vec::new());
        assert_eq!(state.view(), LauncherView::TileStart);
    }

    #[test]
    fn escape_in_all_apps_view_returns_to_tile_start_without_closing() {
        let mut state = LauncherState::new_with_apps(Vec::new());
        state.open = true;
        state.set_view(LauncherView::AllApps);

        let result = state.handle_key(None, false, false, true, false, false);

        assert!(matches!(result, LauncherInputResult::Redraw));
        assert!(state.open);
        assert_eq!(state.view(), LauncherView::TileStart);
    }

    #[test]
    fn escape_in_tile_start_closes_launcher() {
        let mut state = LauncherState::new_with_apps(Vec::new());
        state.open = true;
        assert_eq!(state.view(), LauncherView::TileStart);

        let result = state.handle_key(None, false, false, true, false, false);

        assert!(matches!(result, LauncherInputResult::Close));
        assert!(!state.open);
    }

    #[test]
    fn set_view_resets_selection_and_pending_confirmation() {
        let mut state = LauncherState::new_with_apps(Vec::new());
        state.open = true;
        state.selected_index = 5;
        state.pending_action_confirmation = Some(LauncherAction::ExitMeridian);

        let changed = state.set_view(LauncherView::AllApps);

        assert!(changed);
        assert_eq!(state.selected_index, 0);
        assert!(state.pending_action_confirmation().is_none());
    }

    #[test]
    fn tile_size_grid_units_matches_spec() {
        assert_eq!(TileSize::Small.grid_units(), (1, 1));
        assert_eq!(TileSize::Medium.grid_units(), (2, 2));
        assert_eq!(TileSize::Wide.grid_units(), (4, 2));
    }

    #[test]
    fn random_tile_size_is_deterministic_for_same_name() {
        let app = pinned_test_app("app042");
        let a = random_tile_size_for_app(&app);
        let b = random_tile_size_for_app(&app);
        assert_eq!(a, b);
    }

    #[test]
    fn random_tile_size_distribution_is_roughly_50_35_15() {
        let mut wide_count = 0usize;
        let mut medium_count = 0usize;
        let mut small_count = 0usize;
        for idx in 0..200 {
            let app = pinned_test_app(&format!("app{idx:03}"));
            match random_tile_size_for_app(&app) {
                TileSize::Wide => wide_count += 1,
                TileSize::Medium => medium_count += 1,
                TileSize::Small => small_count += 1,
            }
        }
        assert!(
            (10..=60).contains(&wide_count),
            "wide_count out of range: {wide_count}"
        );
        assert!(
            (50..=90).contains(&medium_count),
            "medium_count out of range: {medium_count}"
        );
        assert!(
            (70..=130).contains(&small_count),
            "small_count out of range: {small_count}"
        );
    }

    #[test]
    fn random_tile_size_two_different_names_can_differ() {
        let mut found = false;
        for a_idx in 0..40 {
            for b_idx in (a_idx + 1)..40 {
                let a = pinned_test_app(&format!("app{a_idx:03}"));
                let b = pinned_test_app(&format!("app{b_idx:03}"));
                if random_tile_size_for_app(&a) != random_tile_size_for_app(&b) {
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
        assert!(found, "expected at least one differing random tile size");
    }

    #[test]
    fn pack_app_sections_places_tiles_left_to_right_within_section() {
        // All names start with 'a' so they land in one section.
        let mut apps = Vec::new();
        for idx in 0..500 {
            let app = pinned_test_app(&format!("app{idx:03}"));
            if random_tile_size_for_app(&app) == TileSize::Small {
                apps.push(app);
            }
            if apps.len() == 6 {
                break;
            }
        }
        assert_eq!(apps.len(), 6, "need at least 6 small apps for this test");
        let sections = pack_app_sections(&apps);
        assert_eq!(sections.len(), 1);
        let tiles = &sections[0].tiles;
        assert_eq!(tiles.len(), 6);
        for (idx, tile) in tiles.iter().enumerate() {
            assert_eq!(tile.col, idx as u8);
            assert_eq!(tile.row, 0);
            assert_eq!(tile.size, TileSize::Small);
        }
    }

    #[test]
    fn pack_app_sections_wraps_to_next_row_when_row_full() {
        let mut apps = Vec::new();
        for idx in 0..600 {
            let app = pinned_test_app(&format!("app{idx:03}"));
            if random_tile_size_for_app(&app) == TileSize::Small {
                apps.push(app);
            }
            if apps.len() == 7 {
                break;
            }
        }
        assert_eq!(apps.len(), 7, "need at least 7 small apps for this test");
        let sections = pack_app_sections(&apps);
        assert_eq!(sections.len(), 1);
        let tiles = &sections[0].tiles;
        assert_eq!(tiles.len(), 7);
        assert_eq!(tiles[6].col, 0);
        assert_eq!(tiles[6].row, 1);
    }

    #[test]
    fn pack_app_sections_keeps_wide_in_one_row() {
        let mut wide = None;
        let mut small = Vec::new();
        for idx in 0..2000 {
            let app = pinned_test_app(&format!("app{idx:04}"));
            match random_tile_size_for_app(&app) {
                TileSize::Wide if wide.is_none() => wide = Some(app),
                TileSize::Small if small.len() < 2 => small.push(app),
                _ => {}
            }
            if wide.is_some() && small.len() == 2 {
                break;
            }
        }
        let wide = wide.expect("need one wide app");
        assert_eq!(small.len(), 2, "need two small apps");
        let sections = pack_app_sections(&[wide, small[0].clone(), small[1].clone()]);
        assert_eq!(sections.len(), 1);
        let tiles = &sections[0].tiles;
        assert_eq!(tiles.len(), 3);
        assert_eq!(tiles[0].size, TileSize::Wide);
        assert_eq!((tiles[0].col, tiles[0].row), (0, 0));
        assert_eq!((tiles[1].col, tiles[1].row), (4, 0));
        assert_eq!((tiles[2].col, tiles[2].row), (5, 0));
    }

    #[test]
    fn pack_app_sections_splits_by_starting_letter() {
        let apps = vec![
            pinned_test_app("alpha"),
            pinned_test_app("aardvark"),
            pinned_test_app("bravo"),
            pinned_test_app("charlie"),
        ];
        let sections = pack_app_sections(&apps);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].letter, 'A');
        assert_eq!(sections[0].tiles.len(), 2);
        assert_eq!(sections[1].letter, 'B');
        assert_eq!(sections[1].tiles.len(), 1);
        assert_eq!(sections[2].letter, 'C');
        assert_eq!(sections[2].tiles.len(), 1);
    }

    #[test]
    fn section_letter_for_uppercases_alpha_and_buckets_non_alpha() {
        assert_eq!(section_letter_for("alpha"), 'A');
        assert_eq!(section_letter_for("Æther"), '#'); // non-ascii bucket
        assert_eq!(section_letter_for("3d-tool"), '#');
        assert_eq!(section_letter_for(""), '#');
    }

    #[test]
    fn app_tile_count_matches_loaded_apps() {
        let apps = DesktopApp::load_system();
        let sections = pack_app_sections(&apps);
        let total: usize = sections.iter().map(|s| s.tiles.len()).sum();
        eprintln!("APP_TILE_COUNT={} SECTION_COUNT={}", total, sections.len());
        assert_eq!(total, apps.len());
    }

    #[test]
    fn compute_tile_grid_geometry_yields_six_columns_within_area() {
        let area = Rect {
            x: 0,
            y: 0,
            w: 820,
            h: 400,
        };
        let geo = compute_tile_grid_geometry(area);
        let used_w = geo.slot_w * TILE_GRID_COLS as i32 + geo.gap * (TILE_GRID_COLS as i32 - 1);
        assert_eq!(geo.cols, TILE_GRID_COLS);
        assert!(geo.slot_w >= TILE_SLOT_MIN_PX);
        assert!(used_w <= area.w);
    }

    #[test]
    fn compute_tile_grid_geometry_fills_available_width() {
        // 848 wide area with 6 cols + 5 gaps (=40) leaves 808px for slots,
        // so slot_w = 134. The packed grid_w should match within one column.
        let area = Rect {
            x: 50,
            y: 100,
            w: 848,
            h: 450,
        };
        let geo = compute_tile_grid_geometry(area);
        let grid_w = geo.slot_w * TILE_GRID_COLS as i32 + geo.gap * (TILE_GRID_COLS as i32 - 1);
        let leftover = area.w - grid_w;
        assert!(
            leftover < geo.slot_w,
            "leftover {} must be < one slot {} so the grid fills the area",
            leftover,
            geo.slot_w
        );
        assert!(
            geo.slot_w >= 100,
            "slot_w should be >= 100 at this width, got {}",
            geo.slot_w
        );
    }

    #[test]
    fn compute_tile_grid_geometry_centers_grid_horizontally() {
        let area = Rect {
            x: 50,
            y: 100,
            w: 848,
            h: 450,
        };
        let geo = compute_tile_grid_geometry(area);
        let grid_w = geo.slot_w * TILE_GRID_COLS as i32 + geo.gap * (TILE_GRID_COLS as i32 - 1);
        assert_eq!(geo.origin_x, area.x + (area.w - grid_w).max(0) / 2);
    }

    #[test]
    fn compute_tile_grid_geometry_yields_at_least_three_rows_for_phase2a_tile_area() {
        let area = Rect {
            x: 0,
            y: 0,
            w: 848,
            h: 450,
        };
        let geo = compute_tile_grid_geometry(area);
        assert!(
            geo.rows >= 3,
            "expected >=3 rows for tile_area at slot ~134, got {}",
            geo.rows
        );
    }

    #[test]
    fn scroll_tile_area_clamps_to_zero_when_content_fits_viewport() {
        let mut state = LauncherState::new_with_apps(Vec::new());
        assert!(!state.scroll_tile_area(120, 500, 400));
        assert_eq!(state.tile_scroll_y, 0);
    }

    #[test]
    fn scroll_tile_area_clamps_to_max_at_end() {
        let mut state = LauncherState::new_with_apps(Vec::new());
        assert!(state.scroll_tile_area(9_999, 200, 1_000));
        assert_eq!(state.tile_scroll_y, 800);
    }

    #[test]
    fn scroll_tile_area_returns_true_when_position_changes() {
        let mut state = LauncherState::new_with_apps(Vec::new());
        assert!(state.scroll_tile_area(60, 200, 1_000));
        assert_eq!(state.tile_scroll_y, 60);
    }

    #[test]
    fn wide_tile_dimensions_span_four_columns_and_two_rows() {
        let geo = compute_tile_grid_geometry(Rect {
            x: 10,
            y: 20,
            w: 820,
            h: 400,
        });
        let (cw, rh) = TileSize::Wide.grid_units();
        let w = geo.slot_w * cw as i32 + geo.gap * (cw as i32 - 1);
        let h = geo.slot_h * rh as i32 + geo.gap * (rh as i32 - 1);
        assert_eq!(w, geo.slot_w * 4 + geo.gap * 3);
        assert_eq!(h, geo.slot_h * 2 + geo.gap);
    }

    #[test]
    fn draw_launcher_app_visual_falls_back_to_initial_badge_on_cache_miss() {
        let app = DesktopApp::new(
            "Fallback".to_string(),
            vec!["fallback-app".to_string()],
            false,
        );
        let mut buffer = vec![0u8; 32 * 32 * 4];
        let mut painter = Painter::new(&mut buffer, 32, 32);
        let font = RefCell::new(None);
        let cache = IconCache::new();
        let theme = meridian_config::ThemeConfig::default();
        let slot = Rect {
            x: 4,
            y: 4,
            w: 24,
            h: 24,
        };
        super::draw_launcher_app_visual(
            &mut painter,
            &font,
            &cache,
            slot,
            &app,
            &theme,
            super::InteractiveState::Default,
        );

        let center_x = slot.x + slot.w / 2;
        let center_y = slot.y + slot.h / 2;
        let offset = ((center_y * 32 + center_x) * 4) as usize;
        assert_ne!(&buffer[offset..offset + 4], &[0, 0, 0, 0]);
    }

    #[test]
    fn rejects_hidden_nodisplay_and_non_application_entries() {
        let hidden = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Hidden
Exec=hidden
Hidden=true
"#,
        );
        assert!(hidden.is_err());

        let nodisplay = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=NoDisplay
Exec=nodisplay
NoDisplay=true
"#,
        );
        assert!(nodisplay.is_err());

        let link_type = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Link
Name=Link
Exec=link
"#,
        );
        assert!(link_type.is_err());
    }

    #[test]
    fn only_show_in_with_meridian_is_allowed() {
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Allowed
Exec=allowed
OnlyShowIn=GNOME;Meridian;KDE;
"#,
        );
        assert!(app.is_ok());
    }

    #[test]
    fn only_show_in_without_meridian_is_ignored() {
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Blocked
Exec=blocked
OnlyShowIn=GNOME;KDE;
"#,
        );
        assert!(app.is_err());
    }

    #[test]
    fn not_show_in_with_meridian_is_ignored() {
        let app = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=Blocked
Exec=blocked
NotShowIn=Meridian;XFCE;
"#,
        );
        assert!(app.is_err());
    }

    #[test]
    fn rejects_empty_name_or_exec() {
        let missing_name = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Exec=app
"#,
        );
        assert!(missing_name.is_err());

        let missing_exec = DesktopApp::from_desktop_entry_str_with_reason(
            r#"
[Desktop Entry]
Type=Application
Name=App
Exec=%U
"#,
        );
        assert!(missing_exec.is_err());
    }

    #[test]
    fn load_from_dirs_sorts_stably_and_ignores_invalid_entries() {
        let dir = unique_test_dir("scan");

        fs::write(
            dir.join("03-invalid.desktop"),
            r#"
[Desktop Entry]
Type=Application
Name=Invalid
Exec=%U
"#,
        )
        .expect("write invalid");

        fs::write(
            dir.join("02-zeta.desktop"),
            r#"
[Desktop Entry]
Type=Application
Name=Zeta
Exec=zeta
"#,
        )
        .expect("write zeta");

        fs::write(
            dir.join("01-alpha.desktop"),
            r#"
[Desktop Entry]
Type=Application
Name=alpha
Exec=alpha
"#,
        )
        .expect("write alpha");

        let apps = DesktopApp::load_from_dirs(vec![dir.clone()]);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].name, "alpha");
        assert_eq!(apps[1].name, "Zeta");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn load_from_dirs_deduplicates_same_name_and_exec() {
        let dir = unique_test_dir("dedupe");

        fs::write(
            dir.join("app-a.desktop"),
            r#"
[Desktop Entry]
Type=Application
Name=Viewer
Exec=viewer %F
"#,
        )
        .expect("write app a");

        fs::write(
            dir.join("app-b.desktop"),
            r#"
[Desktop Entry]
Type=Application
Name=viewer
Exec=viewer %U
"#,
        )
        .expect("write app b");

        let apps = DesktopApp::load_from_dirs(vec![dir.clone()]);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name.to_lowercase(), "viewer");
        assert_eq!(apps[0].program, "viewer");
        assert!(apps[0].args.is_empty());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn filter_is_case_insensitive_and_checks_name_and_exec() {
        let apps = vec![
            DesktopApp::new("Firefox".to_string(), vec!["firefox".to_string()], false),
            DesktopApp::new("Terminal".to_string(), vec!["foot".to_string()], true),
        ];

        let query = "FIRE";
        let query = query.to_lowercase();
        let filtered = apps
            .iter()
            .filter(|app| app.matches_query(&query))
            .collect::<Vec<_>>();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Firefox");

        let query = "FOOT".to_lowercase();
        let filtered = apps
            .iter()
            .filter(|app| app.matches_query(&query))
            .collect::<Vec<_>>();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Terminal");
    }

    #[test]
    fn exec_field_codes_are_removed() {
        let argv = parse_exec_argv(r#"app --open %f --name %c %%"#);
        assert_eq!(argv, vec!["app", "--open", "--name", "%"]);
    }

    #[test]
    fn exec_simple_quotes_are_handled() {
        let argv = parse_exec_argv(r#"myapp --title "Hello World" --class 'Meridian App'"#);
        assert_eq!(
            argv,
            vec!["myapp", "--title", "Hello World", "--class", "Meridian App"]
        );
    }

    #[test]
    fn exec_empty_is_rejected() {
        let argv = parse_exec_argv("%f %u %i %c %k");
        assert!(argv.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn try_exec_accepts_absolute_executable_and_path_lookup() {
        use std::os::unix::fs::PermissionsExt;
        let _env_lock = env_lock().lock().expect("env lock");

        let bin_dir = unique_test_dir("tryexec-bin");
        let bin = bin_dir.join("demo-bin");
        fs::write(&bin, "#!/bin/sh\nexit 0\n").expect("write script");
        let mut perms = fs::metadata(&bin).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&bin, perms).expect("chmod");

        assert!(is_executable_available(
            bin.to_str().expect("absolute path utf8")
        ));

        let _path = EnvVarGuard::set("PATH", bin_dir.to_str().expect("path utf8"));
        assert!(is_executable_available("demo-bin"));

        let _ = fs::remove_dir_all(bin_dir);
    }

    #[cfg(unix)]
    #[test]
    fn try_exec_rejects_file_without_execute_bit() {
        use std::os::unix::fs::PermissionsExt;

        let bin_dir = unique_test_dir("tryexec-noexec");
        let bin = bin_dir.join("demo-noexec");
        fs::write(&bin, "echo nope\n").expect("write file");
        let mut perms = fs::metadata(&bin).expect("metadata").permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&bin, perms).expect("chmod");

        assert!(!is_executable_available(
            bin.to_str().expect("absolute path utf8")
        ));

        let _ = fs::remove_dir_all(bin_dir);
    }

    #[test]
    fn desktop_app_dirs_uses_xdg_data_dirs_default_when_empty() {
        let _env_lock = env_lock().lock().expect("env lock");
        let _xdg_data_dirs = EnvVarGuard::set("XDG_DATA_DIRS", "");
        let _xdg_data_home = EnvVarGuard::set("XDG_DATA_HOME", "/tmp/meridian-xdg-home");
        let dirs = desktop_app_dirs();

        assert!(dirs
            .iter()
            .any(|path| path == &PathBuf::from("/tmp/meridian-xdg-home/applications")));
        for base in XDG_DATA_DIRS_DEFAULT.split(':') {
            assert!(dirs
                .iter()
                .any(|path| path == &PathBuf::from(base).join("applications")));
        }
    }

    #[test]
    fn launcher_selection_moves_with_up_down_and_enter_uses_selection() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::AllApps,
            clicks: Vec::new(),
            apps: vec![
                DesktopApp::new("Alpha".to_string(), vec!["alpha".to_string()], false),
                DesktopApp::new("Beta".to_string(), vec!["beta".to_string()], false),
                DesktopApp::new("Gamma".to_string(), vec!["gamma".to_string()], false),
            ],
            pending_action_confirmation: None,
            view: LauncherView::AllApps,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let moved = state.handle_key(None, false, false, false, false, true);
        assert!(matches!(moved, LauncherInputResult::Redraw));
        assert_eq!(state.selected_index, 1);

        let moved = state.handle_key(None, false, false, false, false, true);
        assert!(matches!(moved, LauncherInputResult::Redraw));
        assert_eq!(state.selected_index, 2);

        let moved = state.handle_key(None, false, false, false, true, false);
        assert!(matches!(moved, LauncherInputResult::Redraw));
        assert_eq!(state.selected_index, 1);

        let launch = state.handle_key(None, false, true, false, false, false);
        assert!(matches!(launch, LauncherInputResult::Launch(1)));
    }

    #[test]
    fn computed_layout_footer_is_below_results_area() {
        let layout = compute_launcher_layout(720, 520, 1);
        let results_bottom = layout.results.y + layout.results.h;
        assert!(results_bottom <= layout.footer.y);
    }

    #[test]
    fn computed_layout_footer_sections_fit_inside_footer_and_do_not_overlap() {
        let layout = compute_launcher_layout(720, 520, 1);
        assert!(layout.footer_left.x >= layout.footer.x);
        assert!(layout.footer_right.x >= layout.footer.x);
        assert!(layout.footer_left.x + layout.footer_left.w <= layout.footer.x + layout.footer.w);
        assert!(layout.footer_right.x + layout.footer_right.w <= layout.footer.x + layout.footer.w);
        assert!(layout.footer_left.y >= layout.footer.y);
        assert!(layout.footer_right.y >= layout.footer.y);
        assert!(layout.footer_left.y + layout.footer_left.h <= layout.footer.y + layout.footer.h);
        assert!(layout.footer_right.y + layout.footer_right.h <= layout.footer.y + layout.footer.h);
        assert!(layout.footer_left.x + layout.footer_left.w <= layout.footer_right.x);
    }

    #[test]
    fn computed_layout_results_end_before_footer_content_starts() {
        let layout = compute_launcher_layout(720, 520, 1);
        let results_bottom = layout.results.y + layout.results.h;
        assert!(results_bottom <= layout.footer_right.y);
    }

    #[test]
    fn computed_layout_footer_height_is_compact_for_single_action() {
        let layout = compute_launcher_layout(720, 520, 1);
        assert_eq!(
            layout.footer.h,
            FOOTER_ACTION_BUTTON_H + FOOTER_BAR_V_PADDING * 2
        );
    }

    #[test]
    fn system_category_exposes_exit_meridian_action() {
        let state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::System,
            clicks: Vec::new(),
            apps: vec![app_with_categories("Settings", "settings", &["settings"])],
            pending_action_confirmation: None,
            view: LauncherView::AllApps,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        assert_eq!(state.visible_actions(), vec![LauncherAction::ExitMeridian]);
        assert_eq!(state.filtered_visible_count(), 2);
    }

    #[test]
    fn exit_action_is_visible_across_categories_and_query() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::AllApps,
            clicks: Vec::new(),
            apps: vec![DesktopApp::new(
                "Alpha".to_string(),
                vec!["alpha".to_string()],
                false,
            )],
            pending_action_confirmation: None,
            view: LauncherView::AllApps,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };
        assert_eq!(state.visible_actions(), vec![LauncherAction::ExitMeridian]);

        state.sidebar_category = SidebarCategory::System;
        assert_eq!(state.visible_actions(), vec![LauncherAction::ExitMeridian]);

        state.query = "alpha".to_string();
        assert_eq!(state.visible_actions(), vec![LauncherAction::ExitMeridian]);
    }

    #[test]
    fn exit_action_is_not_added_to_app_results() {
        let state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::System,
            clicks: Vec::new(),
            apps: vec![app_with_categories("Settings", "settings", &["settings"])],
            pending_action_confirmation: None,
            view: LauncherView::AllApps,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };
        let app_results = state.filtered_apps();
        assert_eq!(app_results.len(), 1);
        assert_eq!(app_results[0].name, "Settings");
    }

    #[test]
    fn enter_on_exit_meridian_action_returns_action_result() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 1,
            sidebar_category: SidebarCategory::System,
            clicks: Vec::new(),
            apps: vec![app_with_categories("Settings", "settings", &["settings"])],
            pending_action_confirmation: None,
            view: LauncherView::AllApps,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let result = state.handle_key(None, false, true, false, false, false);
        assert!(matches!(
            result,
            LauncherInputResult::Action(LauncherAction::ExitMeridian)
        ));
    }

    #[test]
    fn enter_on_app_row_remains_launch_even_when_system_action_is_present() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::System,
            clicks: Vec::new(),
            apps: vec![app_with_categories("Settings", "settings", &["settings"])],
            pending_action_confirmation: None,
            view: LauncherView::AllApps,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let result = state.handle_key(None, false, true, false, false, false);
        assert!(matches!(result, LauncherInputResult::Launch(0)));
    }

    #[test]
    fn global_footer_reserves_visible_slot_when_app_results_hit_cap() {
        let mut apps = Vec::new();
        for idx in 0..(MAX_RESULTS + 4) {
            apps.push(DesktopApp::new(
                format!("App {}", idx),
                vec![format!("app-{}", idx)],
                false,
            ));
        }

        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::AllApps,
            clicks: Vec::new(),
            apps,
            pending_action_confirmation: None,
            view: LauncherView::AllApps,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let visible = state.visible_apps();
        assert_eq!(visible.apps.len(), MAX_RESULTS - 1);
        assert_eq!(state.filtered_visible_count(), MAX_RESULTS);

        state.selected_index = MAX_RESULTS - 1;
        let result = state.handle_key(None, false, true, false, false, false);
        assert!(matches!(
            result,
            LauncherInputResult::Action(LauncherAction::ExitMeridian)
        ));
    }

    #[test]
    fn exit_action_requires_confirmation_before_quit() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::System,
            clicks: Vec::new(),
            apps: vec![app_with_categories("Settings", "settings", &["settings"])],
            pending_action_confirmation: None,
            view: LauncherView::AllApps,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let first = state.activate_action(LauncherAction::ExitMeridian);
        assert_eq!(first, LauncherActionActivationResult::Armed);
        assert_eq!(
            state.pending_action_confirmation,
            Some(LauncherAction::ExitMeridian)
        );

        let second = state.activate_action(LauncherAction::ExitMeridian);
        assert_eq!(second, LauncherActionActivationResult::Confirmed);
    }

    #[test]
    fn changing_category_or_query_cancels_exit_confirmation() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::System,
            clicks: Vec::new(),
            apps: vec![app_with_categories("Settings", "settings", &["settings"])],
            pending_action_confirmation: None,
            view: LauncherView::AllApps,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        assert_eq!(
            state.activate_action(LauncherAction::ExitMeridian),
            LauncherActionActivationResult::Armed
        );
        assert_eq!(
            state.pending_action_confirmation,
            Some(LauncherAction::ExitMeridian)
        );

        assert!(state.set_sidebar_category_from_click(SidebarCategory::AllApps.to_click_id()));
        assert_eq!(state.pending_action_confirmation, None);

        state.sidebar_category = SidebarCategory::System;
        state.query.clear();
        assert_eq!(
            state.activate_action(LauncherAction::ExitMeridian),
            LauncherActionActivationResult::Armed
        );
        assert!(matches!(
            state.handle_key(Some('a'), false, false, false, false, false),
            LauncherInputResult::Redraw
        ));
        assert_eq!(state.pending_action_confirmation, None);
    }

    #[test]
    fn launcher_query_edit_resets_selection() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 2,
            sidebar_category: SidebarCategory::Favorites,
            clicks: Vec::new(),
            apps: vec![DesktopApp::new(
                "Terminal".to_string(),
                vec!["foot".to_string()],
                false,
            )],
            pending_action_confirmation: None,
            view: LauncherView::AllApps,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let redraw = state.handle_key(Some('t'), false, false, false, false, false);
        assert!(matches!(redraw, LauncherInputResult::Redraw));
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn hover_sets_selected_index() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::AllApps,
            clicks: vec![ClickZone {
                rect: Rect {
                    x: 0,
                    y: 0,
                    w: 100,
                    h: 20,
                },
                action: ClickAction::LaunchApp(2),
            }],
            apps: vec![
                DesktopApp::new("A".to_string(), vec!["a".to_string()], false),
                DesktopApp::new("B".to_string(), vec!["b".to_string()], false),
                DesktopApp::new("C".to_string(), vec!["c".to_string()], false),
            ],
            pending_action_confirmation: None,
            view: LauncherView::AllApps,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let changed = state.update_hover_selection(10.0, 10.0);
        assert!(changed);
        assert_eq!(state.selected_index, 2);
    }

    #[test]
    fn update_app_hover_sets_and_clears_hover_tile_for_tile_start_view() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::Favorites,
            clicks: vec![ClickZone {
                rect: Rect {
                    x: 0,
                    y: 0,
                    w: 100,
                    h: 100,
                },
                action: ClickAction::LaunchApp(0),
            }],
            apps: vec![pinned_test_app("Firefox")],
            app_sections: vec![AppSection {
                letter: 'F',
                tiles: vec![AppTile {
                    app_index: 0,
                    size: TileSize::Wide,
                    col: 0,
                    row: 0,
                }],
                rows: 2,
            }],
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
        };

        assert!(state.update_app_hover(10.0, 10.0));
        assert_eq!(state.hover_app_index, Some(0));
        assert!(state.update_app_hover(200.0, 200.0));
        assert_eq!(state.hover_app_index, None);
    }

    #[test]
    fn hover_out_of_range_is_ignored() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 1,
            sidebar_category: SidebarCategory::Favorites,
            clicks: vec![ClickZone {
                rect: Rect {
                    x: 0,
                    y: 0,
                    w: 10,
                    h: 10,
                },
                action: ClickAction::LaunchApp(0),
            }],
            apps: vec![
                DesktopApp::new("A".to_string(), vec!["a".to_string()], false),
                DesktopApp::new("B".to_string(), vec!["b".to_string()], false),
            ],
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let changed = state.update_hover_selection(30.0, 30.0);
        assert!(!changed);
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn hover_sets_selected_index_for_launcher_action() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::System,
            clicks: vec![ClickZone {
                rect: Rect {
                    x: 0,
                    y: 0,
                    w: 100,
                    h: 20,
                },
                action: ClickAction::LauncherAction {
                    action: LauncherAction::ExitMeridian,
                    index: 1,
                },
            }],
            apps: vec![app_with_categories("Settings", "settings", &["settings"])],
            pending_action_confirmation: None,
            view: LauncherView::AllApps,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let changed = state.update_hover_selection(10.0, 10.0);
        assert!(changed);
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn keyboard_navigation_continues_from_hover_selection() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::AllApps,
            clicks: vec![ClickZone {
                rect: Rect {
                    x: 0,
                    y: 0,
                    w: 50,
                    h: 20,
                },
                action: ClickAction::LaunchApp(1),
            }],
            apps: vec![
                DesktopApp::new("A".to_string(), vec!["a".to_string()], false),
                DesktopApp::new("B".to_string(), vec!["b".to_string()], false),
                DesktopApp::new("C".to_string(), vec!["c".to_string()], false),
            ],
            pending_action_confirmation: None,
            view: LauncherView::AllApps,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        assert!(state.update_hover_selection(5.0, 5.0));
        assert_eq!(state.selected_index, 1);

        let moved = state.handle_key(None, false, false, false, false, true);
        assert!(matches!(moved, LauncherInputResult::Redraw));
        assert_eq!(state.selected_index, 2);
    }

    #[test]
    fn enter_with_no_visible_results_is_ignored() {
        let mut state = LauncherState {
            open: true,
            query: "zzzz-no-hit".to_string(),
            selected_index: 5,
            sidebar_category: SidebarCategory::Favorites,
            clicks: Vec::new(),
            apps: vec![DesktopApp::new(
                "Terminal".to_string(),
                vec!["foot".to_string()],
                false,
            )],
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let result = state.handle_key(None, false, true, false, false, false);
        assert!(matches!(
            result,
            LauncherInputResult::Action(LauncherAction::ExitMeridian)
        ));
        assert_eq!(state.selected_index, 5);
    }

    #[test]
    fn enter_uses_clamped_selected_index_for_visible_results() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 99,
            sidebar_category: SidebarCategory::AllApps,
            clicks: Vec::new(),
            apps: vec![
                DesktopApp::new("Alpha".to_string(), vec!["alpha".to_string()], false),
                DesktopApp::new("Beta".to_string(), vec!["beta".to_string()], false),
            ],
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let result = state.handle_key(None, false, true, false, false, false);
        assert!(matches!(
            result,
            LauncherInputResult::Action(LauncherAction::ExitMeridian)
        ));
    }

    #[test]
    fn pinned_section_shows_only_existing_apps_without_duplicates() {
        let state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::Favorites,
            clicks: Vec::new(),
            apps: vec![
                DesktopApp::new("Alpha".to_string(), vec!["alpha".to_string()], false),
                DesktopApp::new("Firefox".to_string(), vec!["firefox".to_string()], false),
                DesktopApp::new("Terminal".to_string(), vec!["foot".to_string()], true),
            ],
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let visible = state.visible_apps();
        assert_eq!(visible.pinned_count, 2);
        assert_eq!(visible.total_results, 2);
        assert_eq!(visible.apps.len(), 2);
        assert_eq!(visible.apps[0].name, "Firefox");
        assert_eq!(visible.apps[1].name, "Terminal");
    }

    #[test]
    fn search_query_bypasses_pinned_section() {
        let mut state = LauncherState {
            open: true,
            query: "alp".to_string(),
            selected_index: 0,
            sidebar_category: SidebarCategory::Favorites,
            clicks: Vec::new(),
            apps: vec![
                DesktopApp::new("Alpha".to_string(), vec!["alpha".to_string()], false),
                DesktopApp::new("Firefox".to_string(), vec!["firefox".to_string()], false),
                DesktopApp::new("Terminal".to_string(), vec!["foot".to_string()], true),
            ],
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let visible = state.visible_apps();
        assert_eq!(visible.pinned_count, 0);
        assert_eq!(visible.total_results, 1);
        assert_eq!(visible.apps.len(), 1);
        assert_eq!(visible.apps[0].name, "Alpha");

        state.query.clear();
        let visible = state.visible_apps();
        assert!(visible.pinned_count > 0);
    }

    #[test]
    fn enter_launch_maps_to_visible_index_with_pinned_section() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::Favorites,
            clicks: Vec::new(),
            apps: vec![
                DesktopApp::new("Alpha".to_string(), vec!["alpha".to_string()], false),
                DesktopApp::new("Firefox".to_string(), vec!["firefox".to_string()], false),
                DesktopApp::new("Terminal".to_string(), vec!["foot".to_string()], true),
            ],
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let visible = state.visible_apps();
        assert_eq!(visible.apps[0].name, "Firefox");
        assert_eq!(visible.apps[1].name, "Terminal");

        state.selected_index = 1;
        let result = state.handle_key(None, false, true, false, false, false);
        assert!(matches!(result, LauncherInputResult::Launch(1)));
    }

    #[test]
    fn app_initial_uses_first_uppercase_char() {
        assert_eq!(app_initial("firefox"), "F");
    }

    #[test]
    fn app_initial_skips_leading_whitespace() {
        assert_eq!(app_initial("   terminal"), "T");
    }

    #[test]
    fn app_initial_empty_falls_back_to_question_mark() {
        assert_eq!(app_initial(""), "?");
    }

    #[test]
    fn search_ranking_prefers_name_prefix_over_name_substring() {
        let state = LauncherState {
            open: true,
            query: "alp".to_string(),
            selected_index: 0,
            sidebar_category: SidebarCategory::Favorites,
            clicks: Vec::new(),
            apps: vec![
                DesktopApp::new("Zeta Alpha".to_string(), vec!["za".to_string()], false),
                DesktopApp::new(
                    "Alpha Tools".to_string(),
                    vec!["alpha-tools".to_string()],
                    false,
                ),
            ],
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let visible = state.visible_apps();
        assert_eq!(visible.apps.len(), 2);
        assert_eq!(visible.apps[0].name, "Alpha Tools");
        assert_eq!(visible.apps[1].name, "Zeta Alpha");
    }

    #[test]
    fn search_ranking_prefers_name_match_over_exec_only_match() {
        let state = LauncherState {
            open: true,
            query: "browser".to_string(),
            selected_index: 0,
            sidebar_category: SidebarCategory::Favorites,
            clicks: Vec::new(),
            apps: vec![
                DesktopApp::new("Browser Hub".to_string(), vec!["hub".to_string()], false),
                DesktopApp::new(
                    "Notes".to_string(),
                    vec!["browser-helper".to_string()],
                    false,
                ),
            ],
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let visible = state.visible_apps();
        assert_eq!(visible.apps.len(), 2);
        assert_eq!(visible.apps[0].name, "Browser Hub");
        assert_eq!(visible.apps[1].name, "Notes");
    }

    #[test]
    fn sidebar_category_token_mapping_matches_expected_internet_tokens() {
        assert_eq!(
            SidebarCategory::Internet.category_tokens(),
            &[
                "network",
                "webbrowser",
                "email",
                "chat",
                "instantmessaging",
                "ircclient",
                "filetransfer",
                "remoteaccess",
            ]
        );
    }

    #[test]
    fn selecting_internet_category_filters_apps_by_parsed_categories() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::Favorites,
            clicks: Vec::new(),
            apps: vec![
                app_with_categories("Browser", "browser", &["webbrowser"]),
                app_with_categories("Mail", "mail", &["email"]),
                app_with_categories("Editor", "editor", &["texteditor"]),
            ],
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        assert!(state.set_sidebar_category_from_click(SidebarCategory::Internet.to_click_id()));
        let visible = state.visible_apps();
        assert_eq!(visible.total_results, 2);
        assert_eq!(visible.pinned_count, 0);
        assert_eq!(visible.apps.len(), 2);
        assert_eq!(visible.apps[0].name, "Browser");
        assert_eq!(visible.apps[1].name, "Mail");
    }

    #[test]
    fn non_empty_query_ignores_sidebar_category_filter() {
        let mut state = LauncherState {
            open: true,
            query: "notes".to_string(),
            selected_index: 0,
            sidebar_category: SidebarCategory::Favorites,
            clicks: Vec::new(),
            apps: vec![
                app_with_categories("Browser", "browser", &["webbrowser"]),
                app_with_categories("Notes", "notes", &["texteditor"]),
            ],
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        assert!(state.set_sidebar_category_from_click(SidebarCategory::Internet.to_click_id()));
        let visible = state.visible_apps();
        assert_eq!(visible.total_results, 1);
        assert_eq!(visible.apps.len(), 1);
        assert_eq!(visible.apps[0].name, "Notes");
    }

    #[test]
    fn all_apps_category_shows_all_apps() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::Favorites,
            clicks: Vec::new(),
            apps: vec![
                app_with_categories("Alpha", "alpha", &["development"]),
                app_with_categories("Browser", "browser", &["webbrowser"]),
                app_with_categories("Settings", "settings", &["settings"]),
            ],
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        assert!(state.set_sidebar_category_from_click(SidebarCategory::AllApps.to_click_id()));
        let visible = state.visible_apps();
        assert_eq!(visible.total_results, 3);
        assert_eq!(visible.pinned_count, 0);
        assert_eq!(visible.apps.len(), 3);
    }

    #[test]
    fn selecting_same_sidebar_category_reports_no_change() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 2,
            sidebar_category: SidebarCategory::Favorites,
            clicks: Vec::new(),
            apps: vec![DesktopApp::new(
                "Terminal".to_string(),
                vec!["foot".to_string()],
                true,
            )],
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        assert!(!state.set_sidebar_category_from_click(SidebarCategory::Favorites.to_click_id()));
        assert_eq!(state.sidebar_category, SidebarCategory::Favorites);
        assert_eq!(state.selected_index, 2);
    }

    #[test]
    fn empty_query_favorites_shows_only_pinned_apps() {
        let state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::Favorites,
            clicks: Vec::new(),
            apps: vec![
                DesktopApp::new("Alpha".to_string(), vec!["alpha".to_string()], false),
                DesktopApp::new("Firefox".to_string(), vec!["firefox".to_string()], false),
                DesktopApp::new("Terminal".to_string(), vec!["foot".to_string()], true),
            ],
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let visible = state.visible_apps();
        assert_eq!(visible.pinned_count, 2);
        assert_eq!(visible.total_results, 2);
        assert_eq!(visible.apps.len(), 2);
        assert_eq!(visible.apps[0].name, "Firefox");
        assert_eq!(visible.apps[1].name, "Terminal");
    }

    #[test]
    fn empty_query_favorites_falls_back_to_all_apps_when_no_pinned_match_exists() {
        let state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::Favorites,
            clicks: Vec::new(),
            apps: vec![
                DesktopApp::new("Alpha".to_string(), vec!["alpha".to_string()], false),
                DesktopApp::new("Browser".to_string(), vec!["browser".to_string()], false),
            ],
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        let visible = state.visible_apps();
        assert_eq!(visible.pinned_count, 0);
        assert_eq!(visible.total_results, 2);
        assert_eq!(visible.apps.len(), 2);
        assert_eq!(visible.apps[0].name, "Alpha");
        assert_eq!(visible.apps[1].name, "Browser");
    }

    #[test]
    fn favorites_category_shows_only_pinned_apps() {
        let mut state = LauncherState {
            open: true,
            query: String::new(),
            selected_index: 0,
            sidebar_category: SidebarCategory::AllApps,
            clicks: Vec::new(),
            apps: vec![
                DesktopApp::new("Alpha".to_string(), vec!["alpha".to_string()], false),
                DesktopApp::new("Firefox".to_string(), vec!["firefox".to_string()], false),
                DesktopApp::new("Terminal".to_string(), vec!["foot".to_string()], true),
            ],
            pending_action_confirmation: None,
            view: LauncherView::TileStart,
            app_sections: Vec::new(),
            hover_app_index: None,
            tile_scroll_y: 0,
            tile_content_h_cache: 0,
            tile_viewport_h_cache: 0,
        };

        assert!(state.set_sidebar_category_from_click(SidebarCategory::Favorites.to_click_id()));
        let visible = state.visible_apps();
        assert_eq!(visible.pinned_count, 2);
        assert_eq!(visible.total_results, 2);
        assert_eq!(visible.apps.len(), 2);
        assert_eq!(visible.apps[0].name, "Firefox");
        assert_eq!(visible.apps[1].name, "Terminal");
    }

    #[test]
    fn opening_launcher_rescans_and_picks_up_new_desktop_file() {
        let _env_lock = env_lock().lock().expect("env lock");
        let xdg_home = unique_test_dir("toggle-rescan-add-home");
        let xdg_dirs = unique_test_dir("toggle-rescan-add-dirs");
        let applications_dir = xdg_home.join("applications");
        fs::create_dir_all(&applications_dir).expect("create applications dir");

        write_desktop_entry(&applications_dir, "alpha.desktop", "Alpha", "alpha");

        let _xdg_data_home =
            EnvVarGuard::set("XDG_DATA_HOME", xdg_home.to_str().expect("home utf8"));
        let _xdg_data_dirs =
            EnvVarGuard::set("XDG_DATA_DIRS", xdg_dirs.to_str().expect("dirs utf8"));

        let mut state = LauncherState::new();
        assert!(state.apps.iter().any(|app| app.name == "Alpha"));
        assert!(!state.apps.iter().any(|app| app.name == "Beta"));

        state.toggle();
        state.toggle();
        write_desktop_entry(&applications_dir, "beta.desktop", "Beta", "beta");
        state.toggle();

        assert!(state.apps.iter().any(|app| app.name == "Alpha"));
        assert!(state.apps.iter().any(|app| app.name == "Beta"));

        let _ = fs::remove_dir_all(xdg_home);
        let _ = fs::remove_dir_all(xdg_dirs);
    }

    #[test]
    fn opening_launcher_rescans_and_drops_removed_desktop_file() {
        let _env_lock = env_lock().lock().expect("env lock");
        let xdg_home = unique_test_dir("toggle-rescan-remove-home");
        let xdg_dirs = unique_test_dir("toggle-rescan-remove-dirs");
        let applications_dir = xdg_home.join("applications");
        fs::create_dir_all(&applications_dir).expect("create applications dir");

        write_desktop_entry(&applications_dir, "alpha.desktop", "Alpha", "alpha");
        write_desktop_entry(&applications_dir, "beta.desktop", "Beta", "beta");

        let _xdg_data_home =
            EnvVarGuard::set("XDG_DATA_HOME", xdg_home.to_str().expect("home utf8"));
        let _xdg_data_dirs =
            EnvVarGuard::set("XDG_DATA_DIRS", xdg_dirs.to_str().expect("dirs utf8"));

        let mut state = LauncherState::new();
        assert!(state.apps.iter().any(|app| app.name == "Beta"));

        state.toggle();
        state.toggle();
        fs::remove_file(applications_dir.join("beta.desktop")).expect("remove beta desktop file");
        state.toggle();

        assert!(state.apps.iter().any(|app| app.name == "Alpha"));
        assert!(!state.apps.iter().any(|app| app.name == "Beta"));

        let _ = fs::remove_dir_all(xdg_home);
        let _ = fs::remove_dir_all(xdg_dirs);
    }

    #[test]
    fn query_and_category_changes_do_not_rescan_desktop_entries() {
        let _env_lock = env_lock().lock().expect("env lock");
        let xdg_home = unique_test_dir("toggle-rescan-query-category-home");
        let xdg_dirs = unique_test_dir("toggle-rescan-query-category-dirs");
        let applications_dir = xdg_home.join("applications");
        fs::create_dir_all(&applications_dir).expect("create applications dir");

        write_desktop_entry(&applications_dir, "alpha.desktop", "Alpha", "alpha");

        let _xdg_data_home =
            EnvVarGuard::set("XDG_DATA_HOME", xdg_home.to_str().expect("home utf8"));
        let _xdg_data_dirs =
            EnvVarGuard::set("XDG_DATA_DIRS", xdg_dirs.to_str().expect("dirs utf8"));

        let mut state = LauncherState::new();
        state.toggle();
        assert!(state.apps.iter().any(|app| app.name == "Alpha"));
        assert!(!state.apps.iter().any(|app| app.name == "Beta"));

        write_desktop_entry(&applications_dir, "beta.desktop", "Beta", "beta");

        state.handle_key(Some('a'), false, false, false, false, false);
        assert_eq!(state.query, "a");
        assert!(!state.apps.iter().any(|app| app.name == "Beta"));

        assert!(state.set_sidebar_category_from_click(SidebarCategory::AllApps.to_click_id()));
        assert!(!state.apps.iter().any(|app| app.name == "Beta"));

        state.toggle();
        state.toggle();
        assert!(state.apps.iter().any(|app| app.name == "Beta"));

        let _ = fs::remove_dir_all(xdg_home);
        let _ = fs::remove_dir_all(xdg_dirs);
    }
}
