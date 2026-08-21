use super::{
    decode_command, decode_event, decode_screenshot_bridge_message, encode_command, encode_event,
    encode_screenshot_bridge_message, AppearanceSnapshot, AppearanceTheme, AppearanceWallpaperMode,
    OutputModeState, OutputWorkspaceSnapshot, OutputWorkspaceState, QuickSettingsAudio,
    QuickSettingsBattery, QuickSettingsNetwork, QuickSettingsPowerProfile, QuickSettingsSnapshot,
    QuickSettingsWifiNetwork, ScreenshotBridgeError, ScreenshotBridgeMessage,
    ScreenshotBridgeRequest, ScreenshotBridgeResponse, ScreenshotBridgeResult, ScreenshotKind,
    ScreenshotRegion, ScreenshotRequestMetadata, ScreenshotRequestOrigin, ShellCommand, ShellEvent,
    WindowSnapshotEntry,
};

#[test]
fn quick_settings_control_commands_roundtrip() {
    for command in [
        ShellCommand::AudioVolumeSet { percent: 72 },
        ShellCommand::AudioMuteToggle,
        ShellCommand::PowerProfileSet {
            profile: QuickSettingsPowerProfile::Eco,
        },
        ShellCommand::OpenSystemSettings,
        ShellCommand::QuickSettingsNetworkRefresh,
        ShellCommand::QuickSettingsNetworkConnect {
            ssid: "Meridian".to_string(),
            password: Some("secret phrase".to_string()),
        },
        ShellCommand::QuickSettingsNetworkDisconnect,
    ] {
        let bytes = encode_command(&command).expect("encode command");
        let decoded = decode_command(std::str::from_utf8(&bytes).expect("utf8")).expect("decode");
        assert_eq!(decoded, command);
    }
}

#[test]
fn appearance_control_commands_roundtrip() {
    for command in [
        ShellCommand::AppearanceRefresh,
        ShellCommand::AppearanceThemeSet {
            theme: AppearanceTheme::Light,
        },
        ShellCommand::AppearanceWallpaperSet {
            path: "/home/user/Pictures/meridian.png".to_string(),
        },
        ShellCommand::AppearanceWallpaperModeSet {
            mode: AppearanceWallpaperMode::Fit,
        },
    ] {
        let bytes = encode_command(&command).expect("encode command");
        let decoded = decode_command(std::str::from_utf8(&bytes).expect("utf8")).expect("decode");
        assert_eq!(decoded, command);
    }
}

#[test]
fn appearance_snapshot_rejects_untrusted_display_text() {
    let mut snapshot = AppearanceSnapshot {
        theme: AppearanceTheme::Dark,
        wallpaper_name: Some("Berge".to_string()),
        wallpaper_mode: AppearanceWallpaperMode::Fill,
    };
    assert_eq!(snapshot.validate(), Ok(()));
    snapshot.wallpaper_name = Some("bad\nname".to_string());
    assert!(snapshot.validate().is_err());
}

#[test]
fn quick_settings_snapshot_validates_display_boundary() {
    let mut snapshot = QuickSettingsSnapshot {
        network: QuickSettingsNetwork {
            available: true,
            connected: true,
            kind: Some("WLAN".to_string()),
            name: Some("Meridian Lab".to_string()),
            signal_percent: Some(82),
            wifi_networks: vec![QuickSettingsWifiNetwork {
                ssid: "Meridian Lab".to_string(),
                signal_percent: 82,
                secured: true,
                known: true,
                in_use: true,
            }],
        },
        audio: QuickSettingsAudio {
            available: true,
            output_name: Some("Audio".to_string()),
            volume_percent: Some(62),
            muted: false,
        },
        battery: QuickSettingsBattery {
            present: true,
            capacity: 84,
            charging: false,
            on_ac: false,
        },
        power_profile: Some("Standard".to_string()),
    };
    assert_eq!(snapshot.validate(), Ok(()));
    snapshot.audio.volume_percent = Some(101);
    assert!(snapshot.validate().is_err());
    snapshot.audio.volume_percent = Some(62);
    snapshot.network.name = Some("bad\nlabel".to_string());
    assert!(snapshot.validate().is_err());
}

#[test]
fn window_snapshot_entry_contains_workspace_id_and_title() {
    let entry = WindowSnapshotEntry {
        workspace: 2,
        id: "win-1".to_string(),
        title: "Terminal".to_string(),
        minimized: false,
        app_id: None,
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
                app_id: None,
            },
            WindowSnapshotEntry {
                workspace: 3,
                id: "b".to_string(),
                title: "B".to_string(),
                minimized: true,
                app_id: None,
            },
            WindowSnapshotEntry {
                workspace: 9,
                id: "c".to_string(),
                title: "C".to_string(),
                minimized: false,
                app_id: None,
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
                x: 0,
                y: 0,
                width: 2880,
                height: 1800,
                scale_millis: 1500,
                transform: Some("Normal".to_string()),
                refresh_millihz: Some(60_000),
                modes: vec![OutputModeState {
                    width: 2880,
                    height: 1800,
                    refresh_millihz: Some(60_000),
                    current: true,
                    preferred: true,
                }],
            },
            OutputWorkspaceState {
                output_id: 99,
                output_name: Some("HDMI-A-1".to_string()),
                active_workspace: 1,
                primary: false,
                focused: false,
                x: 2880,
                y: 0,
                width: 1920,
                height: 1080,
                scale_millis: 1000,
                transform: Some("Normal".to_string()),
                refresh_millihz: Some(144_000),
                modes: vec![OutputModeState {
                    width: 1920,
                    height: 1080,
                    refresh_millihz: Some(144_000),
                    current: true,
                    preferred: false,
                }],
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
fn desktop_context_menu_event_roundtrip_is_supported() {
    let event = ShellEvent::DesktopContextMenu { x: 120, y: 340 };
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
            ..Default::default()
        }],
    };
    let bytes = encode_event(&event).expect("encode");
    let decoded = decode_event(std::str::from_utf8(&bytes).expect("utf8")).expect("decode");
    assert_eq!(decoded, event);
}

#[test]
fn output_workspace_snapshot_decodes_legacy_output_without_display_details() {
    let decoded = decode_event(
        r#"{"type":"output-workspace-snapshot","focused_output_id":1,"outputs":[{"output_id":1,"output_name":"eDP-1","active_workspace":2,"primary":true,"focused":true}]}"#,
    )
    .expect("decode legacy output snapshot");

    let ShellEvent::OutputWorkspaceSnapshot { outputs, .. } = decoded else {
        panic!("expected output workspace snapshot");
    };
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].width, 0);
    assert_eq!(outputs[0].height, 0);
    assert_eq!(outputs[0].scale_millis, 1000);
    assert_eq!(outputs[0].refresh_millihz, None);
    assert!(outputs[0].modes.is_empty());
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
            interactive: false,
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
fn screenshot_bridge_request_accepts_region_with_nonzero_size() {
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

    assert_eq!(request.validate(), Ok(()));
}

#[test]
fn screenshot_bridge_request_rejects_region_with_zero_dimension() {
    let request = ScreenshotBridgeRequest {
        request_id: "req-2b".to_string(),
        kind: ScreenshotKind::FullOutput,
        output: None,
        include_cursor: false,
        region: Some(ScreenshotRegion {
            x: 0,
            y: 0,
            width: 0,
            height: 100,
        }),
        metadata: ScreenshotRequestMetadata::default(),
    };

    assert_eq!(
        request.validate(),
        Err(ScreenshotBridgeError::InvalidRequest(
            "region width and height must be nonzero".to_string()
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
                interactive: false,
            },
        },
    };
    let bytes = encode_screenshot_bridge_message(&message).expect("encode bridge request");
    let decoded =
        decode_screenshot_bridge_message(std::str::from_utf8(&bytes).expect("bridge request utf8"))
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
    let bytes = encode_screenshot_bridge_message(&message).expect("encode bridge success response");
    let decoded =
        decode_screenshot_bridge_message(std::str::from_utf8(&bytes).expect("bridge success utf8"))
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
            interactive: false,
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

#[test]
fn capture_window_thumbnail_command_roundtrip_is_supported() {
    let command = ShellCommand::CaptureWindowThumbnail {
        id: "win-1".to_string(),
        max_width: 200,
        max_height: 112,
    };
    let bytes = encode_command(&command).expect("encode");
    let decoded = decode_command(std::str::from_utf8(&bytes).expect("utf8")).expect("decode");
    assert_eq!(decoded, command);
}

#[test]
fn window_thumbnail_event_roundtrip_is_supported() {
    let event = ShellEvent::WindowThumbnail {
        id: "win-1".to_string(),
        path: "/tmp/test.rgba".to_string(),
        width: 200,
        height: 112,
    };
    let bytes = encode_event(&event).expect("encode");
    let decoded = decode_event(std::str::from_utf8(&bytes).expect("utf8")).expect("decode");
    assert_eq!(decoded, event);
}
