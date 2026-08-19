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
//! Covered in Phase 1+2: window-position survival for Smithay `Space` on
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

    fn move_window_between_workspaces(&mut self, name: &str, from: usize, to: usize) -> bool {
        if from == to || from >= self.spaces.len() || to >= self.spaces.len() {
            return false;
        }
        let Some(elem) = self.spaces[from]
            .elements()
            .find(|e| e.name == name)
            .cloned()
        else {
            return false;
        };
        let loc = self.spaces[from]
            .element_location(&elem)
            .unwrap_or_default();
        self.spaces[from].unmap_elem(&elem);
        self.spaces[to].map_element(elem, (loc.x, loc.y), false);
        self.spaces[from].refresh();
        self.spaces[to].refresh();
        true
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

include!("output_hotplug_tests/basic.rs");
include!("output_hotplug_tests/reload.rs");
include!("output_hotplug_tests/windows.rs");
