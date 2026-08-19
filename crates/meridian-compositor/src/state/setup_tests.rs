use meridian_config::{CursorConfig, MeridianConfig, ThemeManager, WallpaperConfig, WallpaperMode};

use super::apply_config_overrides;

#[test]
fn apply_config_overrides_marks_cursor_change_when_cursor_override_differs() {
    let mut theme_manager = ThemeManager::new();
    let config = MeridianConfig {
        cursor: Some(CursorConfig {
            theme: "Breeze".to_string(),
            size: 32,
        }),
        ..Default::default()
    };

    let changes = apply_config_overrides(&mut theme_manager, &config);

    assert!(!changes.theme_changed);
    assert!(changes.cursor_changed);
    assert!(!changes.wallpaper_changed);
    assert_eq!(theme_manager.current().config.cursor.theme, "Breeze");
    assert_eq!(theme_manager.current().config.cursor.size, 32);
}

#[test]
fn apply_config_overrides_marks_wallpaper_change_and_updates_override() {
    let mut theme_manager = ThemeManager::new();
    let config = MeridianConfig {
        wallpaper: Some(WallpaperConfig {
            path: "/tmp/wallpaper.png".to_string(),
            mode: WallpaperMode::Tile,
        }),
        ..Default::default()
    };

    let changes = apply_config_overrides(&mut theme_manager, &config);

    assert!(!changes.theme_changed);
    assert!(!changes.cursor_changed);
    assert!(changes.wallpaper_changed);
    let wallpaper = theme_manager
        .current()
        .config
        .wallpaper
        .as_ref()
        .expect("wallpaper override applied");
    assert_eq!(wallpaper.path, "/tmp/wallpaper.png");
    assert_eq!(wallpaper.mode, WallpaperMode::Tile);
}

#[test]
fn apply_config_overrides_with_unknown_theme_keeps_current_theme_and_flags_unchanged() {
    let mut theme_manager = ThemeManager::new();
    let mut config = MeridianConfig::default();
    config.general.theme = "theme-that-does-not-exist".to_string();

    let before = theme_manager.current().name.clone();
    let changes = apply_config_overrides(&mut theme_manager, &config);

    assert_eq!(theme_manager.current().name, before);
    assert!(!changes.theme_changed);
    assert!(!changes.cursor_changed);
    assert!(!changes.wallpaper_changed);
}
