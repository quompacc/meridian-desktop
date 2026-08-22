use std::{env, io, path::PathBuf};

use serde::{Deserialize, Serialize};

mod appearance;
pub use appearance::{AppearanceSnapshot, AppearanceTheme, AppearanceWallpaperMode};
mod settings;
pub use settings::{SettingsSnapshot, SystemSettingsSnapshot};

pub const SOCKET_NAME: &str = "meridian.sock";
pub const IPC_TOKEN_ENV: &str = "MERIDIAN_IPC_TOKEN";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSnapshotEntry {
    pub workspace: u8,
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub minimized: bool,
    #[serde(default)]
    pub app_id: Option<String>,
}

fn default_output_scale_millis() -> u32 {
    1000
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputModeState {
    pub width: i32,
    pub height: i32,
    #[serde(default)]
    pub refresh_millihz: Option<i32>,
    #[serde(default)]
    pub current: bool,
    #[serde(default)]
    pub preferred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputWorkspaceState {
    pub output_id: u32,
    pub output_name: Option<String>,
    pub active_workspace: usize,
    pub primary: bool,
    pub focused: bool,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub height: i32,
    #[serde(default = "default_output_scale_millis")]
    pub scale_millis: u32,
    #[serde(default)]
    pub transform: Option<String>,
    #[serde(default)]
    pub refresh_millihz: Option<i32>,
    #[serde(default)]
    pub modes: Vec<OutputModeState>,
}

impl Default for OutputWorkspaceState {
    fn default() -> Self {
        Self {
            output_id: 0,
            output_name: None,
            active_workspace: 1,
            primary: false,
            focused: false,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            scale_millis: default_output_scale_millis(),
            transform: None,
            refresh_millihz: None,
            modes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputWorkspaceSnapshot {
    pub focused_output_id: Option<u32>,
    pub outputs: Vec<OutputWorkspaceState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScreenshotKind {
    FullOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ScreenshotRequestOrigin {
    PortalDbus,
    Internal,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ScreenshotRequestMetadata {
    #[serde(default)]
    pub requester: Option<String>,
    #[serde(default)]
    pub origin: ScreenshotRequestOrigin,
    #[serde(default)]
    pub request_marker: Option<u64>,
    #[serde(default)]
    pub identity_trusted: bool,
    /// `interactive=true` (per the freedesktop Screenshot portal `options`)
    /// requests an in-shell area/region selection UI before capture.
    #[serde(default)]
    pub interactive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenshotRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenshotBridgeRequest {
    pub request_id: String,
    pub kind: ScreenshotKind,
    pub output: Option<String>,
    pub include_cursor: bool,
    pub region: Option<ScreenshotRegion>,
    #[serde(default)]
    pub metadata: ScreenshotRequestMetadata,
}

impl ScreenshotBridgeRequest {
    pub fn validate(&self) -> Result<(), ScreenshotBridgeError> {
        if self.request_id.trim().is_empty() {
            return Err(ScreenshotBridgeError::InvalidRequest(
                "request_id must not be empty".to_string(),
            ));
        }

        if self
            .output
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(ScreenshotBridgeError::InvalidRequest(
                "output identifier must not be empty".to_string(),
            ));
        }

        if let Some(region) = self.region {
            if region.width == 0 || region.height == 0 {
                return Err(ScreenshotBridgeError::InvalidRequest(
                    "region width and height must be nonzero".to_string(),
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenshotBridgeResponse {
    pub request_id: String,
    pub file_descriptor_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScreenshotBridgeError {
    Unsupported(String),
    PermissionDenied(String),
    CompositorUnavailable(String),
    InvalidRequest(String),
    Internal(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum ScreenshotBridgeResult {
    Success { response: ScreenshotBridgeResponse },
    Error { error: ScreenshotBridgeError },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ScreenshotBridgeMessage {
    ScreenshotRequest {
        request: ScreenshotBridgeRequest,
    },
    ScreenshotResponse {
        request_id: String,
        result: ScreenshotBridgeResult,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ShellEvent {
    // Legacy transition event. Kept for backward compatibility while output-aware
    // workspace events are introduced in parallel.
    WorkspaceChanged {
        workspace: u8,
    },
    WindowSnapshot {
        active_workspace: u8,
        windows: Vec<WindowSnapshotEntry>,
    },
    OutputWorkspaceChanged {
        output_id: u32,
        output_name: Option<String>,
        workspace: usize,
        focused: bool,
    },
    OutputWorkspaceSnapshot {
        focused_output_id: Option<u32>,
        outputs: Vec<OutputWorkspaceState>,
    },
    WindowOpened {
        id: String,
        title: String,
    },
    WindowClosed {
        id: String,
    },
    WindowFocused {
        id: String,
    },
    WindowFocusCleared,
    ConfigReloaded {
        success: bool,
    },
    ToggleLauncher,
    ToggleQuickSettings,
    OpenSystemSettings,
    SettingsRefresh,
    AppearanceRefresh,
    AppearanceThemeSet {
        theme: AppearanceTheme,
    },
    AppearanceWallpaperSet {
        path: String,
    },
    AppearanceWallpaperModeSet {
        mode: AppearanceWallpaperMode,
    },
    QuickSettingsNetworkRefresh,
    QuickSettingsNetworkConnect {
        ssid: String,
        password: Option<String>,
    },
    QuickSettingsNetworkDisconnect,
    AudioVolumeSet {
        percent: u8,
    },
    /// A multimedia volume key was pressed. The compositor intercepts the
    /// XF86Audio{Raise,Lower}Volume keysyms (so they never reach apps) and asks
    /// the shell, which owns the platform audio backend, to nudge the level by
    /// `delta` percent (positive = louder).
    AudioVolumeStep {
        delta: i8,
    },
    /// XF86AudioMute was pressed; the shell toggles the default sink mute.
    AudioMuteToggle,
    /// Quick Settings already provides visual feedback, so toggling mute from
    /// that surface must not also open the centre-screen volume OSD.
    QuickSettingsAudioMuteToggle,
    PowerProfileSet {
        profile: QuickSettingsPowerProfile,
    },
    DesktopContextMenu {
        x: i32,
        y: i32,
    },
    WindowThumbnail {
        id: String,
        path: String,
        width: u32,
        height: u32,
    },
    /// A screenshot request (via the portal bridge) needs the user's consent.
    /// The shell shows a modal; the user's answer comes back as
    /// `ShellCommand::ScreenshotConsentResponse` with the same `request_id`.
    ScreenshotConsentRequest {
        request_id: String,
        /// Best-effort requesting app identity for display ("" if unknown).
        app_id: String,
    },
    /// An interactive screenshot request (`interactive=true`) needs a region
    /// to be picked by the user. The shell shows a fullscreen drag-rectangle
    /// picker; the picked region comes back as
    /// `ShellCommand::ScreenshotRegionResponse` with the same `request_id`.
    ScreenshotRegionRequest {
        request_id: String,
        /// Best-effort requesting app identity for display ("" if unknown).
        app_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ShellCommand {
    Authenticate {
        role: String,
        token: String,
    },
    SwitchWorkspace {
        workspace: u8,
    },
    ToggleLauncher,
    ToggleQuickSettings,
    OpenSystemSettings,
    SettingsRefresh,
    AppearanceRefresh,
    AppearanceThemeSet {
        theme: AppearanceTheme,
    },
    AppearanceWallpaperSet {
        path: String,
    },
    AppearanceWallpaperModeSet {
        mode: AppearanceWallpaperMode,
    },
    QuickSettingsNetworkRefresh,
    QuickSettingsNetworkConnect {
        ssid: String,
        password: Option<String>,
    },
    QuickSettingsNetworkDisconnect,
    AudioVolumeSet {
        percent: u8,
    },
    AudioMuteToggle,
    PowerProfileSet {
        profile: QuickSettingsPowerProfile,
    },
    FocusWindow {
        id: String,
    },
    LaunchApp {
        #[serde(default, alias = "command")]
        program: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        terminal: bool,
    },
    ReloadConfig,
    Quit,
    CaptureWindowThumbnail {
        id: String,
        #[serde(default)]
        max_width: u32,
        #[serde(default)]
        max_height: u32,
    },
    /// The user's answer to a `ShellEvent::ScreenshotConsentRequest`.
    ScreenshotConsentResponse {
        request_id: String,
        allowed: bool,
    },
    /// The user's answer to a `ShellEvent::ScreenshotRegionRequest`. `None`
    /// means the user cancelled (Esc); `Some(region)` carries the selection.
    ScreenshotRegionResponse {
        request_id: String,
        region: Option<ScreenshotRegion>,
    },
}

impl ShellCommand {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Authenticate { .. } => "authenticate",
            Self::SwitchWorkspace { .. } => "switch-workspace",
            Self::ToggleLauncher => "toggle-launcher",
            Self::ToggleQuickSettings => "toggle-quick-settings",
            Self::OpenSystemSettings => "open-system-settings",
            Self::SettingsRefresh => "settings-refresh",
            Self::AppearanceRefresh => "appearance-refresh",
            Self::AppearanceThemeSet { .. } => "appearance-theme-set",
            Self::AppearanceWallpaperSet { .. } => "appearance-wallpaper-set",
            Self::AppearanceWallpaperModeSet { .. } => "appearance-wallpaper-mode-set",
            Self::QuickSettingsNetworkRefresh => "quick-settings-network-refresh",
            Self::QuickSettingsNetworkConnect { .. } => "quick-settings-network-connect",
            Self::QuickSettingsNetworkDisconnect => "quick-settings-network-disconnect",
            Self::AudioVolumeSet { .. } => "audio-volume-set",
            Self::AudioMuteToggle => "audio-mute-toggle",
            Self::PowerProfileSet { .. } => "power-profile-set",
            Self::FocusWindow { .. } => "focus-window",
            Self::LaunchApp { .. } => "launch-app",
            Self::ReloadConfig => "reload-config",
            Self::Quit => "quit",
            Self::CaptureWindowThumbnail { .. } => "capture-window-thumbnail",
            Self::ScreenshotConsentResponse { .. } => "screenshot-consent-response",
            Self::ScreenshotRegionResponse { .. } => "screenshot-region-response",
        }
    }
}

/// State sent from the shell owner to the unprivileged Quick Settings
/// document. Backend identities and privileged implementation details stay
/// out of this display contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickSettingsSnapshot {
    pub network: QuickSettingsNetwork,
    pub audio: QuickSettingsAudio,
    pub battery: QuickSettingsBattery,
    pub power_profile: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QuickSettingsPowerProfile {
    Eco,
    Standard,
    Performance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickSettingsNetwork {
    pub available: bool,
    pub connected: bool,
    pub kind: Option<String>,
    pub name: Option<String>,
    pub signal_percent: Option<u8>,
    pub wifi_networks: Vec<QuickSettingsWifiNetwork>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickSettingsWifiNetwork {
    pub ssid: String,
    pub signal_percent: u8,
    pub secured: bool,
    pub known: bool,
    pub in_use: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickSettingsAudio {
    pub available: bool,
    pub output_name: Option<String>,
    pub volume_percent: Option<u8>,
    pub muted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickSettingsBattery {
    pub present: bool,
    pub capacity: u8,
    pub charging: bool,
    pub on_ac: bool,
}

impl QuickSettingsSnapshot {
    pub fn validate(&self) -> Result<(), &'static str> {
        fn valid_label(value: &Option<String>, limit: usize) -> bool {
            value.as_ref().is_none_or(|value| {
                !value.is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
            })
        }

        if self.network.signal_percent.is_some_and(|value| value > 100)
            || self.network.wifi_networks.len() > 64
            || self
                .network
                .wifi_networks
                .iter()
                .any(|network| network.signal_percent > 100)
            || self.audio.volume_percent.is_some_and(|value| value > 100)
            || self.battery.capacity > 100
        {
            return Err("percentage outside 0..=100");
        }
        if !valid_label(&self.network.kind, 32)
            || !valid_label(&self.network.name, 256)
            || self
                .network
                .wifi_networks
                .iter()
                .any(|network| !valid_label(&Some(network.ssid.clone()), 128))
            || !valid_label(&self.audio.output_name, 256)
            || !valid_label(&self.power_profile, 64)
        {
            return Err("invalid display label");
        }
        Ok(())
    }
}

pub fn socket_path() -> PathBuf {
    if let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join(SOCKET_NAME);
    }

    // SAFETY: `geteuid` has no preconditions and returns the effective uid of this process.
    let uid = unsafe { libc::geteuid() };
    PathBuf::from(format!("/run/user/{uid}")).join(SOCKET_NAME)
}

pub fn encode_command(command: &ShellCommand) -> io::Result<Vec<u8>> {
    encode_json_line(command)
}

pub fn encode_event(event: &ShellEvent) -> io::Result<Vec<u8>> {
    encode_json_line(event)
}

pub fn decode_command(line: &str) -> serde_json::Result<ShellCommand> {
    serde_json::from_str(line.trim())
}

pub fn decode_event(line: &str) -> serde_json::Result<ShellEvent> {
    serde_json::from_str(line.trim())
}

pub fn encode_screenshot_bridge_message(message: &ScreenshotBridgeMessage) -> io::Result<Vec<u8>> {
    encode_json_line(message)
}

pub fn decode_screenshot_bridge_message(line: &str) -> serde_json::Result<ScreenshotBridgeMessage> {
    serde_json::from_str(line.trim())
}

fn encode_json_line<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
    let mut bytes =
        serde_json::to_vec(value).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
