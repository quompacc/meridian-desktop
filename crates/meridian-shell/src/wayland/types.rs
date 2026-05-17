use crate::launcher::LauncherAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurfaceKind {
    None,
    Panel,
    Launcher,
    Calendar,
    WorkspacePopup,
}

#[derive(Debug, Clone)]
pub(crate) struct WindowInfo {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) workspace: u8,
    pub(crate) minimized: bool,
}

#[derive(Debug, Clone)]
pub enum ClickAction {
    SwitchWorkspace(u8),
    FocusWindow(String),
    LaunchPinnedApp(usize),
    SelectLauncherCategory(u8),
    LaunchApp(usize),
    LauncherAction {
        action: LauncherAction,
        index: usize,
    },
    ToggleLauncher,
    ToggleWorkspacePopup,
    Clock,
}

#[derive(Debug, Clone)]
pub struct ClickZone {
    pub rect: Rect,
    pub action: ClickAction,
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x as f64
            && y >= self.y as f64
            && x < (self.x + self.w) as f64
            && y < (self.y + self.h) as f64
    }
}
