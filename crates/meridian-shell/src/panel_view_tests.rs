use meridian_ui::Widget;

use super::*;
use crate::{audio::AudioSnapshot, icons::IconCache, network::NetworkState};

#[test]
fn panel_chip_style_returns_correct_size() {
    let chip = PanelChip::new("test", "Test".into(), None, 58, false);
    let style = chip.style();
    assert_eq!(style.size.width, ui_length(58.0));
    assert_eq!(style.size.height, ui_length(CHIP_H as f32));
}

#[test]
fn tray_chip_widths_match() {
    assert_eq!(SNI_W, TRAY_W);
    assert_eq!(SCREENSHOT_W, PanelTokens::DEFAULT.control_width as i32);
    const { assert!(TRAY_W < SCREENSHOT_W) };
}

#[test]
fn panel_pinned_chip_pinned_app_idx_returns_idx() {
    let chip = PanelPinnedChip {
        idx: 2,
        label: "App".into(),
        icon: None,
        program: "prog".into(),
        args: vec![],
        window_count: 0,
        has_focused: false,
    };
    assert_eq!(chip.pinned_app_idx(), Some(2));
}

#[test]
fn panel_pinned_chip_launch_info_returns_program_and_args() {
    let chip = PanelPinnedChip {
        idx: 0,
        label: "Firefox".into(),
        icon: None,
        program: "firefox".into(),
        args: vec![],
        window_count: 0,
        has_focused: false,
    };
    assert_eq!(chip.launch_info(), Some(("firefox", &[] as &[String])));
}

#[test]
fn panel_window_chip_focus_window_id_returns_id() {
    let chip = PanelWindowChip {
        window_id: "win-1".into(),
        title: "Window".into(),
        focused: false,
        minimized: false,
        width: 100,
    };
    assert_eq!(chip.focus_window_id(), Some("win-1"));
}

#[test]
fn status_notifier_label_prefers_title_then_icon_then_service() {
    assert_eq!(
        status_notifier_label(&StatusNotifierItem {
            service: "org.example.Service".to_string(),
            title: Some("Dropbox".to_string()),
            icon_name: Some("cloud-sync".to_string()),
            menu_path: Some("/Menu".to_string()),
        }),
        "DR"
    );
    assert_eq!(
        status_notifier_label(&StatusNotifierItem {
            service: "org.example.Service".to_string(),
            title: None,
            icon_name: Some("cloud-sync".to_string()),
            menu_path: None,
        }),
        "CL"
    );
    assert_eq!(
        status_notifier_label(&StatusNotifierItem {
            service: "org.example.Service".to_string(),
            title: None,
            icon_name: None,
            menu_path: None,
        }),
        "SE"
    );
}

#[test]
fn status_icons_are_smaller_than_application_icons() {
    assert!(STATUS_ICON_SIZE < APP_ICON_SIZE);
}

#[test]
fn status_icon_tint_preserves_alpha_and_uses_premultiplied_theme_color() {
    let mut pixmap = Pixmap::new(1, 1).expect("pixmap");
    pixmap.data_mut().copy_from_slice(&[0, 0, 0, 128]);
    tint_pixmap_premul(&mut pixmap, Color::rgb(100, 80, 60));
    assert_eq!(pixmap.data(), &[50, 40, 30, 128]);
}

#[test]
fn build_panel_widget_tree_root_has_three_children() {
    let icon_cache = IconCache::new();
    let network = NetworkState::Disconnected;
    let audio = AudioSnapshot::unavailable();
    let tree = build_panel_widget_tree(
        1920,
        &[],
        &[],
        &network,
        &audio,
        &[],
        false,
        false,
        &crate::battery::BatterySnapshot::default(),
        None,
        1,
        9,
        "12:34",
        &icon_cache,
        None,
        &Theme::TOKYO_NIGHT_METRO,
    );
    // Floating-Island-Struktur: Root umschliesst die bar, die bar haelt die 3 Cluster.
    assert_eq!(tree.children().len(), 1, "root wraps the island bar");
    assert_eq!(
        tree.children()[0].children().len(),
        3,
        "bar holds left/center/right clusters"
    );
}

#[test]
fn draw_panel_ui_modifies_canvas_and_fills_clicks() {
    let width = 1024u32;
    let height = PANEL_HEIGHT;
    let mut canvas = vec![0u8; (width * height * 4) as usize];
    let icon_cache = IconCache::new();
    let network = NetworkState::Disconnected;
    let audio = AudioSnapshot::unavailable();
    let mut clicks = Vec::new();
    let state_fn = |_: &[usize]| WidgetState::Idle;

    draw_panel_ui(
        &mut canvas,
        width,
        height,
        &[],
        &[],
        &network,
        &audio,
        &[],
        false,
        false,
        &crate::battery::BatterySnapshot::default(),
        None,
        1,
        9,
        "12:34",
        &icon_cache,
        None,
        &meridian_config::ThemeConfig::default(),
        &state_fn,
        &mut clicks,
    );

    assert!(canvas.iter().any(|byte| *byte != 0));
    assert!(!clicks.is_empty());
    assert_eq!(
        clicks
            .iter()
            .filter(|zone| zone.id.as_deref() == Some("panel-status"))
            .count(),
        1,
        "the system tray is one Quick Settings click target"
    );
    assert!(clicks.iter().all(|zone| !matches!(
        zone.id.as_deref(),
        Some("panel-network" | "panel-sound" | "panel-battery")
    )));
}

#[test]
fn action_for_id_as_click_screenshot() {
    assert!(matches!(
        action_for_id_as_click("panel-screenshot"),
        Some(ClickAction::TakeScreenshot)
    ));
    assert!(matches!(
        action_for_id_as_click("panel-status"),
        Some(ClickAction::ToggleNetworkPopup)
    ));
    assert!(action_for_id_as_click("panel-sound").is_none());
}
