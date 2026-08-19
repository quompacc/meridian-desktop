#[test]
fn outputs_section_parses_two_outputs_with_relative_position() {
    let path = unique_test_path("outputs-two.toml");
    write(
        &path,
        r#"
[outputs.eDP-1]
primary = true
scale = 1.5
position = "auto"

[outputs.HDMI-A-1]
position = { right-of = "eDP-1" }
"#,
    );

    let config = MeridianConfig::load_from(&path).expect("outputs parse");
    assert_eq!(config.outputs.len(), 2);
    assert_eq!(config.outputs[0].name, "HDMI-A-1");
    assert_eq!(config.outputs[1].name, "eDP-1");
    assert_eq!(
        config.outputs[0].position,
        OutputPositionConfig::RightOf("eDP-1".to_string())
    );
    assert_eq!(config.outputs[1].position, OutputPositionConfig::Auto);
}

#[test]
fn outputs_position_table_variants_parse() {
    let path = unique_test_path("outputs-relations.toml");
    write(
        &path,
        r#"
[outputs.a]
position = { right-of = "base" }

[outputs.b]
position = { left-of = "base" }

[outputs.c]
position = { below = "base" }

[outputs.d]
position = { above = "base" }
"#,
    );

    let config = MeridianConfig::load_from(&path).expect("outputs parse");
    assert_eq!(
        output_by_name(&config, "a").position,
        OutputPositionConfig::RightOf("base".to_string())
    );
    assert_eq!(
        output_by_name(&config, "b").position,
        OutputPositionConfig::LeftOf("base".to_string())
    );
    assert_eq!(
        output_by_name(&config, "c").position,
        OutputPositionConfig::Below("base".to_string())
    );
    assert_eq!(
        output_by_name(&config, "d").position,
        OutputPositionConfig::Above("base".to_string())
    );
}

#[test]
fn outputs_position_coord_inline_table_parses() {
    let path = unique_test_path("outputs-coord.toml");
    write(
        &path,
        r#"
[outputs.DP-1]
position = { x = 100, y = 200 }
"#,
    );

    let config = MeridianConfig::load_from(&path).expect("outputs parse");
    assert_eq!(
        output_by_name(&config, "DP-1").position,
        OutputPositionConfig::Coord { x: 100, y: 200 }
    );
}

#[test]
fn outputs_position_string_auto_parses() {
    let path = unique_test_path("outputs-auto.toml");
    write(
        &path,
        r#"
[outputs.DP-1]
position = "auto"
"#,
    );

    let config = MeridianConfig::load_from(&path).expect("outputs parse");
    assert_eq!(
        output_by_name(&config, "DP-1").position,
        OutputPositionConfig::Auto
    );
}

#[test]
fn outputs_position_string_other_returns_error() {
    let path = unique_test_path("outputs-string-invalid.toml");

    write(
        &path,
        r#"
[outputs.DP-1]
position = "right-of:X"
"#,
    );
    assert!(MeridianConfig::load_from(&path).is_err());

    write(
        &path,
        r#"
[outputs.DP-1]
position = "left-of"
"#,
    );
    assert!(MeridianConfig::load_from(&path).is_err());

    write(
        &path,
        r#"
[outputs.DP-1]
position = "weird"
"#,
    );
    assert!(MeridianConfig::load_from(&path).is_err());
}

#[test]
fn outputs_position_multiple_relations_returns_error() {
    let path = unique_test_path("outputs-multi-rel.toml");
    write(
        &path,
        r#"
[outputs.DP-1]
position = { right-of = "A", below = "B" }
"#,
    );

    assert!(MeridianConfig::load_from(&path).is_err());
}

#[test]
fn outputs_position_xy_mixed_with_relation_returns_error() {
    let path = unique_test_path("outputs-xy-plus-rel.toml");
    write(
        &path,
        r#"
[outputs.DP-1]
position = { x = 0, y = 0, right-of = "A" }
"#,
    );

    assert!(MeridianConfig::load_from(&path).is_err());
}

#[test]
fn outputs_position_only_x_returns_error() {
    let path = unique_test_path("outputs-only-x.toml");
    write(
        &path,
        r#"
[outputs.DP-1]
position = { x = 100 }
"#,
    );

    assert!(MeridianConfig::load_from(&path).is_err());
}

#[test]
fn outputs_position_empty_table_means_auto() {
    let path = unique_test_path("outputs-empty-position-table.toml");
    write(
        &path,
        r#"
[outputs.DP-1]
position = {}
"#,
    );

    let config = MeridianConfig::load_from(&path).expect("outputs parse");
    assert_eq!(
        output_by_name(&config, "DP-1").position,
        OutputPositionConfig::Auto
    );
}

#[test]
fn outputs_defaults_when_only_name_set() {
    let path = unique_test_path("outputs-defaults.toml");
    write(
        &path,
        r#"
[outputs.eDP-1]
"#,
    );

    let config = MeridianConfig::load_from(&path).expect("outputs parse");
    let entry = output_by_name(&config, "eDP-1");
    assert!(!entry.primary);
    assert!(entry.enabled);
    assert_eq!(entry.scale, 1.0);
    assert_eq!(entry.position, OutputPositionConfig::Auto);
    assert!(entry.mode.is_none());
    assert!(entry.transform.is_none());
}

#[test]
fn outputs_mode_table_parses() {
    let with_refresh = unique_test_path("outputs-mode-with-refresh.toml");
    write(
        &with_refresh,
        r#"
[outputs.eDP-1]
mode = { width = 1920, height = 1080, refresh_millihz = 60000 }
"#,
    );
    let config = MeridianConfig::load_from(&with_refresh).expect("outputs parse");
    assert_eq!(
        output_by_name(&config, "eDP-1").mode,
        Some(OutputModeConfig {
            width: 1920,
            height: 1080,
            refresh_millihz: Some(60000),
        })
    );

    let without_refresh = unique_test_path("outputs-mode-without-refresh.toml");
    write(
        &without_refresh,
        r#"
[outputs.eDP-1]
mode = { width = 2560, height = 1440 }
"#,
    );
    let config = MeridianConfig::load_from(&without_refresh).expect("outputs parse");
    assert_eq!(
        output_by_name(&config, "eDP-1").mode,
        Some(OutputModeConfig {
            width: 2560,
            height: 1440,
            refresh_millihz: None,
        })
    );
}

#[test]
fn outputs_transform_passes_through_string_unvalidated() {
    let path = unique_test_path("outputs-transform.toml");
    write(
        &path,
        r#"
[outputs.DP-1]
transform = "90"

[outputs.eDP-1]
transform = "garbage"
"#,
    );

    let config = MeridianConfig::load_from(&path).expect("outputs parse");
    assert_eq!(
        output_by_name(&config, "DP-1").transform,
        Some("90".to_string())
    );
    assert_eq!(
        output_by_name(&config, "eDP-1").transform,
        Some("garbage".to_string())
    );
}
