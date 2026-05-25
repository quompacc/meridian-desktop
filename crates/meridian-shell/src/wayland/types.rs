#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurfaceKind {
    None,
    Panel,
    Launcher,
    Calendar,
    WorkspacePopup,
    NetworkPopup,
    ThumbnailPopup,
}

#[derive(Debug, Clone)]
pub(crate) struct WindowInfo {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) workspace: u8,
    pub(crate) minimized: bool,
    pub(crate) app_id: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ClickAction {
    SwitchWorkspace(u8),
    FocusWindow(String),
    LaunchPinnedApp(usize),
    ToggleLauncher,
    ToggleWorkspacePopup,
    ToggleNetworkPopup,
    ToggleAudioPopup,
    OpenSoundSettings,
    ActivateStatusNotifierItem(usize),
    Clock,
    TakeScreenshot,
    ToggleSettings,
}

impl ClickAction {
    pub(crate) fn test_name(&self) -> String {
        match self {
            ClickAction::SwitchWorkspace(workspace) => format!("switch-workspace-{workspace}"),
            ClickAction::FocusWindow(id) => format!("focus-window-{id}"),
            ClickAction::LaunchPinnedApp(idx) => format!("launch-pinned-app-{idx}"),
            ClickAction::ToggleLauncher => "toggle-launcher".to_string(),
            ClickAction::ToggleWorkspacePopup => "toggle-workspace-popup".to_string(),
            ClickAction::ToggleNetworkPopup => "toggle-network-popup".to_string(),
            ClickAction::ToggleAudioPopup => "toggle-audio-popup".to_string(),
            ClickAction::OpenSoundSettings => "open-sound-settings".to_string(),
            ClickAction::ActivateStatusNotifierItem(idx) => {
                format!("activate-status-notifier-item-{idx}")
            }
            ClickAction::Clock => "clock".to_string(),
            ClickAction::TakeScreenshot => "take-screenshot".to_string(),
            ClickAction::ToggleSettings => "toggle-settings".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClickZone {
    pub id: Option<String>,
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
