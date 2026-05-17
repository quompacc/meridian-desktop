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

impl OutputLayout {
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

#[cfg(test)]
mod tests {
    use super::{ConnectedOutput, OutputLayout, OutputPlacement, OutputPosition, ResolvedOutput};

    #[test]
    fn empty_layout_two_outputs_chains_horizontally() {
        let layout = OutputLayout::default();
        let connected = vec![
            connected("eDP-1", 1920, 1080),
            connected("HDMI-A-1", 2560, 1440),
        ];

        let resolved_outputs = layout.resolve(&connected);

        assert_eq!(
            resolved_outputs,
            vec![
                expected_output("eDP-1", 0, 0, 1920, 1080, true, true),
                expected_output("HDMI-A-1", 1920, 0, 2560, 1440, false, true),
            ]
        );
    }

    #[test]
    fn coord_position_places_at_exact_xy() {
        let layout = OutputLayout {
            placements: vec![placement(
                "HDMI-A-1",
                OutputPosition::Coord { x: 100, y: 200 },
                false,
                true,
            )],
        };
        let connected = vec![
            connected("eDP-1", 1920, 1080),
            connected("HDMI-A-1", 2560, 1440),
        ];

        let resolved = layout.resolve(&connected);

        assert_eq!(resolved[0].x, 0);
        assert_eq!(resolved[0].y, 0);
        assert_eq!(resolved[1].x, 100);
        assert_eq!(resolved[1].y, 200);
    }

    #[test]
    fn right_of_resolves_to_target_right_edge() {
        let layout = OutputLayout {
            placements: vec![placement(
                "HDMI-A-1",
                OutputPosition::RightOf("eDP-1".to_string()),
                false,
                true,
            )],
        };
        let connected = vec![
            connected("eDP-1", 1920, 1080),
            connected("HDMI-A-1", 2560, 1440),
        ];

        let resolved = layout.resolve(&connected);

        assert_eq!(resolved[1].x, 1920);
        assert_eq!(resolved[1].y, 0);
    }

    #[test]
    fn left_of_uses_self_width() {
        let layout = OutputLayout {
            placements: vec![
                placement(
                    "eDP-1",
                    OutputPosition::Coord { x: 2000, y: 0 },
                    false,
                    true,
                ),
                placement(
                    "HDMI-A-1",
                    OutputPosition::LeftOf("eDP-1".to_string()),
                    false,
                    true,
                ),
            ],
        };
        let connected = vec![
            connected("eDP-1", 1920, 1080),
            connected("HDMI-A-1", 1280, 720),
        ];

        let resolved = layout.resolve(&connected);

        assert_eq!(resolved[1].x, 720);
        assert_eq!(resolved[1].y, 0);
    }

    #[test]
    fn below_stacks_vertically() {
        let layout = OutputLayout {
            placements: vec![placement(
                "HDMI-A-1",
                OutputPosition::Below("eDP-1".to_string()),
                false,
                true,
            )],
        };
        let connected = vec![
            connected("eDP-1", 1920, 1080),
            connected("HDMI-A-1", 1920, 1080),
        ];

        let resolved = layout.resolve(&connected);

        assert_eq!(resolved[1].x, 0);
        assert_eq!(resolved[1].y, 1080);
    }

    #[test]
    fn above_uses_self_height() {
        let layout = OutputLayout {
            placements: vec![
                placement(
                    "eDP-1",
                    OutputPosition::Coord { x: 0, y: 1080 },
                    false,
                    true,
                ),
                placement(
                    "HDMI-A-1",
                    OutputPosition::Above("eDP-1".to_string()),
                    false,
                    true,
                ),
            ],
        };
        let connected = vec![
            connected("eDP-1", 1920, 1080),
            connected("HDMI-A-1", 1920, 720),
        ];

        let resolved = layout.resolve(&connected);

        assert_eq!(resolved[1].x, 0);
        assert_eq!(resolved[1].y, 360);
    }

    #[test]
    fn dangling_reference_falls_back_to_auto() {
        let layout = OutputLayout {
            placements: vec![placement(
                "HDMI-A-1",
                OutputPosition::RightOf("UNKNOWN".to_string()),
                false,
                true,
            )],
        };
        let connected = vec![
            connected("eDP-1", 1920, 1080),
            connected("HDMI-A-1", 1920, 1080),
        ];

        let resolved = layout.resolve(&connected);

        assert_eq!(resolved[1].x, 1920);
        assert_eq!(resolved[1].y, 0);
    }

    #[test]
    fn cycle_two_outputs_falls_back_to_auto_for_second() {
        let layout = OutputLayout {
            placements: vec![
                placement("A", OutputPosition::RightOf("B".to_string()), false, true),
                placement("B", OutputPosition::RightOf("A".to_string()), false, true),
            ],
        };
        let connected = vec![connected("A", 100, 100), connected("B", 100, 100)];

        let resolved = layout.resolve(&connected);

        assert_eq!(resolved[0].x, 0);
        assert_eq!(resolved[0].y, 0);
        assert_eq!(resolved[1].x, 100);
        assert_eq!(resolved[1].y, 0);
        assert_ne!(resolved[0].x, resolved[1].x);
    }

    #[test]
    fn disabled_output_is_skipped_in_chain() {
        let layout = OutputLayout {
            placements: vec![placement("HDMI-A-1", OutputPosition::Auto, false, false)],
        };
        let connected = vec![
            connected("eDP-1", 1920, 1080),
            connected("HDMI-A-1", 1920, 1080),
            connected("DP-1", 1920, 1080),
        ];

        let resolved = layout.resolve(&connected);

        assert_eq!(resolved[0].x, 0);
        assert!(resolved[0].enabled);
        assert_eq!(resolved[1].x, 0);
        assert_eq!(resolved[1].y, 0);
        assert!(!resolved[1].enabled);
        assert_eq!(resolved[2].x, 1920);
        assert!(resolved[2].enabled);
    }

    #[test]
    fn explicit_primary_overrides_first() {
        let layout = OutputLayout {
            placements: vec![placement("HDMI-A-1", OutputPosition::Auto, true, true)],
        };
        let connected = vec![
            connected("eDP-1", 1920, 1080),
            connected("HDMI-A-1", 1920, 1080),
        ];

        let resolved = layout.resolve(&connected);

        assert!(!resolved[0].primary);
        assert!(resolved[1].primary);
    }

    #[test]
    fn multiple_primary_keeps_first_in_placements() {
        let layout = OutputLayout {
            placements: vec![
                placement("eDP-1", OutputPosition::Auto, true, true),
                placement("HDMI-A-1", OutputPosition::Auto, true, true),
            ],
        };
        let connected = vec![
            connected("eDP-1", 1920, 1080),
            connected("HDMI-A-1", 1920, 1080),
        ];

        let resolved = layout.resolve(&connected);

        assert!(resolved[0].primary);
        assert!(!resolved[1].primary);
    }

    #[test]
    fn no_enabled_output_yields_no_primary() {
        let layout = OutputLayout {
            placements: vec![placement("eDP-1", OutputPosition::Auto, false, false)],
        };
        let connected = vec![connected("eDP-1", 1920, 1080)];

        let resolved = layout.resolve(&connected);

        assert!(!resolved[0].enabled);
        assert!(!resolved[0].primary);
    }

    #[test]
    fn connected_order_preserved_in_result() {
        let layout = OutputLayout::default();
        let connected = vec![
            connected("c", 100, 100),
            connected("a", 100, 100),
            connected("b", 100, 100),
        ];

        let resolved = layout.resolve(&connected);

        assert_eq!(
            resolved.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(),
            vec!["c", "a", "b"]
        );
        assert_eq!(resolved[0].x, 0);
        assert_eq!(resolved[1].x, 100);
        assert_eq!(resolved[2].x, 200);
    }

    #[test]
    fn placement_for_returns_existing_or_none() {
        let layout = OutputLayout {
            placements: vec![placement("eDP-1", OutputPosition::Auto, false, true)],
        };

        assert!(layout.placement_for("eDP-1").is_some());
        assert!(layout.placement_for("HDMI-A-1").is_none());
    }

    fn placement(
        name: &str,
        position: OutputPosition,
        primary: bool,
        enabled: bool,
    ) -> OutputPlacement {
        OutputPlacement {
            name: name.to_string(),
            position,
            primary,
            enabled,
        }
    }

    fn connected(name: &str, width: i32, height: i32) -> ConnectedOutput {
        ConnectedOutput {
            name: name.to_string(),
            width,
            height,
        }
    }

    fn expected_output(
        name: &str,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        primary: bool,
        enabled: bool,
    ) -> ResolvedOutput {
        ResolvedOutput {
            name: name.to_string(),
            x,
            y,
            width,
            height,
            primary,
            enabled,
        }
    }
}
