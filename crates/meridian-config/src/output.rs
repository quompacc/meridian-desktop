use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum OutputPositionConfig {
    #[default]
    Auto,
    Coord {
        x: i32,
        y: i32,
    },
    RightOf(String),
    LeftOf(String),
    Below(String),
    Above(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputModeConfig {
    pub width: i32,
    pub height: i32,
    pub refresh_millihz: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputEntry {
    pub name: String,
    pub position: OutputPositionConfig,
    pub primary: bool,
    pub enabled: bool,
    pub scale: f64,
    pub transform: Option<String>,
    pub mode: Option<OutputModeConfig>,
}

impl OutputEntry {
    pub fn defaults_for(name: &str) -> Self {
        Self {
            name: name.to_string(),
            position: OutputPositionConfig::Auto,
            primary: false,
            enabled: true,
            scale: 1.0,
            transform: None,
            mode: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum OutputPositionToml {
    Str(String),
    Table(OutputPositionTableToml),
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct OutputPositionTableToml {
    x: Option<i32>,
    y: Option<i32>,
    #[serde(rename = "right-of")]
    right_of: Option<String>,
    #[serde(rename = "left-of")]
    left_of: Option<String>,
    below: Option<String>,
    above: Option<String>,
}

impl Default for OutputPositionToml {
    fn default() -> Self {
        Self::Str("auto".to_string())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct OutputModeToml {
    width: i32,
    height: i32,
    refresh_millihz: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct OutputToml {
    position: OutputPositionToml,
    primary: bool,
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default = "default_scale")]
    scale: f64,
    transform: Option<String>,
    mode: Option<OutputModeToml>,
}

impl Default for OutputToml {
    fn default() -> Self {
        Self {
            position: OutputPositionToml::default(),
            primary: false,
            enabled: default_enabled(),
            scale: default_scale(),
            transform: None,
            mode: None,
        }
    }
}

fn default_enabled() -> bool {
    true
}

fn default_scale() -> f64 {
    1.0
}

impl OutputPositionToml {
    fn into_config(self) -> Result<OutputPositionConfig, String> {
        match self {
            OutputPositionToml::Str(value) => {
                let trimmed = value.trim();
                if trimmed.eq_ignore_ascii_case("auto") {
                    Ok(OutputPositionConfig::Auto)
                } else {
                    Err(format!(
                        "invalid output position string: {:?} (only \"auto\" is valid as string; use inline table for coord or relation)",
                        value
                    ))
                }
            }
            OutputPositionToml::Table(table) => {
                let OutputPositionTableToml {
                    x,
                    y,
                    right_of,
                    left_of,
                    below,
                    above,
                } = table;

                let has_xy = x.is_some() || y.is_some();
                let relations = [
                    ("right-of", right_of),
                    ("left-of", left_of),
                    ("below", below),
                    ("above", above),
                ];
                let relation_set: Vec<(&str, String)> = relations
                    .into_iter()
                    .filter_map(|(key, value)| value.map(|target| (key, target)))
                    .collect();

                if has_xy && !relation_set.is_empty() {
                    return Err("output position cannot combine x/y with a relation".to_string());
                }
                if relation_set.len() > 1 {
                    let keys: Vec<&str> = relation_set.iter().map(|(key, _)| *key).collect();
                    return Err(format!(
                        "output position has multiple relations: {:?}",
                        keys
                    ));
                }

                if has_xy {
                    return match (x, y) {
                        (Some(coord_x), Some(coord_y)) => Ok(OutputPositionConfig::Coord {
                            x: coord_x,
                            y: coord_y,
                        }),
                        _ => Err("output position with x or y requires both x and y".to_string()),
                    };
                }

                if let Some((key, target)) = relation_set.into_iter().next() {
                    return Ok(match key {
                        "right-of" => OutputPositionConfig::RightOf(target),
                        "left-of" => OutputPositionConfig::LeftOf(target),
                        "below" => OutputPositionConfig::Below(target),
                        "above" => OutputPositionConfig::Above(target),
                        _ => unreachable!(),
                    });
                }

                Ok(OutputPositionConfig::Auto)
            }
        }
    }
}

impl OutputToml {
    pub(crate) fn into_entry(self, name: String) -> Result<OutputEntry, String> {
        Ok(OutputEntry {
            name,
            position: self.position.into_config()?,
            primary: self.primary,
            enabled: self.enabled,
            scale: self.scale,
            transform: self.transform,
            mode: self.mode.map(|mode| OutputModeConfig {
                width: mode.width,
                height: mode.height,
                refresh_millihz: mode.refresh_millihz,
            }),
        })
    }
}
