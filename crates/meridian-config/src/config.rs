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
            // "default" exists on every distro (it's a meta-theme that
            // points at whatever the system picked) so the out-of-box
            // experience always works. Users wanting the classic DMZ
            // arrow set should install dmz-cursor-theme (Debian/Ubuntu)
            // and either set theme = "DMZ-White" in their config or
            // symlink Vanilla-DMZ -> DMZ-White. See INSTALL.md.
            theme: "default".to_string(),
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
            theme: "default".to_string(),
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

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use super::{MeridianConfig, WallpaperMode};
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
