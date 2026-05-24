use crate::ClickZone;

pub struct PanelState {
    pub clicks: Vec<ClickZone>,
}

#[derive(Debug, Clone)]
pub struct PinnedApp {
    pub label: String,
    pub program: String,
    pub args: Vec<String>,
    pub terminal: bool,
    pub icon_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PanelWindowEntry {
    pub id: String,
    pub title: String,
    pub focused: bool,
    pub minimized: bool,
    pub app_id: Option<String>,
}

impl PanelState {
    pub fn new() -> Self {
        Self { clicks: Vec::new() }
    }
}
