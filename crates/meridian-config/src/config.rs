use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{
    keybind::KeybindConfig,
    output::{OutputEntry, OutputToml},
    theme::{Wallpaper, WallpaperMode},
};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct GeneralConfig {
    pub theme: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CursorConfig {
    pub theme: String,
    pub size: u32,
}

impl Default for CursorConfig {
    fn default() -> Self {
        Self {
            // Meridian defaults to KDE Breeze so the desktop cursor
            // matches the small white login cursor by default. Source-only
            // builds without Breeze installed fall back through the compositor
            // cursor loader.
            theme: "Breeze_Light".to_string(),
            size: 24,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WallpaperConfig {
    pub path: String,
    pub mode: WallpaperMode,
}

impl Default for WallpaperConfig {
    fn default() -> Self {
        Self {
            path: String::new(),
            mode: WallpaperMode::Fill,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PinnedAppConfig {
    pub label: String,
    pub program: String,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PanelConfig {
    pub pinned: Vec<PinnedAppConfig>,
}

/// A wallpaper entry returned by scan_wallpaper_dirs.
/// Multiple resolution variants of the same pack are collapsed into one entry.
#[derive(Debug, Clone)]
pub struct WallpaperEntry {
    pub display_name: String,
    /// Best-quality image to apply (largest file in the group).
    pub apply_path: String,
    /// Smallest file — used for fast thumbnail decoding.
    pub thumbnail_path: String,
}

#[derive(Default)]
pub struct MeridianConfig {
    pub keybinds: KeybindConfig,
    pub general: GeneralConfig,
    pub cursor: Option<CursorConfig>,
    pub wallpaper: Option<WallpaperConfig>,
    pub outputs: Vec<OutputEntry>,
    pub panel: PanelConfig,
}

impl MeridianConfig {
    pub fn load() -> Self {
        let config_path = config_directory().join("config.toml");
        Self::load_or_default_from_path(&config_path)
    }

    pub fn reload(&mut self) -> Result<(), String> {
        let config_path = config_directory().join("config.toml");
        self.reload_from_path(&config_path)
    }

    pub fn reload_from_path(&mut self, path: &Path) -> Result<(), String> {
        if !path.exists() {
            info!("config file not found; using defaults ({:?})", path);
            *self = Self::default();
            return Ok(());
        }

        *self = Self::load_from(path)?;
        info!("Reloaded config from {:?}", path);
        Ok(())
    }

    fn load_from(path: &Path) -> Result<Self, String> {
        let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let toml: MeridianToml =
            toml::from_str(&raw).map_err(|e| format!("TOML parse error: {}", e))?;

        let keybinds = if toml.keybinds.is_empty() {
            KeybindConfig::default()
        } else {
            KeybindConfig::from_map(&toml.keybinds)?
        };

        Ok(Self {
            keybinds,
            general: GeneralConfig {
                theme: toml.general.theme,
            },
            cursor: toml.cursor.map(|cursor| CursorConfig {
                theme: cursor.theme,
                size: cursor.size,
            }),
            wallpaper: toml.wallpaper.map(|wallpaper| WallpaperConfig {
                path: wallpaper.path,
                mode: wallpaper.mode,
            }),
            outputs: toml
                .outputs
                .into_iter()
                .map(|(name, raw)| raw.into_entry(name))
                .collect::<Result<Vec<_>, String>>()?,
            panel: PanelConfig {
                pinned: toml
                    .panel
                    .pinned
                    .into_iter()
                    .map(|app| PinnedAppConfig {
                        label: app.label,
                        program: app.program,
                        icon: app.icon,
                    })
                    .collect(),
            },
        })
    }

    pub fn wallpaper_override(&self) -> Option<Wallpaper> {
        let wallpaper = self.wallpaper.as_ref()?;
        Some(Wallpaper {
            path: wallpaper.path.clone(),
            mode: wallpaper.mode,
        })
    }

    fn load_or_default_from_path(path: &Path) -> Self {
        if !path.exists() {
            info!("config file not found; using defaults ({:?})", path);
            return Self::default();
        }

        match Self::load_from(path) {
            Ok(config) => {
                info!("Loaded config from {:?}", path);
                config
            }
            Err(err) => {
                warn!(
                    "Failed to load config from {:?}: {} — using defaults",
                    path, err
                );
                Self::default()
            }
        }
    }
}

fn config_directory() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".config").join("meridian")
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PinnedAppToml {
    label: String,
    program: String,
    #[serde(default)]
    icon: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PanelToml {
    pinned: Vec<PinnedAppToml>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct MeridianToml {
    keybinds: HashMap<String, String>,
    general: GeneralToml,
    cursor: Option<CursorToml>,
    wallpaper: Option<WallpaperToml>,
    outputs: BTreeMap<String, OutputToml>,
    panel: PanelToml,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct GeneralToml {
    theme: String,
}

impl Default for GeneralToml {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct CursorToml {
    theme: String,
    size: u32,
}

impl Default for CursorToml {
    fn default() -> Self {
        Self {
            theme: "Breeze_Light".to_string(),
            size: 24,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct WallpaperToml {
    path: String,
    mode: WallpaperMode,
}

impl Default for WallpaperToml {
    fn default() -> Self {
        Self {
            path: String::new(),
            mode: WallpaperMode::Fill,
        }
    }
}

// Keep the large config parser/save tests near the parser fixtures they cover;
// moving them below the save/wallpaper helpers would be a noisy-only diff.
#[cfg_attr(test, allow(clippy::items_after_test_module))]
#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use super::{
        set_output_mode_in_toml, set_primary_output_in_toml, MeridianConfig, WallpaperMode,
    };
    use crate::{OutputModeConfig, OutputPositionConfig};

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
        assert_eq!(config.general.theme, "default");
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
        assert_eq!(config.general.theme, "default");
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
    fn reload_from_path_with_missing_file_resets_to_defaults() {
        let path = unique_test_path("reload-missing.toml");
        let mut config = MeridianConfig::default();
        config.general.theme = "custom".to_string();
        config.cursor = Some(super::CursorConfig {
            theme: "custom-cursor".to_string(),
            size: 33,
        });

        config.reload_from_path(&path).expect("reload missing");
        assert_eq!(config.general.theme, "default");
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
theme = "default"

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

    #[test]
    fn outputs_section_missing_keeps_empty_vec() {
        let path = unique_test_path("outputs-missing.toml");
        write(
            &path,
            r#"
[general]
theme = "default"
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
theme = "default"

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

        assert!(updated.contains("[general]\ntheme = \"default\""));
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

        assert!(
            updated.contains("mode = { width = 2560, height = 1440, refresh_millihz = 144000 }")
        );
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
}

impl MeridianConfig {
    /// Write (or update) only the theme key in the config file without
    /// touching any other settings the user may have made.
    pub fn save_theme(name: &str) {
        let config_path = config_directory().join("config.toml");
        let raw = if config_path.exists() {
            fs::read_to_string(&config_path).unwrap_or_default()
        } else {
            String::new()
        };

        let new_line = format!("theme = \"{}\"", name);
        let nl = '\n';

        // If the file already contains a `theme = ...` line inside
        // `[general]`, replace it in-place; otherwise append a
        // `[general]` section.
        let updated = if let Some(pos) = find_theme_line(&raw) {
            // Reconstruct with the theme line replaced.
            let mut out = String::new();
            for (i, l) in raw.lines().enumerate() {
                if i == pos {
                    out.push_str(&new_line);
                } else {
                    out.push_str(l);
                }
                out.push(nl);
            }
            out
        } else if has_general_section(&raw) {
            // Section exists but no theme key yet — insert after [general].
            let mut out = String::new();
            let mut inserted = false;
            for line in raw.lines() {
                out.push_str(line);
                out.push(nl);
                if !inserted && line.trim() == "[general]" {
                    out.push_str(&new_line);
                    out.push(nl);
                    inserted = true;
                }
            }
            out
        } else {
            // No [general] section at all — append it.
            let mut out = raw.clone();
            if !out.ends_with(nl) && !out.is_empty() {
                out.push(nl);
            }
            out.push_str("\n[general]\n");
            out.push_str(&new_line);
            out.push(nl);
            out
        };

        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(&config_path, updated.as_bytes()) {
            warn!("Failed to write theme to config: {}", e);
        } else {
            info!("Saved theme {:?} to {:?}", name, config_path);
        }
    }
    /// Write (or update) the [wallpaper] section in config.toml.
    /// Pass an empty `path` to remove the wallpaper section entirely.
    pub fn save_wallpaper(path: &str, mode: WallpaperMode) {
        let config_path = config_directory().join("config.toml");
        let raw = if config_path.exists() {
            fs::read_to_string(&config_path).unwrap_or_default()
        } else {
            String::new()
        };

        let stripped = strip_toml_section(&raw, "wallpaper");
        let updated = if path.is_empty() {
            stripped
        } else {
            let mut out = stripped;
            if !out.ends_with('\n') && !out.is_empty() {
                out.push('\n');
            }
            out.push_str("\n[wallpaper]\n");
            out.push_str(&format!("path = \"{}\"\n", path));
            out.push_str(&format!("mode = \"{}\"\n", mode));
            out
        };

        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(&config_path, updated.as_bytes()) {
            warn!("Failed to write wallpaper to config: {}", e);
        } else {
            info!(
                "Saved wallpaper {:?} mode={} to {:?}",
                path, mode, config_path
            );
        }
    }

    /// Write (or update) the [panel] section in config.toml with the current pinned apps.
    pub fn save_pinned_apps(apps: &[PinnedAppConfig]) {
        let config_path = config_directory().join("config.toml");
        let raw = if config_path.exists() {
            fs::read_to_string(&config_path).unwrap_or_default()
        } else {
            String::new()
        };

        let stripped = strip_toml_section(&raw, "panel");
        let mut out = stripped;
        if !out.ends_with('\n') && !out.is_empty() {
            out.push('\n');
        }
        out.push_str("\n[panel]\npinned = [\n");
        for app in apps {
            let icon_part = app
                .icon
                .as_deref()
                .map_or(String::new(), |i| format!(", icon = {:?}", i));
            out.push_str(&format!(
                "  {{ label = {:?}, program = {:?}{} }},\n",
                app.label, app.program, icon_part
            ));
        }
        out.push_str("]\n");

        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(&config_path, out.as_bytes()) {
            warn!("Failed to write pinned apps to config: {}", e);
        } else {
            info!("Saved {} pinned app(s) to {:?}", apps.len(), config_path);
        }
    }

    /// Mark one output as primary in config.toml while preserving the rest of
    /// each output section.
    pub fn save_primary_output(output_name: &str) {
        let output_name = output_name.trim();
        if output_name.is_empty() {
            warn!("Refusing to save empty primary output name");
            return;
        }

        let config_path = config_directory().join("config.toml");
        let raw = if config_path.exists() {
            fs::read_to_string(&config_path).unwrap_or_default()
        } else {
            String::new()
        };
        let updated = set_primary_output_in_toml(&raw, output_name);

        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(&config_path, updated.as_bytes()) {
            warn!("Failed to write primary output to config: {}", e);
        } else {
            info!(
                "Saved primary output {:?} to {:?}",
                output_name, config_path
            );
        }
    }

    /// Set the configured mode for one output while preserving the rest of the file.
    pub fn save_output_mode(
        output_name: &str,
        width: i32,
        height: i32,
        refresh_millihz: Option<i32>,
    ) {
        let output_name = output_name.trim();
        if output_name.is_empty() || width <= 0 || height <= 0 {
            warn!(
                "Refusing to save invalid output mode: output={:?} width={} height={}",
                output_name, width, height
            );
            return;
        }

        let config_path = config_directory().join("config.toml");
        let raw = if config_path.exists() {
            fs::read_to_string(&config_path).unwrap_or_default()
        } else {
            String::new()
        };
        let updated = set_output_mode_in_toml(&raw, output_name, width, height, refresh_millihz);

        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(&config_path, updated.as_bytes()) {
            warn!("Failed to write output mode to config: {}", e);
        } else {
            info!(
                "Saved output mode {}x{}@{:?} for {:?} to {:?}",
                width, height, refresh_millihz, output_name, config_path
            );
        }
    }

    /// Scan standard wallpaper directories; group resolution variants into one entry per pack.
    pub fn scan_wallpaper_dirs() -> Vec<WallpaperEntry> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let top_dirs: &[std::path::PathBuf] = &[
            std::path::PathBuf::from("/usr/share/wallpapers"),
            std::path::PathBuf::from("/usr/share/backgrounds"),
            std::path::PathBuf::from(format!("{}/Pictures", home)),
        ];
        let mut by_dir: std::collections::BTreeMap<std::path::PathBuf, Vec<(u64, String)>> =
            std::collections::BTreeMap::new();
        for dir in top_dirs {
            if dir.exists() {
                collect_images_by_dir(dir, &mut by_dir, 5);
            }
        }
        let mut entries: Vec<WallpaperEntry> = Vec::new();
        for (dir, mut files) in by_dir {
            if files.is_empty() {
                continue;
            }
            files.sort_by_key(|(sz, _)| *sz);
            let thumbnail_path = files[0].1.clone();
            let apply_path = files.last().unwrap().1.clone();
            let display_name = if files.len() == 1 {
                wallpaper_entry_display_name(&apply_path)
            } else {
                wallpaper_dir_display_name(&dir)
            };
            entries.push(WallpaperEntry {
                display_name,
                apply_path,
                thumbnail_path,
            });
        }
        entries.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        entries
    }
}

fn strip_toml_section(raw: &str, section: &str) -> String {
    let header = format!("[{}]", section);
    let mut out = String::new();
    let mut in_target = false;
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with('[') && !t.starts_with("[[") {
            in_target = t == header;
        }
        if !in_target {
            out.push_str(line);
            out.push('\n');
        }
    }
    let trimmed = out.trim_end_matches('\n').to_string();
    if trimmed.is_empty() {
        trimmed
    } else {
        trimmed + "\n"
    }
}

fn set_output_mode_in_toml(
    raw: &str,
    output_name: &str,
    width: i32,
    height: i32,
    refresh_millihz: Option<i32>,
) -> String {
    let mut out = String::new();
    let mut current_output: Option<String> = None;
    let mut current_output_has_mode = false;
    let mut found_target_output = false;

    for line in raw.lines() {
        if let Some(next_output) = output_section_name(line) {
            if current_output.as_deref() == Some(output_name) && !current_output_has_mode {
                push_output_mode_line(&mut out, width, height, refresh_millihz);
            }
            found_target_output |= next_output == output_name;
            current_output = Some(next_output);
            current_output_has_mode = false;
            out.push_str(line);
            out.push('\n');
            continue;
        }

        let trimmed = line.trim();
        if current_output.as_deref() == Some(output_name)
            && trimmed.starts_with('[')
            && !trimmed.starts_with("[[")
        {
            if !current_output_has_mode {
                push_output_mode_line(&mut out, width, height, refresh_millihz);
            }
            current_output = None;
            current_output_has_mode = false;
        }

        if current_output.as_deref() == Some(output_name) && is_mode_key_line(line) {
            push_output_mode_line(&mut out, width, height, refresh_millihz);
            current_output_has_mode = true;
            continue;
        }

        out.push_str(line);
        out.push('\n');
    }

    if current_output.as_deref() == Some(output_name) && !current_output_has_mode {
        push_output_mode_line(&mut out, width, height, refresh_millihz);
    }

    if !found_target_output {
        if !out.ends_with('\n') && !out.is_empty() {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(&format!("[outputs.{:?}]\n", output_name));
        out.push_str("enabled = true\n");
        out.push_str("position = \"auto\"\n");
        push_output_mode_line(&mut out, width, height, refresh_millihz);
    }

    out
}

fn set_primary_output_in_toml(raw: &str, output_name: &str) -> String {
    let mut out = String::new();
    let mut current_output: Option<String> = None;
    let mut current_output_has_primary = false;
    let mut found_target_output = false;

    for line in raw.lines() {
        if let Some(next_output) = output_section_name(line) {
            if let Some(previous_output) = current_output.take() {
                if !current_output_has_primary {
                    push_primary_line(&mut out, &previous_output, output_name);
                }
            }
            found_target_output |= next_output == output_name;
            current_output = Some(next_output);
            current_output_has_primary = false;
            out.push_str(line);
            out.push('\n');
            continue;
        }

        let trimmed = line.trim();
        if current_output.is_some() && trimmed.starts_with('[') && !trimmed.starts_with("[[") {
            if let Some(previous_output) = current_output.take() {
                if !current_output_has_primary {
                    push_primary_line(&mut out, &previous_output, output_name);
                }
            }
            current_output_has_primary = false;
        }

        if let Some(ref name) = current_output {
            if is_primary_key_line(line) {
                push_primary_line(&mut out, name, output_name);
                current_output_has_primary = true;
                continue;
            }
        }

        out.push_str(line);
        out.push('\n');
    }

    if let Some(previous_output) = current_output {
        if !current_output_has_primary {
            push_primary_line(&mut out, &previous_output, output_name);
        }
    }

    if !found_target_output {
        if !out.ends_with('\n') && !out.is_empty() {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(&format!("[outputs.{:?}]\n", output_name));
        out.push_str("primary = true\n");
        out.push_str("enabled = true\n");
        out.push_str("position = \"auto\"\n");
    }

    out
}

fn output_section_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.starts_with("[[") || !trimmed.starts_with("[outputs.") || !trimmed.ends_with(']') {
        return None;
    }
    let raw_name = trimmed.strip_prefix("[outputs.")?.strip_suffix(']')?.trim();
    if raw_name.is_empty() {
        return None;
    }
    if raw_name.starts_with('"') && raw_name.ends_with('"') && raw_name.len() >= 2 {
        return Some(raw_name[1..raw_name.len() - 1].replace("\\\"", "\""));
    }
    Some(raw_name.to_string())
}

fn is_mode_key_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix("mode") else {
        return false;
    };
    rest.trim_start().starts_with('=')
}

fn is_primary_key_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix("primary") else {
        return false;
    };
    rest.trim_start().starts_with('=')
}

fn push_output_mode_line(out: &mut String, width: i32, height: i32, refresh_millihz: Option<i32>) {
    out.push_str("mode = { width = ");
    out.push_str(&width.to_string());
    out.push_str(", height = ");
    out.push_str(&height.to_string());
    if let Some(refresh) = refresh_millihz {
        out.push_str(", refresh_millihz = ");
        out.push_str(&refresh.to_string());
    }
    out.push_str(" }\n");
}

fn push_primary_line(out: &mut String, current_output: &str, primary_output: &str) {
    out.push_str("primary = ");
    out.push_str(if current_output == primary_output {
        "true"
    } else {
        "false"
    });
    out.push('\n');
}

const WALLPAPER_SKIP: &[&str] = &[
    "usr",
    "share",
    "wallpapers",
    "backgrounds",
    "contents",
    "images",
    "pictures",
    "home",
];

fn collect_images_by_dir(
    dir: &std::path::Path,
    out: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<(u64, String)>>,
    depth: usize,
) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_images_by_dir(&path, out, depth - 1);
        } else {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase());
            if matches!(ext.as_deref(), Some("png" | "jpg" | "jpeg" | "webp")) {
                let sz = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                if let (Some(s), Some(parent)) = (path.to_str(), path.parent()) {
                    out.entry(parent.to_path_buf())
                        .or_default()
                        .push((sz, s.to_string()));
                }
            }
        }
    }
}

fn wallpaper_entry_display_name(path: &str) -> String {
    let meaningful: Vec<&str> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .filter(|p| {
            let lo = p.to_ascii_lowercase();
            !WALLPAPER_SKIP.contains(&lo.as_str())
        })
        .collect();
    let filename = meaningful.last().copied().unwrap_or(path);
    let stem = filename.rsplitn(2, '.').last().unwrap_or(filename);
    if meaningful.len() >= 2 {
        format!("{} \u{00b7} {}", meaningful[meaningful.len() - 2], stem)
    } else {
        stem.to_string()
    }
}

fn wallpaper_dir_display_name(dir: &std::path::Path) -> String {
    let s = dir.to_str().unwrap_or("");
    let meaningful: Vec<&str> = s
        .split('/')
        .filter(|c| !c.is_empty())
        .filter(|p| {
            let lo = p.to_ascii_lowercase();
            !WALLPAPER_SKIP.contains(&lo.as_str())
        })
        .collect();
    meaningful.last().copied().unwrap_or(s).to_string()
}

fn has_general_section(raw: &str) -> bool {
    raw.lines().any(|l| l.trim() == "[general]")
}

/// Returns the line index of the first `theme = ...` key that appears
/// after a `[general]` section header.
fn find_theme_line(raw: &str) -> Option<usize> {
    let mut in_general = false;
    for (i, line) in raw.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('[') {
            in_general = t == "[general]";
            continue;
        }
        if in_general && t.starts_with("theme") && t.contains('=') {
            return Some(i);
        }
    }
    None
}
