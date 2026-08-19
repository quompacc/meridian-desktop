use super::*;

fn state(is_terminal: bool, is_pinned: bool) -> ContextMenuState {
    ContextMenuState {
        x: 0,
        y: 0,
        app_name: "Test".into(),
        exec: "test".into(),
        is_terminal,
        is_pinned,
        running_window_id: None,
        hover_idx: None,
    }
}

#[test]
fn item_list_non_terminal_non_pinned_has_five_items() {
    let items = item_list(false, false, false);
    assert_eq!(items.len(), 5);
    assert!(matches!(items[0].1, ContextMenuAction::Launch));
    assert!(matches!(items[1].1, ContextMenuAction::NewWindow));
    assert!(matches!(items[2].1, ContextMenuAction::LaunchInTerminal));
    assert!(matches!(items[3].1, ContextMenuAction::PinToPanel));
    assert!(matches!(items[4].1, ContextMenuAction::RemoveFromLauncher));
}

#[test]
fn item_list_terminal_pinned_has_four_items() {
    let items = item_list(true, true, false);
    assert_eq!(items.len(), 4);
    assert!(matches!(items[0].1, ContextMenuAction::Launch));
    assert!(matches!(items[1].1, ContextMenuAction::NewWindow));
    assert!(matches!(items[2].1, ContextMenuAction::UnpinFromPanel));
    assert!(matches!(items[3].1, ContextMenuAction::RemoveFromLauncher));
}

#[test]
fn item_list_non_terminal_pinned_shows_unpin() {
    let items = item_list(false, true, false);
    assert_eq!(items.len(), 5);
    assert!(matches!(items[3].1, ContextMenuAction::UnpinFromPanel));
    assert!(matches!(items[4].1, ContextMenuAction::RemoveFromLauncher));
}

#[test]
fn hit_item_above_menu_is_none() {
    let s = state(false, false);
    let items = item_list(false, false, false);
    assert!(hit_item(&s, items.len(), 50.0, -10.0).is_none());
}

#[test]
fn hit_item_first_row() {
    let s = state(false, false);
    let items = item_list(false, false, false);
    let n = items.len();
    let mid_y = (VPAD + ITEM_H / 2) as f64;
    assert_eq!(hit_item(&s, n, 50.0, mid_y), Some(0));
}

#[test]
fn hit_item_last_row() {
    let s = state(false, false);
    let items = item_list(false, false, false);
    let n = items.len();
    // last item is n-1, with 1px separator shift
    let last_top = VPAD + (n as i32 - 1) * ITEM_H + 1;
    let mid_y = (last_top + ITEM_H / 2) as f64;
    assert_eq!(hit_item(&s, n, 50.0, mid_y), Some(n - 1));
}

#[test]
fn contains_point_outside_returns_false() {
    let s = ContextMenuState {
        x: 100,
        y: 100,
        ..state(false, false)
    };
    let items = item_list(false, false, false);
    assert!(!contains_point(&s, items.len(), 50.0, 50.0));
}

#[test]
fn clamp_position_fits_inside_launcher() {
    let (x, y) = clamp_position(10, 10, 3, 880, 620);
    assert!(x >= 0 && x + MENU_WIDTH <= 880);
    assert!(y >= 0 && y + menu_height(3) <= 620);
}

#[test]
fn clamp_position_right_edge_clamped() {
    let (x, _) = clamp_position(870, 10, 3, 880, 620);
    assert!(x + MENU_WIDTH <= 880);
}

#[test]
fn clamp_position_bottom_edge_flips_up() {
    let n = 3;
    let (_, y) = clamp_position(10, 610, n, 880, 620);
    assert!(y + menu_height(n) <= 620);
}

#[test]
fn draw_overlay_does_not_panic() {
    let s = state(false, false);
    let items = item_list(false, false, false);
    let mut canvas = vec![0u8; 880 * 620 * 4];
    draw_overlay(
        &mut canvas,
        880,
        620,
        &s,
        &items,
        &[],
        &[],
        &ThemeConfig::default(),
    );
}

#[test]
fn draw_overlay_modifies_canvas_at_menu_location() {
    let s = state(false, false);
    let items = item_list(false, false, false);
    let mut canvas = vec![0u8; 880 * 620 * 4];
    draw_overlay(
        &mut canvas,
        880,
        620,
        &s,
        &items,
        &[],
        &[],
        &ThemeConfig::default(),
    );
    // At least some pixel in the menu area should be non-zero.
    let row_stride = 880 * 4;
    let menu_start = (s.y * row_stride + s.x * 4) as usize;
    assert!(canvas[menu_start..menu_start + MENU_WIDTH as usize * 4]
        .iter()
        .any(|b| *b != 0));
}

#[test]
fn desktop_item_list_has_five_items_with_settings_at_idx_three() {
    let items = desktop_item_list();
    assert_eq!(items.len(), 5);
    assert_eq!(items[0].1, DesktopContextMenuAction::Terminal);
    assert_eq!(items[1].1, DesktopContextMenuAction::Launcher);
    assert_eq!(items[2].1, DesktopContextMenuAction::FileManager);
    assert_eq!(
        items[SETTINGS_ITEM_IDX].1,
        DesktopContextMenuAction::Settings
    );
    assert_eq!(items[4].1, DesktopContextMenuAction::LockScreen);
}

#[test]
fn submenu_items_has_expected_categories() {
    let items = submenu_items();
    assert!(items.iter().any(|(_, a)| *a == SettingsSubAction::Display));
    assert!(items
        .iter()
        .any(|(_, a)| *a == SettingsSubAction::Wallpaper));
    assert!(items.iter().any(|(_, a)| *a == SettingsSubAction::Theme));
    assert!(items.iter().any(|(_, a)| *a == SettingsSubAction::Sound));
    assert!(items.iter().any(|(_, a)| *a == SettingsSubAction::Network));
    assert!(items.iter().any(|(_, a)| *a == SettingsSubAction::Power));
}

#[test]
fn submenu_hit_item_local_returns_correct_index() {
    let sub_x = (MENU_WIDTH + SUBMENU_GAP + 10) as f64;
    let first_mid_y = (VPAD + ITEM_H / 2) as f64;
    assert_eq!(submenu_hit_item_local(sub_x, first_mid_y), Some(0));
    assert_eq!(submenu_hit_item_local(5.0, first_mid_y), None);
}

#[test]
fn glass_menu_palette_uses_theme_colors() {
    let mut theme = ThemeConfig::default();
    theme.decorations.glass = true;
    theme.decorations.glass_blur = true;
    theme.colors.text = meridian_config::Color::rgb(1, 2, 3);
    theme.colors.border = meridian_config::Color::rgb(4, 5, 6);

    let pal = menu_palette_from_config(&theme);

    assert_eq!(
        (pal.text.r, pal.text.g, pal.text.b, pal.text.a),
        (1, 2, 3, 255)
    );
    assert_eq!(
        (pal.border.r, pal.border.g, pal.border.b, pal.border.a),
        (4, 5, 6, 255)
    );
}

#[test]
fn submenu_hit_item_local_hits_every_row_and_rejects_gap() {
    let x = (MENU_WIDTH + SUBMENU_GAP + 1) as f64;
    for i in 0..submenu_items().len() {
        let y = (VPAD + i as i32 * ITEM_H + ITEM_H / 2) as f64;
        assert_eq!(submenu_hit_item_local(x, y), Some(i));
    }

    let first_mid_y = (VPAD + ITEM_H / 2) as f64;
    assert_eq!(submenu_hit_item_local(MENU_WIDTH as f64, first_mid_y), None);
    assert_eq!(
        submenu_hit_item_local((MENU_WIDTH + SUBMENU_GAP - 1) as f64, first_mid_y),
        None
    );
    assert_eq!(submenu_hit_item_local(x, (VPAD - 1) as f64), None);
    assert_eq!(
        submenu_hit_item_local(x, (VPAD + submenu_items().len() as i32 * ITEM_H) as f64),
        None
    );
}

#[test]
fn total_menu_width_grows_when_submenu_open() {
    assert_eq!(total_menu_width(false), MENU_WIDTH);
    assert!(total_menu_width(true) > MENU_WIDTH);
}

#[test]
fn desktop_hit_item_uses_desktop_coordinates() {
    let state = DesktopContextMenuState {
        x: 40,
        y: 50,
        hover_idx: None,
        submenu_open: false,
        submenu_hover_idx: None,
    };
    assert_eq!(desktop_hit_item(&state, 55.0, 65.0), Some(0));
    assert_eq!(desktop_hit_item(&state, 5.0, 65.0), None);
}
