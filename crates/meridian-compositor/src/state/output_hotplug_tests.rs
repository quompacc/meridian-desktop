//! Headless hotplug regression tests for the output-layer stack.
//!
//! These tests exercise the integrated flow of `OutputLayout` →
//! `OutputRegistry` → `WorkspaceOutputState` without spinning up the
//! actual DRM/Wayland backend. They guard the data-flow between these
//! layers; they do not cover:
//!
//! - Window migration as behavior change (Phase 3)
//! - DRM mode selection or compositor lifecycle
//! - Mirror-Mode (P2)
//!
//! Covered in Phase 1: window-position survival for Smithay `Space` on
//! output remove/reconfigure (no automatic migration).
//!
//! When a phase touches the output layer, add a snapshot-style case here
//! BEFORE changing behavior, so regressions are explicit.

use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    hash::{Hash, Hasher},
    rc::Rc,
};

use meridian_config::{OutputEntry, OutputModeConfig, OutputPositionConfig};
use smithay::{
    desktop::{space::SpaceElement, Space},
    output::{Mode as OutputMode, Output, PhysicalProperties, Scale as OutputScale, Subpixel},
    utils::{IsAlive, Logical, Point, Rectangle, Transform},
};

use crate::state::{
    ConnectedOutput, MeridianState, OutputGeometry, OutputId, OutputLayout, OutputReconfigure,
    OutputRegistration, OutputRegistry, WorkspaceOutputState,
};

const TEST_WORKSPACE_COUNT: usize = 9;

#[derive(Debug, Clone)]
struct TestSpaceElement {
    name: String,
    bbox: Rectangle<i32, Logical>,
    entered_outputs: Rc<RefCell<BTreeSet<String>>>,
    alive: Rc<RefCell<bool>>,
}

impl TestSpaceElement {
    fn new(name: &str, x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            name: name.to_string(),
            bbox: Rectangle::new((x, y).into(), (w, h).into()),
            entered_outputs: Rc::new(RefCell::new(BTreeSet::new())),
            alive: Rc::new(RefCell::new(true)),
        }
    }

    fn entered(&self) -> Vec<String> {
        self.entered_outputs.borrow().iter().cloned().collect()
    }
}

impl PartialEq for TestSpaceElement {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for TestSpaceElement {}

impl Hash for TestSpaceElement {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl IsAlive for TestSpaceElement {
    fn alive(&self) -> bool {
        *self.alive.borrow()
    }
}

impl SpaceElement for TestSpaceElement {
    fn bbox(&self) -> Rectangle<i32, Logical> {
        self.bbox
    }

    fn is_in_input_region(&self, _point: &Point<f64, Logical>) -> bool {
        false
    }

    fn set_activate(&self, _activated: bool) {}

    fn output_enter(&self, output: &Output, _overlap: Rectangle<i32, Logical>) {
        self.entered_outputs.borrow_mut().insert(output.name());
    }

    fn output_leave(&self, output: &Output) {
        self.entered_outputs.borrow_mut().remove(&output.name());
    }
}

fn make_output(name: &str, width: i32, height: i32) -> Output {
    let output = Output::new(
        name.to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Test".into(),
            model: "Test".into(),
            serial_number: "Test".into(),
        },
    );
    output.change_current_state(
        Some(OutputMode {
            size: (width, height).into(),
            refresh: 60_000,
        }),
        Some(Transform::Normal),
        Some(OutputScale::Integer(1)),
        Some((0, 0).into()),
    );
    output
}

struct OutputHotplugFixture {
    layout: OutputLayout,
    registry: OutputRegistry,
    pending_disabled: Vec<ConnectedOutput>,
    workspaces: WorkspaceOutputState,
    global_active_workspace: usize,
    spaces: Vec<Space<TestSpaceElement>>,
    smithay_outputs: HashMap<String, Output>,
}

impl OutputHotplugFixture {
    fn new() -> Self {
        Self {
            layout: OutputLayout::default(),
            registry: OutputRegistry::new(),
            pending_disabled: Vec::new(),
            workspaces: WorkspaceOutputState::default(),
            global_active_workspace: 0,
            spaces: (0..TEST_WORKSPACE_COUNT)
                .map(|_| Space::default())
                .collect(),
            smithay_outputs: HashMap::new(),
        }
    }

    fn with_layout_from_entries(entries: &[OutputEntry]) -> Self {
        Self {
            layout: OutputLayout::from_config_entries(entries),
            registry: OutputRegistry::new(),
            pending_disabled: Vec::new(),
            workspaces: WorkspaceOutputState::default(),
            global_active_workspace: 0,
            spaces: (0..TEST_WORKSPACE_COUNT)
                .map(|_| Space::default())
                .collect(),
            smithay_outputs: HashMap::new(),
        }
    }

    fn sync_smithay_output_state(&mut self, name: &str, width: i32, height: i32, x: i32, y: i32) {
        let Some(output) = self.smithay_outputs.get(name).cloned() else {
            return;
        };

        if output.current_mode().map(|mode| (mode.size.w, mode.size.h)) != Some((width, height)) {
            output.change_current_state(
                Some(OutputMode {
                    size: (width, height).into(),
                    refresh: 60_000,
                }),
                None,
                None,
                None,
            );
        }
        output.change_current_state(None, None, None, Some((x, y).into()));
        for space in &mut self.spaces {
            space.map_output(&output, (x, y));
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

        self.smithay_outputs
            .entry(target.name.clone())
            .or_insert_with(|| make_output(&target.name, target.width, target.height));

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
            self.sync_smithay_output_state(
                &output.name,
                output.width,
                output.height,
                output.x,
                output.y,
            );
        }

        self.workspaces.sync_outputs_with_workspace_state(
            &self.registry,
            self.global_active_workspace,
            TEST_WORKSPACE_COUNT,
        );
        self.refresh_all_spaces();
        Some(id)
    }

    fn remove_output(&mut self, name: &str) -> bool {
        if let Some(output) = self.smithay_outputs.remove(name) {
            for space in &mut self.spaces {
                space.unmap_output(&output);
            }
        }

        let removed = self.registry.remove_by_name(name).is_some();
        if removed {
            self.workspaces.sync_outputs_with_workspace_state(
                &self.registry,
                self.global_active_workspace,
                TEST_WORKSPACE_COUNT,
            );
            self.refresh_all_spaces();
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
            self.sync_smithay_output_state(
                &output.name,
                output.width,
                output.height,
                output.x,
                output.y,
            );
        }

        self.workspaces.sync_outputs_with_workspace_state(
            &self.registry,
            self.global_active_workspace,
            TEST_WORKSPACE_COUNT,
        );
        self.refresh_all_spaces();
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
                self.smithay_outputs.insert(
                    output.name.clone(),
                    make_output(&output.name, output.width, output.height),
                );
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
            self.sync_smithay_output_state(
                &output.name,
                output.width,
                output.height,
                output.x,
                output.y,
            );
        }
        self.workspaces.sync_outputs_with_workspace_state(
            &self.registry,
            self.global_active_workspace,
            TEST_WORKSPACE_COUNT,
        );
        self.refresh_all_spaces();
    }

    fn simulate_disable_output(&mut self, name: &str) -> bool {
        if let Some(output) = self.smithay_outputs.remove(name) {
            for space in &mut self.spaces {
                space.unmap_output(&output);
            }
        }

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

    fn map_window(
        &mut self,
        name: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        workspace: usize,
    ) -> TestSpaceElement {
        let elem = TestSpaceElement::new(name, x, y, w, h);
        self.spaces[workspace].map_element(elem.clone(), (x, y), false);
        self.spaces[workspace].refresh();
        elem
    }

    fn window_location(&self, name: &str, workspace: usize) -> Option<(i32, i32)> {
        self.spaces[workspace]
            .elements()
            .find(|e| e.name == name)
            .and_then(|e| self.spaces[workspace].element_location(e))
            .map(|p| (p.x, p.y))
    }

    fn window_count(&self, workspace: usize) -> usize {
        self.spaces[workspace].elements().count()
    }

    fn refresh_all_spaces(&mut self) {
        for space in &mut self.spaces {
            space.refresh();
        }
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

#[test]
fn window_on_removed_output_stays_at_logical_position() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());

    let w1 = fixture.map_window("w1", 2000, 200, 300, 300, 0);
    assert_eq!(fixture.window_count(0), 1);
    assert_eq!(fixture.window_location("w1", 0), Some((2000, 200)));

    assert!(fixture.remove_output("drm-1"));
    fixture.refresh_all_spaces();

    assert_eq!(fixture.window_location("w1", 0), Some((2000, 200)));
    assert!(w1.entered().is_empty());
}

#[test]
fn window_on_remaining_output_undisturbed_by_sibling_removal() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());

    let w1 = fixture.map_window("w1", 200, 200, 300, 300, 0);
    assert_eq!(w1.entered(), vec!["drm-0".to_string()]);

    assert!(fixture.remove_output("drm-1"));
    fixture.refresh_all_spaces();

    assert_eq!(fixture.window_location("w1", 0), Some((200, 200)));
    assert_eq!(w1.entered(), vec!["drm-0".to_string()]);
}

#[test]
fn window_straddling_two_outputs_loses_one_on_removal() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());

    let w1 = fixture.map_window("w1", 1800, 100, 400, 400, 0);
    assert_eq!(w1.entered(), vec!["drm-0".to_string(), "drm-1".to_string()]);

    assert!(fixture.remove_output("drm-1"));
    fixture.refresh_all_spaces();

    assert_eq!(w1.entered(), vec!["drm-0".to_string()]);
    assert_eq!(fixture.window_location("w1", 0), Some((1800, 100)));
}

#[test]
fn reconfigure_geometry_keeps_window_position_updates_overlap() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());

    let w1 = fixture.map_window("w1", 1800, 100, 400, 400, 0);
    assert_eq!(w1.entered(), vec!["drm-0".to_string(), "drm-1".to_string()]);

    assert!(fixture.reconfigure_output("drm-0", 2400, 1080));
    fixture.refresh_all_spaces();

    assert_eq!(fixture.window_location("w1", 0), Some((1800, 100)));
    assert_eq!(w1.entered(), vec!["drm-0".to_string()]);
}

#[test]
fn cyclic_add_remove_add_does_not_leak_entered_outputs() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());

    let w1 = fixture.map_window("w1", 100, 100, 200, 200, 0);
    assert_eq!(w1.entered(), vec!["drm-0".to_string()]);

    for _ in 0..3 {
        assert!(fixture.add_output("drm-1", 1920, 1080).is_some());
        fixture.refresh_all_spaces();
        assert!(fixture.remove_output("drm-1"));
        fixture.refresh_all_spaces();
    }

    assert_eq!(w1.entered(), vec!["drm-0".to_string()]);
}

#[test]
fn multiple_windows_distributed_across_outputs_snapshot() {
    let mut fixture = OutputHotplugFixture::new();
    assert!(fixture.add_output("drm-0", 1920, 1080).is_some());
    assert!(fixture.add_output("drm-1", 1920, 1080).is_some());

    let w_left = fixture.map_window("w_left", 100, 100, 200, 200, 0);
    let w_right = fixture.map_window("w_right", 2100, 100, 200, 200, 0);
    fixture.refresh_all_spaces();

    assert_eq!(w_left.entered(), vec!["drm-0".to_string()]);
    assert_eq!(w_right.entered(), vec!["drm-1".to_string()]);

    assert!(fixture.remove_output("drm-0"));
    fixture.refresh_all_spaces();

    assert!(w_left.entered().is_empty());
    assert_eq!(w_right.entered(), vec!["drm-1".to_string()]);
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
