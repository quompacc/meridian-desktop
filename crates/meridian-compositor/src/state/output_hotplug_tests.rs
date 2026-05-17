//! Headless hotplug regression tests for the output-layer stack.
//!
//! These tests exercise the integrated flow of `OutputLayout` →
//! `OutputRegistry` → `WorkspaceOutputState` without spinning up the
//! actual DRM/Wayland backend. They guard the data-flow between these
//! layers; they do not cover:
//!
//! - Smithay `Space` window remapping (window-tracking on output remove)
//! - DRM mode selection or compositor lifecycle
//! - Mirror-Mode (P2)
//!
//! When a phase touches the output layer, add a snapshot-style case here
//! BEFORE changing behavior, so regressions are explicit.

use meridian_config::{OutputEntry, OutputModeConfig, OutputPositionConfig};
use smithay::utils::Transform;

use crate::state::{
    ConnectedOutput, MeridianState, OutputGeometry, OutputId, OutputLayout, OutputReconfigure,
    OutputRegistration, OutputRegistry, WorkspaceOutputState,
};

const TEST_WORKSPACE_COUNT: usize = 9;

struct OutputHotplugFixture {
    layout: OutputLayout,
    registry: OutputRegistry,
    pending_disabled: Vec<ConnectedOutput>,
    workspaces: WorkspaceOutputState,
    global_active_workspace: usize,
}

impl OutputHotplugFixture {
    fn new() -> Self {
        Self {
            layout: OutputLayout::default(),
            registry: OutputRegistry::new(),
            pending_disabled: Vec::new(),
            workspaces: WorkspaceOutputState::default(),
            global_active_workspace: 0,
        }
    }

    fn with_layout_from_entries(entries: &[OutputEntry]) -> Self {
        Self {
            layout: OutputLayout::from_config_entries(entries),
            registry: OutputRegistry::new(),
            pending_disabled: Vec::new(),
            workspaces: WorkspaceOutputState::default(),
            global_active_workspace: 0,
        }
    }

    fn add_output(&mut self, name: &str, width: i32, height: i32) -> Option<OutputId> {
        let mut connected: Vec<ConnectedOutput> = self
            .registry
            .list()
            .iter()
            .map(|info| ConnectedOutput {
                name: info.name.clone(),
                width: info.geometry.width,
                height: info.geometry.height,
            })
            .collect();
        connected.push(ConnectedOutput {
            name: name.to_string(),
            width,
            height,
        });

        let mut resolved = self.layout.resolve(&connected);
        MeridianState::enforce_at_least_one_enabled(&mut resolved);

        let target = resolved.iter().find(|output| output.name == name)?;
        if !target.enabled {
            return None;
        }

        let id = self.registry.upsert(OutputRegistration {
            name: target.name.clone(),
            geometry: OutputGeometry {
                x: target.x,
                y: target.y,
                width: target.width,
                height: target.height,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
        });

        for output in &resolved {
            let _ = self.registry.reconfigure_by_name(
                &output.name,
                OutputReconfigure {
                    geometry: OutputGeometry {
                        x: output.x,
                        y: output.y,
                        width: output.width,
                        height: output.height,
                    },
                    scale: 1.0,
                    transform: Transform::Normal,
                    refresh_millihz: Some(60_000),
                    primary: Some(output.primary),
                },
            );
        }

        self.workspaces.sync_outputs_with_workspace_state(
            &self.registry,
            self.global_active_workspace,
            TEST_WORKSPACE_COUNT,
        );
        Some(id)
    }

    fn remove_output(&mut self, name: &str) -> bool {
        let removed = self.registry.remove_by_name(name).is_some();
        if removed {
            self.workspaces.sync_outputs_with_workspace_state(
                &self.registry,
                self.global_active_workspace,
                TEST_WORKSPACE_COUNT,
            );
        }
        removed
    }

    fn reconfigure_output(&mut self, name: &str, width: i32, height: i32) -> bool {
        if self.registry.by_name(name).is_none() {
            return false;
        }

        let connected: Vec<ConnectedOutput> = self
            .registry
            .list()
            .iter()
            .map(|info| {
                if info.name == name {
                    ConnectedOutput {
                        name: info.name.clone(),
                        width,
                        height,
                    }
                } else {
                    ConnectedOutput {
                        name: info.name.clone(),
                        width: info.geometry.width,
                        height: info.geometry.height,
                    }
                }
            })
            .collect();

        let mut resolved = self.layout.resolve(&connected);
        MeridianState::enforce_at_least_one_enabled(&mut resolved);

        for output in &resolved {
            let _ = self.registry.reconfigure_by_name(
                &output.name,
                OutputReconfigure {
                    geometry: OutputGeometry {
                        x: output.x,
                        y: output.y,
                        width: output.width,
                        height: output.height,
                    },
                    scale: 1.0,
                    transform: Transform::Normal,
                    refresh_millihz: Some(60_000),
                    primary: Some(output.primary),
                },
            );
        }

        self.workspaces.sync_outputs_with_workspace_state(
            &self.registry,
            self.global_active_workspace,
            TEST_WORKSPACE_COUNT,
        );
        true
    }

    fn reload_layout_from_entries(&mut self, new_entries: &[OutputEntry]) {
        self.layout = OutputLayout::from_config_entries(new_entries);
        let mut connected: Vec<ConnectedOutput> = self
            .registry
            .list()
            .iter()
            .map(|info| ConnectedOutput {
                name: info.name.clone(),
                width: info.geometry.width,
                height: info.geometry.height,
            })
            .collect();
        let extra_pending = self
            .pending_disabled
            .iter()
            .filter(|pending| !connected.iter().any(|known| known.name == pending.name))
            .cloned()
            .collect::<Vec<_>>();
        connected.extend(extra_pending);
        if connected.is_empty() {
            return;
        }

        let mut resolved = self.layout.resolve(&connected);
        MeridianState::enforce_at_least_one_enabled(&mut resolved);
        for output in &resolved {
            if !output.enabled {
                let _ = self.simulate_disable_output(&output.name);
                if !self
                    .pending_disabled
                    .iter()
                    .any(|pending| pending.name == output.name)
                {
                    self.pending_disabled.push(ConnectedOutput {
                        name: output.name.clone(),
                        width: output.width,
                        height: output.height,
                    });
                }
                continue;
            }

            self.pending_disabled
                .retain(|pending| pending.name != output.name);
            if self.registry.by_name(&output.name).is_none() {
                let _ = self.registry.upsert(OutputRegistration {
                    name: output.name.clone(),
                    geometry: OutputGeometry {
                        x: output.x,
                        y: output.y,
                        width: output.width,
                        height: output.height,
                    },
                    scale: 1.0,
                    transform: Transform::Normal,
                    refresh_millihz: Some(60_000),
                });
            }
            let _ = self.registry.reconfigure_by_name(
                &output.name,
                OutputReconfigure {
                    geometry: OutputGeometry {
                        x: output.x,
                        y: output.y,
                        width: output.width,
                        height: output.height,
                    },
                    scale: 1.0,
                    transform: Transform::Normal,
                    refresh_millihz: Some(60_000),
                    primary: Some(output.primary),
                },
            );
        }
        self.workspaces.sync_outputs_with_workspace_state(
            &self.registry,
            self.global_active_workspace,
            TEST_WORKSPACE_COUNT,
        );
    }

    fn simulate_disable_output(&mut self, name: &str) -> bool {
        let Some(existing) = self.registry.by_name(name).cloned() else {
            return false;
        };
        let removed = self.registry.remove_by_name(name).is_some();
        if removed {
            self.pending_disabled.retain(|pending| pending.name != name);
            self.pending_disabled.push(ConnectedOutput {
                name: existing.name,
                width: existing.geometry.width,
                height: existing.geometry.height,
            });
            self.workspaces.sync_outputs_with_workspace_state(
                &self.registry,
                self.global_active_workspace,
                TEST_WORKSPACE_COUNT,
            );
        }
        removed
    }

    fn snapshot(&self) -> String {
        let mut lines = Vec::new();
        for info in self.registry.list() {
            lines.push(format!(
                "{}: ({},{} {}x{}) primary={} workspace={}",
                info.name,
                info.geometry.x,
                info.geometry.y,
                info.geometry.width,
                info.geometry.height,
                info.primary,
                self.workspaces.active_workspace_for_output(
                    Some(info.id),
                    &self.registry,
                    self.global_active_workspace
                ),
            ));
        }
        lines.join("\n")
    }
}

#[test]
fn single_output_add_yields_zero_origin_primary() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0"#
    );
}

#[test]
fn two_outputs_default_layout_chains_horizontally() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 2560, 1440).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (1920,0 2560x1440) primary=false workspace=0"#
    );
}

#[test]
fn remove_primary_falls_back_to_remaining() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 2560, 1440).is_some());
    assert!(fixture.remove_output("drm-0"));
    assert_eq!(
        fixture.snapshot(),
        r#"drm-1: (1920,0 2560x1440) primary=true workspace=0"#
    );
}

#[test]
fn add_remove_add_is_idempotent() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    assert!(fixture.remove_output("drm-0"));
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-1: (0,0 1920x1080) primary=true workspace=0
drm-0: (1920,0 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn remove_unknown_output_is_safe_noop() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(!fixture.remove_output("does-not-exist"));
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0"#
    );
}

#[test]
fn reconfigure_changes_width_keeps_chain_after() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    assert!(fixture.reconfigure_output("drm-0", 2560, 1440));
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 2560x1440) primary=true workspace=0
drm-1: (2560,0 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn reconfigure_unknown_output_is_safe_noop() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(!fixture.reconfigure_output("does-not-exist", 100, 100));
}

#[test]
fn reconfigure_same_size_is_stable() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    let before = fixture.snapshot();
    assert!(fixture.reconfigure_output("drm-0", 1920, 1080));
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn layout_with_explicit_primary_overrides_first_default() {
    let entries = vec![entry_with("drm-1", OutputPositionConfig::Auto, true, true)];
    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&entries);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=false workspace=0
drm-1: (1920,0 1920x1080) primary=true workspace=0"#
    );
}

#[test]
fn layout_right_of_chain_three_outputs() {
    let entries = vec![
        entry_with(
            "drm-1",
            OutputPositionConfig::RightOf("drm-0".to_string()),
            false,
            true,
        ),
        entry_with(
            "drm-2",
            OutputPositionConfig::RightOf("drm-1".to_string()),
            false,
            true,
        ),
    ];
    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&entries);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-2", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (1920,0 1920x1080) primary=false workspace=0
drm-2: (3840,0 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn layout_below_chain_two_outputs() {
    let entries = vec![entry_with(
        "drm-1",
        OutputPositionConfig::Below("drm-0".to_string()),
        false,
        true,
    )];
    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&entries);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (0,1080 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn layout_coord_position_pins_exact_xy() {
    let entries = vec![entry_with(
        "drm-1",
        OutputPositionConfig::Coord { x: 500, y: 300 },
        false,
        true,
    )];
    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&entries);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (500,300 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn layout_dangling_reference_falls_back_to_auto() {
    let entries = vec![entry_with(
        "drm-1",
        OutputPositionConfig::RightOf("UNKNOWN".to_string()),
        false,
        true,
    )];
    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&entries);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (1920,0 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn safety_net_triggers_when_all_outputs_disabled_in_layout() {
    let entries = vec![entry_with(
        "drm-0",
        OutputPositionConfig::Auto,
        false,
        false,
    )];
    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&entries);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0"#
    );
}

#[test]
fn cyclic_add_remove_stability() {
    let mut fixture = OutputHotplugFixture::new();
    for _ in 0..5 {
        assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
        assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
        assert!(fixture.remove_output("drm-1"));
        assert!(fixture.remove_output("drm-0"));
        assert!(fixture.registry.list().is_empty());
        assert!(fixture
            .workspaces
            .focused_output(&fixture.registry)
            .is_none());
        assert!(!fixture
            .workspaces
            .has_stale_focused_output(&fixture.registry));
    }
}

#[test]
fn four_outputs_complex_layout_snapshot() {
    let entries = vec![
        entry_with(
            "drm-1",
            OutputPositionConfig::RightOf("drm-0".to_string()),
            false,
            true,
        ),
        entry_with(
            "drm-2",
            OutputPositionConfig::Below("drm-0".to_string()),
            false,
            true,
        ),
        entry_with(
            "drm-3",
            OutputPositionConfig::RightOf("drm-2".to_string()),
            false,
            true,
        ),
    ];
    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&entries);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-2", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-3", 1920, 1080).is_some());
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (1920,0 1920x1080) primary=false workspace=0
drm-2: (0,1080 1920x1080) primary=false workspace=0
drm-3: (1920,1080 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn reload_layout_changes_primary_assignment() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-1",
        OutputPositionConfig::Auto,
        true,
        true,
    )]);
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=false workspace=0
drm-1: (1920,0 1920x1080) primary=true workspace=0"#
    );
}

#[test]
fn reload_layout_repositions_output_via_below() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-1",
        OutputPositionConfig::Below("drm-0".to_string()),
        false,
        true,
    )]);
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (0,1080 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn reload_layout_coord_pin_overrides_chain() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-1",
        OutputPositionConfig::Coord { x: 500, y: 300 },
        false,
        true,
    )]);
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (500,300 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn reload_layout_empty_entries_falls_back_to_auto_chain() {
    let entries = vec![entry_with(
        "drm-1",
        OutputPositionConfig::Coord { x: 500, y: 300 },
        false,
        true,
    )];
    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&entries);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    fixture.reload_layout_from_entries(&[]);
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (1920,0 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn reload_layout_safety_net_re_enables_all_disabled() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-0",
        OutputPositionConfig::Auto,
        false,
        false,
    )]);
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0"#
    );
}

#[test]
fn reload_layout_noop_when_registry_empty() {
    let mut fixture = OutputHotplugFixture::new();
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-0",
        OutputPositionConfig::Coord { x: 120, y: 80 },
        false,
        true,
    )]);
    assert_eq!(fixture.snapshot(), "");
}

#[test]
fn reload_layout_with_mode_change_safely_logs_only_in_harness() {
    let mut initial = entry_with("drm-0", OutputPositionConfig::Auto, true, true);
    initial.mode = Some(OutputModeConfig {
        width: 1920,
        height: 1080,
        refresh_millihz: Some(60_000),
    });
    let mut updated = entry_with("drm-0", OutputPositionConfig::Auto, true, true);
    updated.mode = Some(OutputModeConfig {
        width: 2560,
        height: 1440,
        refresh_millihz: Some(60_000),
    });

    let mut fixture = OutputHotplugFixture::with_layout_from_entries(&[initial]);
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    let before = fixture.snapshot();
    fixture.reload_layout_from_entries(&[updated]);
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn reload_with_enabled_false_simulates_disable_via_registry() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-1",
        OutputPositionConfig::Auto,
        false,
        false,
    )]);
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0"#
    );
}

#[test]
fn reload_re_enabling_output_re_adds_to_registry() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-1",
        OutputPositionConfig::Auto,
        false,
        false,
    )]);
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-1",
        OutputPositionConfig::Auto,
        false,
        true,
    )]);
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0
drm-1: (1920,0 1920x1080) primary=false workspace=0"#
    );
}

#[test]
fn reload_safety_net_re_enables_all_disabled_via_harness() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    fixture.reload_layout_from_entries(&[entry_with(
        "drm-0",
        OutputPositionConfig::Auto,
        false,
        false,
    )]);
    assert_eq!(
        fixture.snapshot(),
        r#"drm-0: (0,0 1920x1080) primary=true workspace=0"#
    );
}

fn entry_with(
    name: &str,
    position: OutputPositionConfig,
    primary: bool,
    enabled: bool,
) -> OutputEntry {
    let mut entry = OutputEntry::defaults_for(name);
    entry.position = position;
    entry.primary = primary;
    entry.enabled = enabled;
    entry
}
