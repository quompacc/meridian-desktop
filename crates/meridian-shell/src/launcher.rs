use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use meridian_config::ThemeConfig;
use meridian_ipc::ShellCommand;
use tracing::{info, warn};

use crate::{ClickAction, ClickZone, Painter, Rect, TextRenderer};

const APP_ROW_H: i32 = 38;
const SEARCH_H: i32 = 42;
const PAD: i32 = 16;
const INNER_PAD: i32 = 14;

#[derive(Debug, Clone)]
pub struct DesktopApp {
    pub name: String,
    pub exec: String,
    pub terminal: bool,
    pub path: PathBuf,
}

impl DesktopApp {
    pub fn load_system() -> Vec<Self> {
        let mut apps = Vec::new();
        let dir = Path::new("/usr/share/applications");
        let Ok(entries) = fs::read_dir(dir) else {
            return apps;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("desktop") {
                continue;
            }
            if let Some(app) = Self::from_file(&path) {
                apps.push(app);
            }
        }

        apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        apps
    }

    fn from_file(path: &Path) -> Option<Self> {
        let raw = fs::read_to_string(path).ok()?;
        let mut in_entry = false;
        let mut name = None;
        let mut exec = None;
        let mut terminal = false;
        let mut hidden = false;
        let mut no_display = false;

        for line in raw.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                in_entry = line == "[Desktop Entry]";
                continue;
            }
            if !in_entry || line.starts_with('#') {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            match key {
                "Name" => {
                    name.get_or_insert_with(|| value.to_string());
                }
                "Exec" => {
                    exec.get_or_insert_with(|| clean_exec(value));
                }
                "Terminal" => terminal = value.eq_ignore_ascii_case("true"),
                "Hidden" => hidden = value.eq_ignore_ascii_case("true"),
                "NoDisplay" => no_display = value.eq_ignore_ascii_case("true"),
                "Type" if value != "Application" => return None,
                _ => {}
            };
        }

        if hidden || no_display {
            return None;
        }

        let name = name?;
        let exec = exec?;
        (!exec.is_empty()).then(|| Self {
            name,
            exec,
            terminal,
            path: path.to_path_buf(),
        })
    }
}

fn clean_exec(exec: &str) -> String {
    let mut cleaned = String::with_capacity(exec.len());
    let mut chars = exec.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let _ = chars.next();
        } else {
            cleaned.push(ch);
        }
    }
    cleaned.trim().to_string()
}

pub struct LauncherState {
    pub open: bool,
    pub query: String,
    pub clicks: Vec<ClickZone>,
    pub apps: Vec<DesktopApp>,
}

impl LauncherState {
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            clicks: Vec::new(),
            apps: DesktopApp::load_system(),
        }
    }

    pub fn toggle(&mut self) -> bool {
        self.open = !self.open;
        if !self.open {
            self.query.clear();
        }
        self.open
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
    }

    pub fn filtered_apps(&self) -> Vec<DesktopApp> {
        let query = self.query.to_lowercase();
        self.apps
            .iter()
            .filter(|app| {
                query.is_empty()
                    || app.name.to_lowercase().contains(&query)
                    || app.exec.to_lowercase().contains(&query)
            })
            .take(9)
            .cloned()
            .collect()
    }

    pub fn launch_app(&mut self, index: usize, ipc: &mut crate::IpcClient) {
        let apps = self.filtered_apps();
        if let Some(app) = apps.get(index).cloned() {
            info!("requesting launch: {} ({})", app.name, app.exec);
            let command = ShellCommand::LaunchApp {
                command: app.exec.clone(),
                terminal: app.terminal,
            };
            if !ipc.send(&command) {
                warn!("IPC unavailable, launching locally: {}", app.exec);
                match Command::new("sh").arg("-c").arg(&app.exec).spawn() {
                    Ok(child) => info!("local launch pid: {}", child.id()),
                    Err(err) => warn!("failed to launch {}: {}", app.name, err),
                }
            }
        }
    }

    pub fn handle_key(&mut self, ch: Option<char>, is_backspace: bool, is_enter: bool, is_escape: bool) -> LauncherInputResult {
        if !self.open {
            return LauncherInputResult::None;
        }

        if is_escape {
            self.close();
            return LauncherInputResult::Close;
        }

        if is_enter {
            return LauncherInputResult::Launch(0);
        }

        if is_backspace {
            self.query.pop();
            return LauncherInputResult::Redraw;
        }

        if let Some(ch) = ch {
            if !ch.is_control() {
                self.query.push(ch);
                return LauncherInputResult::Redraw;
            }
        }

        LauncherInputResult::None
    }
}

pub enum LauncherInputResult {
    None,
    Redraw,
    Close,
    Launch(usize),
}

pub fn draw_launcher(
    launcher_state: &mut LauncherState,
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    width: u32,
    height: u32,
) {
    if !launcher_state.open {
        return;
    }

    let colors = &theme.colors;
    painter.clear(colors.surface);
    painter.stroke_rect(
        Rect {
            x: 0,
            y: 0,
            w: width as i32,
            h: height as i32,
        },
        colors.border,
    );

    launcher_state.clicks.clear();

    let search_rect = Rect {
        x: PAD,
        y: PAD,
        w: width as i32 - PAD * 2,
        h: SEARCH_H,
    };
    painter.roundish_rect(search_rect, colors.background);
    let query_text = if launcher_state.query.is_empty() {
        "Search"
    } else {
        &launcher_state.query
    };
    painter.text_clipped(
        font,
        query_text,
        search_rect.x + INNER_PAD,
        search_rect.y + 27,
        search_rect.w - INNER_PAD * 2,
        colors.text,
    );

    let apps = launcher_state.filtered_apps();
    let mut y = PAD + SEARCH_H + 16;
    for (index, app) in apps.iter().enumerate() {
        let rect = Rect {
            x: PAD,
            y,
            w: width as i32 - PAD * 2,
            h: APP_ROW_H,
        };
        painter.roundish_rect(rect, colors.background);
        painter.text_clipped(
            font,
            &app.name,
            rect.x + INNER_PAD,
            rect.y + 25,
            rect.w - INNER_PAD * 2,
            colors.text,
        );
        launcher_state.clicks.push(ClickZone {
            rect,
            action: ClickAction::LaunchApp(index),
        });
        y += APP_ROW_H + 8;
    }
}
