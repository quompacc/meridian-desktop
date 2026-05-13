use std::{env, io, path::PathBuf};

use serde::{Deserialize, Serialize};

pub const SOCKET_NAME: &str = "meridian.sock";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSnapshotEntry {
    pub workspace: u8,
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub minimized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputWorkspaceState {
    pub output_id: u32,
    pub output_name: Option<String>,
    pub active_workspace: usize,
    pub primary: bool,
    pub focused: bool,
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

        if self.region.is_some() {
            return Err(ScreenshotBridgeError::Unsupported(
                "region capture is not implemented yet".to_string(),
            ));
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ShellCommand {
    SwitchWorkspace {
        workspace: u8,
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
mod tests {
    use super::{
        decode_command, decode_event, decode_screenshot_bridge_message, encode_command,
        encode_event, encode_screenshot_bridge_message, OutputWorkspaceSnapshot,
        OutputWorkspaceState, ScreenshotBridgeError, ScreenshotBridgeMessage,
        ScreenshotBridgeRequest, ScreenshotBridgeResponse, ScreenshotBridgeResult, ScreenshotKind,
        ScreenshotRegion, ScreenshotRequestMetadata, ScreenshotRequestOrigin, ShellCommand,
        ShellEvent, WindowSnapshotEntry,
    };

    #[test]
    fn window_snapshot_entry_contains_workspace_id_and_title() {
        let entry = WindowSnapshotEntry {
            workspace: 2,
            id: "win-1".to_string(),
            title: "Terminal".to_string(),
            minimized: false,
        };
        assert_eq!(entry.workspace, 2);
        assert_eq!(entry.id, "win-1");
        assert_eq!(entry.title, "Terminal");
        assert!(!entry.minimized);
    }

    #[test]
    fn window_snapshot_event_roundtrip_supports_multiple_workspaces() {
        let event = ShellEvent::WindowSnapshot {
            active_workspace: 3,
            windows: vec![
                WindowSnapshotEntry {
                    workspace: 1,
                    id: "a".to_string(),
                    title: "A".to_string(),
                    minimized: false,
                },
                WindowSnapshotEntry {
                    workspace: 3,
                    id: "b".to_string(),
                    title: "B".to_string(),
                    minimized: true,
                },
                WindowSnapshotEntry {
                    workspace: 9,
                    id: "c".to_string(),
                    title: "C".to_string(),
                    minimized: false,
                },
            ],
        };

        let bytes = encode_event(&event).expect("encode snapshot");
        let decoded = decode_event(std::str::from_utf8(&bytes).expect("utf8")).expect("decode");
        assert_eq!(decoded, event);
    }

    #[test]
    fn output_workspace_changed_event_roundtrip_supports_optional_name() {
        let event = ShellEvent::OutputWorkspaceChanged {
            output_id: 7,
            output_name: None,
            workspace: 2,
            focused: true,
        };

        let bytes = encode_event(&event).expect("encode");
        let decoded = decode_event(std::str::from_utf8(&bytes).expect("utf8")).expect("decode");
        assert_eq!(decoded, event);
    }

    #[test]
    fn output_workspace_snapshot_event_roundtrip_supports_two_outputs() {
        let snapshot = OutputWorkspaceSnapshot {
            focused_output_id: Some(42),
            outputs: vec![
                OutputWorkspaceState {
                    output_id: 42,
                    output_name: Some("eDP-1".to_string()),
                    active_workspace: 3,
                    primary: true,
                    focused: true,
                },
                OutputWorkspaceState {
                    output_id: 99,
                    output_name: Some("HDMI-A-1".to_string()),
                    active_workspace: 1,
                    primary: false,
                    focused: false,
                },
            ],
        };

        let event = ShellEvent::OutputWorkspaceSnapshot {
            focused_output_id: snapshot.focused_output_id,
            outputs: snapshot.outputs.clone(),
        };

        let bytes = encode_event(&event).expect("encode");
        let decoded = decode_event(std::str::from_utf8(&bytes).expect("utf8")).expect("decode");
        assert_eq!(decoded, event);
    }

    #[test]
    fn legacy_workspace_changed_roundtrip_remains_stable() {
        let event = ShellEvent::WorkspaceChanged { workspace: 4 };
        let bytes = encode_event(&event).expect("encode");
        let decoded = decode_event(std::str::from_utf8(&bytes).expect("utf8")).expect("decode");
        assert_eq!(decoded, event);
    }

    #[test]
    fn window_focus_cleared_event_roundtrip_is_supported() {
        let event = ShellEvent::WindowFocusCleared;
        let bytes = encode_event(&event).expect("encode");
        let decoded = decode_event(std::str::from_utf8(&bytes).expect("utf8")).expect("decode");
        assert_eq!(decoded, event);
    }

    #[test]
    fn output_workspace_snapshot_allows_missing_focused_output() {
        let event = ShellEvent::OutputWorkspaceSnapshot {
            focused_output_id: None,
            outputs: vec![OutputWorkspaceState {
                output_id: 5,
                output_name: None,
                active_workspace: 0,
                primary: true,
                focused: false,
            }],
        };
        let bytes = encode_event(&event).expect("encode");
        let decoded = decode_event(std::str::from_utf8(&bytes).expect("utf8")).expect("decode");
        assert_eq!(decoded, event);
    }

    #[test]
    fn window_snapshot_entry_missing_minimized_decodes_to_false() {
        let raw = r#"{"workspace":2,"id":"win-1","title":"Terminal"}"#;
        let decoded: WindowSnapshotEntry = serde_json::from_str(raw).expect("decode");
        assert!(!decoded.minimized);
    }

    #[test]
    fn launch_app_command_roundtrip_uses_argv() {
        let command = ShellCommand::LaunchApp {
            program: "alacritty".to_string(),
            args: vec![
                "--class".to_string(),
                "Meridian".to_string(),
                "-e".to_string(),
                "foot".to_string(),
            ],
            terminal: false,
        };

        let bytes = encode_command(&command).expect("encode");
        let decoded = decode_command(std::str::from_utf8(&bytes).expect("utf8")).expect("decode");
        assert_eq!(decoded, command);
    }

    #[test]
    fn launch_app_command_accepts_legacy_command_field() {
        let raw = r#"{"type":"launch-app","command":"foot","terminal":false}"#;
        let decoded = decode_command(raw).expect("decode");
        assert_eq!(
            decoded,
            ShellCommand::LaunchApp {
                program: "foot".to_string(),
                args: Vec::new(),
                terminal: false,
            }
        );
    }

    #[test]
    fn quit_command_roundtrip_is_supported() {
        let command = ShellCommand::Quit;
        let bytes = encode_command(&command).expect("encode");
        let decoded = decode_command(std::str::from_utf8(&bytes).expect("utf8")).expect("decode");
        assert_eq!(decoded, command);
    }

    #[test]
    fn screenshot_bridge_request_supports_full_output_mode() {
        let request = ScreenshotBridgeRequest {
            request_id: "req-1".to_string(),
            kind: ScreenshotKind::FullOutput,
            output: Some("eDP-1".to_string()),
            include_cursor: true,
            region: None,
            metadata: ScreenshotRequestMetadata {
                requester: None,
                origin: ScreenshotRequestOrigin::Unknown,
                request_marker: None,
                identity_trusted: false,
            },
        };

        assert_eq!(request.validate(), Ok(()));
    }

    #[test]
    fn screenshot_bridge_request_rejects_empty_request_id() {
        let request = ScreenshotBridgeRequest {
            request_id: "   ".to_string(),
            kind: ScreenshotKind::FullOutput,
            output: None,
            include_cursor: false,
            region: None,
            metadata: ScreenshotRequestMetadata::default(),
        };

        assert_eq!(
            request.validate(),
            Err(ScreenshotBridgeError::InvalidRequest(
                "request_id must not be empty".to_string()
            ))
        );
    }

    #[test]
    fn screenshot_bridge_request_rejects_region_until_supported() {
        let request = ScreenshotBridgeRequest {
            request_id: "req-2".to_string(),
            kind: ScreenshotKind::FullOutput,
            output: None,
            include_cursor: false,
            region: Some(ScreenshotRegion {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            }),
            metadata: ScreenshotRequestMetadata::default(),
        };

        assert_eq!(
            request.validate(),
            Err(ScreenshotBridgeError::Unsupported(
                "region capture is not implemented yet".to_string()
            ))
        );
    }

    #[test]
    fn screenshot_bridge_request_roundtrip_works() {
        let message = ScreenshotBridgeMessage::ScreenshotRequest {
            request: ScreenshotBridgeRequest {
                request_id: "portal-req-1".to_string(),
                kind: ScreenshotKind::FullOutput,
                output: Some("HDMI-A-1".to_string()),
                include_cursor: true,
                region: None,
                metadata: ScreenshotRequestMetadata {
                    requester: Some("org.example.App".to_string()),
                    origin: ScreenshotRequestOrigin::PortalDbus,
                    request_marker: Some(42),
                    identity_trusted: false,
                },
            },
        };
        let bytes = encode_screenshot_bridge_message(&message).expect("encode bridge request");
        let decoded = decode_screenshot_bridge_message(
            std::str::from_utf8(&bytes).expect("bridge request utf8"),
        )
        .expect("decode bridge request");
        assert_eq!(decoded, message);
    }

    #[test]
    fn screenshot_bridge_response_roundtrip_with_error_works() {
        let message = ScreenshotBridgeMessage::ScreenshotResponse {
            request_id: "portal-req-2".to_string(),
            result: ScreenshotBridgeResult::Error {
                error: ScreenshotBridgeError::PermissionDenied(
                    "permission denied by policy".to_string(),
                ),
            },
        };
        let bytes = encode_screenshot_bridge_message(&message).expect("encode bridge response");
        let decoded = decode_screenshot_bridge_message(
            std::str::from_utf8(&bytes).expect("bridge response utf8"),
        )
        .expect("decode bridge response");
        assert_eq!(decoded, message);
    }

    #[test]
    fn screenshot_bridge_response_roundtrip_with_success_works() {
        let message = ScreenshotBridgeMessage::ScreenshotResponse {
            request_id: "portal-req-3".to_string(),
            result: ScreenshotBridgeResult::Success {
                response: ScreenshotBridgeResponse {
                    request_id: "portal-req-3".to_string(),
                    file_descriptor_token: Some("token-1".to_string()),
                },
            },
        };
        let bytes =
            encode_screenshot_bridge_message(&message).expect("encode bridge success response");
        let decoded = decode_screenshot_bridge_message(
            std::str::from_utf8(&bytes).expect("bridge success utf8"),
        )
        .expect("decode bridge success response");
        assert_eq!(decoded, message);
    }

    #[test]
    fn screenshot_bridge_request_metadata_roundtrip_works() {
        let request = ScreenshotBridgeRequest {
            request_id: "req-meta-1".to_string(),
            kind: ScreenshotKind::FullOutput,
            output: Some("eDP-1".to_string()),
            include_cursor: false,
            region: None,
            metadata: ScreenshotRequestMetadata {
                requester: Some("org.example.Unknown".to_string()),
                origin: ScreenshotRequestOrigin::PortalDbus,
                request_marker: Some(777),
                identity_trusted: false,
            },
        };
        let message = ScreenshotBridgeMessage::ScreenshotRequest { request };
        let bytes = encode_screenshot_bridge_message(&message).expect("encode metadata request");
        let decoded = decode_screenshot_bridge_message(
            std::str::from_utf8(&bytes).expect("metadata request utf8"),
        )
        .expect("decode metadata request");
        assert_eq!(decoded, message);
    }
}
