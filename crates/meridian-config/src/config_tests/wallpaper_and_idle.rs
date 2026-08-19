#[test]
fn outputs_section_missing_keeps_empty_vec() {
    let path = unique_test_path("outputs-missing.toml");
    write(
        &path,
        r#"
[general]
theme = "meridian"
"#,
    );

    let config = MeridianConfig::load_from(&path).expect("config parse");
    assert!(config.outputs.is_empty());
}

#[test]
fn outputs_unknown_field_returns_error() {
    let path = unique_test_path("outputs-unknown-field.toml");
    write(
        &path,
        r#"
[outputs.eDP-1]
foo = 42
"#,
    );

    assert!(MeridianConfig::load_from(&path).is_err());
}

#[test]
fn reload_with_outputs_replaces_previous_set() {
    let path = unique_test_path("outputs-reload.toml");
    write(
        &path,
        r#"
[outputs.eDP-1]
position = "auto"
"#,
    );

    let mut config = MeridianConfig::default();
    config.reload_from_path(&path).expect("first reload");
    assert_eq!(config.outputs.len(), 1);
    assert_eq!(config.outputs[0].name, "eDP-1");

    write(
        &path,
        r#"
[outputs.HDMI-A-1]
position = { right-of = "eDP-1" }
"#,
    );
    config.reload_from_path(&path).expect("second reload");
    assert_eq!(config.outputs.len(), 1);
    assert_eq!(config.outputs[0].name, "HDMI-A-1");
    assert_eq!(
        config.outputs[0].position,
        OutputPositionConfig::RightOf("eDP-1".to_string())
    );
}

#[test]
fn outputs_section_preserved_with_other_sections() {
    let path = unique_test_path("outputs-with-other-sections.toml");
    write(
        &path,
        r#"
[general]
theme = "catppuccin-mocha"

[cursor]
theme = "Vanilla-DMZ"
size = 30

[wallpaper]
path = "/tmp/bg.png"
mode = "fit"

[outputs.eDP-1]
primary = true
enabled = true
scale = 1.25
position = "auto"
transform = "normal"
mode = { width = 1920, height = 1080 }

[keybinds]
"Super+Space" = "toggle-launcher"
"#,
    );

    let config = MeridianConfig::load_from(&path).expect("full config parse");
    assert_eq!(config.general.theme, "catppuccin-mocha");
    assert!(config.cursor.is_some());
    assert!(config.wallpaper.is_some());
    assert_eq!(config.outputs.len(), 1);
    let output = output_by_name(&config, "eDP-1");
    assert!(output.primary);
    assert_eq!(output.scale, 1.25);
    assert_eq!(output.position, OutputPositionConfig::Auto);
    assert_eq!(output.transform, Some("normal".to_string()));
    assert_eq!(
        output.mode,
        Some(OutputModeConfig {
            width: 1920,
            height: 1080,
            refresh_millihz: None,
        })
    );
    assert!(!config.keybinds.bindings().is_empty());
}

#[test]
fn set_primary_output_updates_existing_output_sections() {
    let updated = set_primary_output_in_toml(
        r#"
[general]
theme = "meridian"

[outputs.eDP-1]
primary = true
scale = 1.25

[outputs.HDMI-A-1]
position = { right-of = "eDP-1" }

[panel]
pinned = []
"#,
        "HDMI-A-1",
    );

    assert!(updated.contains("[general]\ntheme = \"meridian\""));
    assert!(updated.contains("[outputs.eDP-1]\nprimary = false\nscale = 1.25"));
    assert!(updated.contains("[outputs.HDMI-A-1]\nposition = { right-of = \"eDP-1\" }"));
    assert!(updated.contains("position = { right-of = \"eDP-1\" }\n\nprimary = true"));
    assert!(updated.contains("[panel]\npinned = []"));
}

#[test]
fn set_primary_output_appends_missing_output_section() {
    let updated = set_primary_output_in_toml(
        r#"
[outputs.eDP-1]
primary = true
"#,
        "DP-3",
    );

    assert!(updated.contains("[outputs.eDP-1]\nprimary = false"));
    assert!(updated.contains("[outputs.\"DP-3\"]\nprimary = true"));
    assert!(updated.contains("position = \"auto\""));
}

#[test]
fn set_output_mode_updates_existing_output_section() {
    let updated = set_output_mode_in_toml(
        r#"
[outputs.eDP-1]
primary = true
mode = { width = 1920, height = 1080, refresh_millihz = 60000 }
scale = 1.25
"#,
        "eDP-1",
        2560,
        1440,
        Some(144_000),
    );

    assert!(updated.contains("mode = { width = 2560, height = 1440, refresh_millihz = 144000 }"));
    assert!(updated.contains("scale = 1.25"));
}

#[test]
fn set_output_mode_appends_missing_output_section() {
    let updated =
        set_output_mode_in_toml("[general]\ntheme = \"default\"\n", "DP-3", 1920, 1080, None);

    assert!(updated.contains("[outputs.\"DP-3\"]"));
    assert!(updated.contains("enabled = true"));
    assert!(updated.contains("position = \"auto\""));
    assert!(updated.contains("mode = { width = 1920, height = 1080 }"));
}

#[test]
fn set_output_key_replaces_existing_scale_in_section() {
    let raw = "[outputs.\"DP-1\"]\nenabled = true\nscale = 1.0\nposition = \"auto\"\n";
    let updated = super::set_output_key_in_toml(raw, "DP-1", "scale", "1.5");
    assert!(updated.contains("scale = 1.5"));
    assert!(!updated.contains("scale = 1.0"));
    // unrelated keys preserved
    assert!(updated.contains("position = \"auto\""));
    let path = unique_test_path("out-scale.toml");
    write(&path, &updated);
    let config = MeridianConfig::load_from(&path).expect("round-trips");
    assert_eq!(output_by_name(&config, "DP-1").scale, 1.5);
}

#[test]
fn set_output_key_inserts_transform_when_absent_and_round_trips() {
    let raw = "[outputs.\"DP-1\"]\nenabled = true\n";
    let updated = super::set_output_key_in_toml(raw, "DP-1", "transform", "\"90\"");
    assert!(updated.contains("transform = \"90\""));
    let path = unique_test_path("out-transform.toml");
    write(&path, &updated);
    let config = MeridianConfig::load_from(&path).expect("round-trips");
    assert_eq!(
        output_by_name(&config, "DP-1").transform.as_deref(),
        Some("90")
    );
}

#[test]
fn set_output_key_appends_section_when_output_absent() {
    let updated = super::set_output_key_in_toml("[general]\ntheme = \"x\"\n", "DP-9", "scale", "2");
    assert!(updated.contains("[outputs.\"DP-9\"]"));
    assert!(updated.contains("scale = 2"));
}

#[test]
fn remove_output_key_drops_transform_only_for_target() {
    let raw = "[outputs.\"DP-1\"]\ntransform = \"90\"\nscale = 1.0\n[outputs.\"DP-2\"]\ntransform = \"180\"\n";
    let updated = super::remove_output_key_in_toml(raw, "DP-1", "transform");
    // DP-1 transform gone, its scale kept, DP-2 transform untouched.
    let dp1 = updated.split("[outputs.\"DP-2\"]").next().unwrap();
    assert!(!dp1.contains("transform"));
    assert!(dp1.contains("scale = 1.0"));
    assert!(updated.contains("[outputs.\"DP-2\"]\ntransform = \"180\""));
}

fn output_by_name<'a>(config: &'a MeridianConfig, name: &str) -> &'a crate::OutputEntry {
    config
        .outputs
        .iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("missing output entry: {}", name))
}

fn unique_test_path(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "meridian-config-test-{}-{}-{}",
        std::process::id(),
        nanos,
        name
    ))
}

fn write(path: &std::path::Path, content: &str) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(path, content).expect("write test file");
}
