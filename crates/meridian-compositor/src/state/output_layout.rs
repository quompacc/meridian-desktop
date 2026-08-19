use meridian_config::{OutputEntry, OutputPositionConfig};
use smithay::utils::Transform;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum OutputPosition {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputPlacement {
    pub name: String,
    pub position: OutputPosition,
    pub primary: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutputLayout {
    pub placements: Vec<OutputPlacement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOutput {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedOutput {
    pub name: String,
    pub width: i32,
    pub height: i32,
}

/// Per-output diff between previous and next config entries.
/// Used by reload propagation to decide which kind of live update is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OutputReloadDiff {
    pub mode_changed: bool,
    pub enabled_changed: bool,
}

impl OutputLayout {
    pub fn from_config_entries(entries: &[OutputEntry]) -> Self {
        Self {
            placements: entries.iter().map(OutputPlacement::from).collect(),
        }
    }

    pub fn placement_for<'a>(&'a self, name: &str) -> Option<&'a OutputPlacement> {
        self.placements
            .iter()
            .find(|placement| placement.name == name)
    }

    pub fn resolve(&self, connected: &[ConnectedOutput]) -> Vec<ResolvedOutput> {
        let enabled_connected_names: Vec<&str> = connected
            .iter()
            .filter_map(|output| {
                let enabled = self
                    .placement_for(&output.name)
                    .map(|placement| placement.enabled)
                    .unwrap_or(true);
                enabled.then_some(output.name.as_str())
            })
            .collect();

        let mut explicit_primary: Option<&str> = None;
        for placement in self.placements.iter().filter(|placement| placement.primary) {
            if !enabled_connected_names.contains(&placement.name.as_str()) {
                continue;
            }

            if explicit_primary.is_some() {
                tracing::warn!(
                    selected = ?explicit_primary,
                    ignored = %placement.name,
                    "multiple primary flags set; keeping first placement primary"
                );
                continue;
            }

            explicit_primary = Some(placement.name.as_str());
        }

        let mut resolved = Vec::with_capacity(connected.len());

        for output in connected {
            let placement = self.placement_for(&output.name);
            let enabled = placement.map(|p| p.enabled).unwrap_or(true);
            let position = placement
                .map(|p| p.position.clone())
                .unwrap_or(OutputPosition::Auto);

            let (x, y) = if !enabled {
                (0, 0)
            } else {
                resolve_position(&resolved, connected, output, &position)
            };

            let resolved_output = ResolvedOutput {
                name: output.name.clone(),
                x,
                y,
                width: output.width,
                height: output.height,
                primary: false,
                enabled,
            };

            resolved.push(resolved_output);
        }

        apply_primary_selection(&mut resolved, explicit_primary);

        for output in &resolved {
            tracing::debug!(
                name = %output.name,
                x = output.x,
                y = output.y,
                width = output.width,
                height = output.height,
                primary = output.primary,
                "resolved output"
            );
        }

        resolved
    }
}

fn apply_primary_selection(resolved: &mut [ResolvedOutput], explicit_primary: Option<&str>) {
    if let Some(primary_name) = explicit_primary {
        if let Some(output) = resolved
            .iter_mut()
            .find(|output| output.enabled && output.name == primary_name)
        {
            output.primary = true;
            return;
        }
    }

    if let Some(output) = resolved.iter_mut().find(|output| output.enabled) {
        output.primary = true;
    }
}

fn resolve_position(
    resolved: &[ResolvedOutput],
    connected: &[ConnectedOutput],
    current: &ConnectedOutput,
    position: &OutputPosition,
) -> (i32, i32) {
    match position {
        OutputPosition::Auto => auto_position(resolved),
        OutputPosition::Coord { x, y } => (*x, *y),
        OutputPosition::RightOf(target) => {
            resolve_relative_to_target(resolved, connected, current, target, |source, _| {
                (source.x + source.width, source.y)
            })
        }
        OutputPosition::LeftOf(target) => resolve_relative_to_target(
            resolved,
            connected,
            current,
            target,
            |source, current_output| (source.x - current_output.width, source.y),
        ),
        OutputPosition::Below(target) => {
            resolve_relative_to_target(resolved, connected, current, target, |source, _| {
                (source.x, source.y + source.height)
            })
        }
        OutputPosition::Above(target) => resolve_relative_to_target(
            resolved,
            connected,
            current,
            target,
            |source, current_output| (source.x, source.y - current_output.height),
        ),
    }
}

fn auto_position(resolved: &[ResolvedOutput]) -> (i32, i32) {
    let x = resolved
        .iter()
        .filter(|output| output.enabled)
        .map(|output| output.width)
        .sum();

    (x, 0)
}

fn resolve_relative_to_target<F>(
    resolved: &[ResolvedOutput],
    connected: &[ConnectedOutput],
    current: &ConnectedOutput,
    target_name: &str,
    resolver: F,
) -> (i32, i32)
where
    F: Fn(&ResolvedOutput, &ConnectedOutput) -> (i32, i32),
{
    if let Some(target) = resolved
        .iter()
        .find(|output| output.name == target_name && output.enabled)
    {
        return resolver(target, current);
    }

    if connected.iter().all(|output| output.name != target_name) {
        tracing::warn!(
            output = %current.name,
            target = %target_name,
            "dangling output reference in placement; falling back to auto"
        );
    } else {
        tracing::warn!(
            output = %current.name,
            target = %target_name,
            "target not yet resolved (possible cycle) or disabled; falling back to auto"
        );
    }

    auto_position(resolved)
}

impl From<&OutputEntry> for OutputPlacement {
    fn from(entry: &OutputEntry) -> Self {
        Self {
            name: entry.name.clone(),
            position: match &entry.position {
                OutputPositionConfig::Auto => OutputPosition::Auto,
                OutputPositionConfig::Coord { x, y } => OutputPosition::Coord { x: *x, y: *y },
                OutputPositionConfig::RightOf(target) => OutputPosition::RightOf(target.clone()),
                OutputPositionConfig::LeftOf(target) => OutputPosition::LeftOf(target.clone()),
                OutputPositionConfig::Below(target) => OutputPosition::Below(target.clone()),
                OutputPositionConfig::Above(target) => OutputPosition::Above(target.clone()),
            },
            primary: entry.primary,
            enabled: entry.enabled,
        }
    }
}

pub fn parse_output_transform(value: &str) -> Transform {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" | "" => Transform::Normal,
        "90" => Transform::_90,
        "180" => Transform::_180,
        "270" => Transform::_270,
        "flipped" => Transform::Flipped,
        "flipped-90" => Transform::Flipped90,
        "flipped-180" => Transform::Flipped180,
        "flipped-270" => Transform::Flipped270,
        other => {
            tracing::warn!("output transform unknown: {:?} — using Normal", other);
            Transform::Normal
        }
    }
}

pub fn detect_output_reload_diff(
    prev: Option<&OutputEntry>,
    next: Option<&OutputEntry>,
) -> OutputReloadDiff {
    let prev_mode = prev.and_then(|entry| entry.mode.as_ref());
    let next_mode = next.and_then(|entry| entry.mode.as_ref());
    let mode_changed = match (prev_mode, next_mode) {
        (None, None) => false,
        (Some(a), Some(b)) => a != b,
        _ => true,
    };

    let prev_enabled = prev.map(|entry| entry.enabled).unwrap_or(true);
    let next_enabled = next.map(|entry| entry.enabled).unwrap_or(true);
    let enabled_changed = prev_enabled != next_enabled;

    OutputReloadDiff {
        mode_changed,
        enabled_changed,
    }
}

#[cfg(test)]
#[path = "output_layout_tests.rs"]
mod tests;
