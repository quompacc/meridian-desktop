#[test]
fn panel_pinned_apps_parse_from_toml() {
    let path = unique_test_path("panel-pinned.toml");
    write(
        &path,
        r#"
[panel]
pinned = [
  { label = "Term", program = "kitty", icon = "utilities-terminal" },
  { label = "Web", program = "firefox" },
]
"#,
    );
    let config = MeridianConfig::load_from(&path).expect("valid config");
    assert_eq!(config.panel.pinned.len(), 2);
    assert_eq!(config.panel.pinned[0].label, "Term");
    assert_eq!(config.panel.pinned[0].program, "kitty");
    assert_eq!(
        config.panel.pinned[0].icon.as_deref(),
        Some("utilities-terminal")
    );
    assert_eq!(config.panel.pinned[1].label, "Web");
    assert_eq!(config.panel.pinned[1].program, "firefox");
    assert!(config.panel.pinned[1].icon.is_none());
}

#[test]
fn panel_section_missing_gives_empty_pinned() {
    let path = unique_test_path("panel-missing.toml");
    write(&path, "[general]\ntheme = \"default\"\n");
    let config = MeridianConfig::load_from(&path).expect("valid config");
    assert!(config.panel.pinned.is_empty());
}

#[test]
fn panel_pinned_unknown_field_returns_error() {
    let path = unique_test_path("panel-unknown.toml");
    write(
        &path,
        "[panel]\npinned = [{ label = \"X\", program = \"x\", bogus = true }]\n",
    );
    assert!(MeridianConfig::load_from(&path).is_err());
}

#[test]
fn missing_file_uses_defaults() {
    let path = unique_test_path("missing.toml");
    let config = MeridianConfig::load_or_default_from_path(&path);
    assert_eq!(config.general.theme, "dark");
    assert!(config.cursor.is_none());
    assert!(config.wallpaper.is_none());
}

#[test]
fn valid_toml_parses_general_cursor_and_wallpaper() {
    let path = unique_test_path("valid.toml");
    write(
        &path,
        r#"
[general]
theme = "catppuccin-mocha"

[cursor]
# Cursor-Theme aus /usr/share/icons/<Theme>/cursors/
# Fallback-Stack: <theme> -> Adwaita -> default -> embedded
theme = "Vanilla-DMZ"
size = 24

[wallpaper]
path = "/tmp/wall.png"
mode = "fill"
"#,
    );

    let config = MeridianConfig::load_from(&path).expect("valid config");
    assert_eq!(config.general.theme, "catppuccin-mocha");
    let cursor = config.cursor.expect("cursor section");
    assert_eq!(cursor.theme, "Vanilla-DMZ");
    assert_eq!(cursor.size, 24);
    let wallpaper = config.wallpaper.expect("wallpaper section");
    assert_eq!(wallpaper.path, "/tmp/wall.png");
    assert_eq!(wallpaper.mode, WallpaperMode::Fill);
}

#[test]
fn invalid_toml_falls_back_to_defaults() {
    let path = unique_test_path("invalid.toml");
    write(&path, r#"[general theme = "broken""#);
    let config = MeridianConfig::load_or_default_from_path(&path);
    assert_eq!(config.general.theme, "dark");
    assert!(config.cursor.is_none());
    assert!(config.wallpaper.is_none());
}

#[test]
fn cursor_and_wallpaper_modes_parse() {
    let path = unique_test_path("cursor-wallpaper.toml");
    write(
        &path,
        r#"
[cursor]
# Cursor-Theme aus /usr/share/icons/<Theme>/cursors/
# Fallback-Stack: <theme> -> Adwaita -> default -> embedded
theme = "Vanilla-DMZ"
size = 32

[wallpaper]
path = "~/wallpapers/space.png"
mode = "fit"
"#,
    );
    let config = MeridianConfig::load_from(&path).expect("valid config");
    let cursor = config.cursor.expect("cursor");
    assert_eq!(cursor.theme, "Vanilla-DMZ");
    assert_eq!(cursor.size, 32);
    let wallpaper = config.wallpaper.expect("wallpaper");
    assert_eq!(wallpaper.path, "~/wallpapers/space.png");
    assert_eq!(wallpaper.mode, WallpaperMode::Fit);
}

#[test]
fn reload_from_path_with_valid_file_updates_all_sections() {
    let path = unique_test_path("reload-valid.toml");
    write(
        &path,
        r#"
[general]
theme = "catppuccin-mocha"

[cursor]
# Cursor-Theme aus /usr/share/icons/<Theme>/cursors/
# Fallback-Stack: <theme> -> Adwaita -> default -> embedded
theme = "Vanilla-DMZ"
size = 28

[wallpaper]
path = ""
mode = "tile"
"#,
    );

    let mut config = MeridianConfig::default();
    config.reload_from_path(&path).expect("reload valid");
    assert_eq!(config.general.theme, "catppuccin-mocha");
    let cursor = config.cursor.expect("cursor");
    assert_eq!(cursor.theme, "Vanilla-DMZ");
    assert_eq!(cursor.size, 28);
    let wallpaper = config.wallpaper.expect("wallpaper");
    assert_eq!(wallpaper.path, "");
    assert_eq!(wallpaper.mode, WallpaperMode::Tile);
}

#[test]
fn reload_from_path_with_invalid_file_returns_error_and_preserves_old_config() {
    let path = unique_test_path("reload-invalid.toml");
    write(&path, r#"[general theme = "broken""#);

    let mut config = MeridianConfig::default();
    config.general.theme = "old-theme".to_string();
    config.cursor = Some(super::CursorConfig {
        theme: "old-cursor".to_string(),
        size: 31,
    });

    let result = config.reload_from_path(&path);
    assert!(result.is_err());
    assert_eq!(config.general.theme, "old-theme");
    let cursor = config.cursor.expect("cursor preserved");
    assert_eq!(cursor.theme, "old-cursor");
    assert_eq!(cursor.size, 31);
}

#[test]
fn set_cursor_in_toml_replaces_existing_section_and_round_trips() {
    let raw = r#"[general]
theme = "meridian"

[cursor]
theme = "Old"
size = 24

[wallpaper]
path = "/tmp/bg.png"
mode = "fill"
"#;
    let updated = super::set_cursor_in_toml(raw, "Breeze_Light", 48);

    // The old cursor values are gone, the new ones present, and unrelated
    // sections survive.
    assert!(!updated.contains("size = 24"));
    assert!(updated.contains("theme = \"Breeze_Light\""));
    assert!(updated.contains("size = 48"));
    assert!(updated.contains("[wallpaper]"));

    let path = unique_test_path("set-cursor.toml");
    write(&path, &updated);
    let config = MeridianConfig::load_from(&path).expect("round-trips");
    let cursor = config.cursor.expect("cursor section");
    assert_eq!(cursor.theme, "Breeze_Light");
    assert_eq!(cursor.size, 48);
    assert_eq!(config.general.theme, "meridian");
}

#[test]
fn set_cursor_in_toml_appends_when_absent() {
    let updated = super::set_cursor_in_toml("[general]\ntheme = \"default\"\n", "X", 32);
    assert!(updated.contains("[cursor]"));
    assert!(updated.contains("size = 32"));

    let path = unique_test_path("append-cursor.toml");
    write(&path, &updated);
    let config = MeridianConfig::load_from(&path).expect("round-trips");
    assert_eq!(config.cursor.expect("cursor").size, 32);
}

#[test]
fn set_idle_timeout_replaces_existing_key_and_round_trips() {
    let raw = "[general]\ntheme = \"default\"\nidle_timeout_secs = 60\n";
    let updated = super::set_idle_timeout_in_toml(raw, Some(600));
    assert!(!updated.contains("= 60\n"));
    assert!(updated.contains("idle_timeout_secs = 600"));

    let path = unique_test_path("idle-replace.toml");
    write(&path, &updated);
    let config = MeridianConfig::load_from(&path).expect("round-trips");
    assert_eq!(config.general.idle_timeout_secs, Some(600));
    assert_eq!(config.general.theme, "default");
}

#[test]
fn set_idle_timeout_inserts_into_existing_general_section() {
    let updated = super::set_idle_timeout_in_toml("[general]\ntheme = \"x\"\n", Some(300));
    assert!(updated.contains("idle_timeout_secs = 300"));

    let path = unique_test_path("idle-insert.toml");
    write(&path, &updated);
    let config = MeridianConfig::load_from(&path).expect("round-trips");
    assert_eq!(config.general.idle_timeout_secs, Some(300));
}

#[test]
fn set_idle_timeout_appends_general_when_absent() {
    let updated = super::set_idle_timeout_in_toml("", Some(120));
    assert!(updated.contains("[general]"));
    assert!(updated.contains("idle_timeout_secs = 120"));

    let path = unique_test_path("idle-append.toml");
    write(&path, &updated);
    let config = MeridianConfig::load_from(&path).expect("round-trips");
    assert_eq!(config.general.idle_timeout_secs, Some(120));
}

#[test]
fn set_idle_timeout_none_removes_the_key() {
    let raw = "[general]\ntheme = \"default\"\nidle_timeout_secs = 60\n";
    let updated = super::set_idle_timeout_in_toml(raw, None);
    assert!(!updated.contains("idle_timeout_secs"));
    assert!(updated.contains("theme = \"default\""));

    let path = unique_test_path("idle-remove.toml");
    write(&path, &updated);
    let config = MeridianConfig::load_from(&path).expect("round-trips");
    assert_eq!(config.general.idle_timeout_secs, None);
}

#[test]
fn set_idle_timeout_none_on_missing_key_is_noop() {
    let raw = "[general]\ntheme = \"default\"\n";
    assert_eq!(super::set_idle_timeout_in_toml(raw, None), raw);
}

#[test]
fn reload_from_path_with_missing_file_resets_to_defaults() {
    let path = unique_test_path("reload-missing.toml");
    let mut config = MeridianConfig::default();
    config.general.theme = "custom".to_string();
    config.cursor = Some(super::CursorConfig {
        theme: "custom-cursor".to_string(),
        size: 33,
    });

    config.reload_from_path(&path).expect("reload missing");
    assert_eq!(config.general.theme, "dark");
    assert!(config.cursor.is_none());
    assert!(config.wallpaper.is_none());
}

#[test]
fn keybinds_section_remains_supported() {
    let path = unique_test_path("keybinds.toml");
    write(
        &path,
        r#"
[keybinds]
"Super+1" = "workspace 1"
"Super+Space" = "toggle-launcher"
"#,
    );
    let config = MeridianConfig::load_from(&path).expect("valid keybind config");
    assert!(config.keybinds.bindings().len() >= 2);
}

#[test]
fn reload_from_path_with_invalid_keybind_keeps_previous_config() {
    let path = unique_test_path("invalid-keybind.toml");
    write(
        &path,
        r#"
[general]
theme = "meridian"

[keybinds]
"Super+NotARealKey" = "toggle-tiling"
"#,
    );

    let mut config = MeridianConfig::default();
    config.general.theme = "old-theme".to_string();

    let result = config.reload_from_path(&path);
    assert!(result.is_err());
    assert_eq!(config.general.theme, "old-theme");
}
