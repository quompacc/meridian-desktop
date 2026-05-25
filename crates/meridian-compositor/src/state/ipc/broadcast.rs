use std::collections::{HashMap, HashSet};

use meridian_ipc::{OutputModeState, OutputWorkspaceState, ShellEvent, WindowSnapshotEntry};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::seat::WaylandFocus;

use super::conversions::index_to_legacy_ipc_workspace;
use crate::state::{
    window_app_id, window_id, window_list_entry, MeridianState, OutputId, OutputInfo,
    OutputModeInfo,
};

fn build_output_workspace_snapshot(
    outputs: &[OutputInfo],
    focused_output: Option<OutputId>,
    mut active_workspace_for_output: impl FnMut(OutputId) -> usize,
    mut modes_for_output: impl FnMut(OutputId) -> Vec<OutputModeInfo>,
) -> (Option<u32>, Vec<OutputWorkspaceState>) {
    let focused_output_id = focused_output
        .filter(|id| outputs.iter().any(|output| output.id == *id))
        .map(|id| id.0);

    let mut states = Vec::with_capacity(outputs.len());
    for output in outputs {
        states.push(OutputWorkspaceState {
            output_id: output.id.0,
            output_name: Some(output.name.clone()),
            active_workspace: active_workspace_for_output(output.id).saturating_add(1),
            primary: output.primary,
            focused: focused_output_id == Some(output.id.0),
            x: output.geometry.x,
            y: output.geometry.y,
            width: output.geometry.width,
            height: output.geometry.height,
            scale_millis: (output.scale * 1000.0).round().clamp(1.0, u32::MAX as f64) as u32,
            transform: Some(format!("{:?}", output.transform)),
            refresh_millihz: output.refresh_millihz,
            modes: modes_for_output(output.id)
                .into_iter()
                .map(|mode| OutputModeState {
                    width: mode.width,
                    height: mode.height,
                    refresh_millihz: mode.refresh_millihz,
                    current: mode.current,
                    preferred: mode.preferred,
                })
                .collect(),
        });
    }

    (focused_output_id, states)
}

impl MeridianState {
    pub fn broadcast_workspace(&mut self) {
        self.ipc.broadcast(&ShellEvent::WorkspaceChanged {
            workspace: index_to_legacy_ipc_workspace(self.workspaces.active),
        });
    }

    pub fn broadcast_window_snapshot(&mut self) {
        let mut windows = Vec::new();
        let mut seen_ids = HashSet::new();
        for idx in 0..self.workspaces.count() {
            for window in self.workspaces.space_at(idx).elements() {
                let Some((id, title)) = window_list_entry(window) else {
                    continue;
                };
                if !seen_ids.insert(id.clone()) {
                    continue;
                }
                windows.push(WindowSnapshotEntry {
                    workspace: index_to_legacy_ipc_workspace(idx),
                    id,
                    title,
                    minimized: false,
                    app_id: window_app_id(window),
                });
            }
        }
        for (id, minimized) in &self.minimized_windows {
            if seen_ids.contains(id) {
                continue;
            }
            let Some((_, title)) = window_list_entry(&minimized.window) else {
                continue;
            };
            windows.push(WindowSnapshotEntry {
                workspace: index_to_legacy_ipc_workspace(minimized.workspace),
                id: id.clone(),
                title,
                minimized: true,
                app_id: window_app_id(&minimized.window),
            });
        }

        tracing::debug!(
            "workspace/window snapshot broadcasted: active_workspace={} windows={}",
            self.workspaces.active + 1,
            windows.len()
        );
        self.ipc.broadcast(&ShellEvent::WindowSnapshot {
            active_workspace: index_to_legacy_ipc_workspace(self.workspaces.active),
            windows,
        });
        self.broadcast_output_workspace_snapshot();
    }

    pub fn broadcast_output_workspace_changed(&mut self, output_id: u32, workspace: usize) {
        let id = OutputId(output_id);
        let output_name = self.output_registry.by_id(id).map(|info| info.name.clone());
        let focused = self.focused_output().is_some_and(|focused| focused == id);
        tracing::debug!(
            "output workspace changed broadcasted: output_id={} output_name={:?} workspace={} focused={}",
            output_id,
            output_name,
            workspace + 1,
            focused
        );
        self.ipc.broadcast(&ShellEvent::OutputWorkspaceChanged {
            output_id,
            output_name,
            workspace: workspace.saturating_add(1),
            focused,
        });
    }

    pub fn broadcast_output_workspace_snapshot(&mut self) {
        let output_modes = self
            .output_registry
            .list()
            .iter()
            .map(|output| {
                (
                    output.id,
                    self.output_registry.modes_for_id(output.id).to_vec(),
                )
            })
            .collect::<HashMap<_, _>>();
        let (focused_output_id, outputs) = build_output_workspace_snapshot(
            self.output_registry.list(),
            self.focused_output(),
            |output_id| self.active_workspace_for_output(Some(output_id)),
            |output_id| output_modes.get(&output_id).cloned().unwrap_or_default(),
        );
        tracing::debug!(
            "output workspace snapshot broadcasted: focused_output_id={:?} outputs={}",
            focused_output_id,
            outputs.len()
        );
        self.ipc.broadcast(&ShellEvent::OutputWorkspaceSnapshot {
            focused_output_id,
            outputs,
        });
    }

    pub fn broadcast_toplevel_opened(
        &mut self,
        surface: &smithay::wayland::shell::xdg::ToplevelSurface,
    ) {
        self.broadcast_window_opened(
            window_id(surface.wl_surface()),
            super::super::toplevel_title(surface),
        );
    }

    pub fn broadcast_window_opened(&mut self, id: String, title: String) {
        self.ipc.broadcast(&ShellEvent::WindowOpened { id, title });
    }

    pub fn broadcast_toplevel_closed(
        &mut self,
        surface: &smithay::wayland::shell::xdg::ToplevelSurface,
    ) {
        self.broadcast_window_closed(window_id(surface.wl_surface()));
    }

    pub fn broadcast_window_closed(&mut self, id: String) {
        self.ipc.broadcast(&ShellEvent::WindowClosed { id });
    }

    pub fn broadcast_toplevel_focused(&mut self, surface: &WlSurface) {
        let idx = self.current_workspace_index();
        let focused_id = self.workspaces.space_at(idx).elements().find_map(|window| {
            let matches_focus = window
                .wl_surface()
                .is_some_and(|window_surface| window_surface.as_ref() == surface);
            if !matches_focus {
                return None;
            }
            window_list_entry(window).map(|(id, _)| id)
        });

        self.ipc.broadcast(&ShellEvent::WindowFocused {
            id: focused_id.unwrap_or_else(|| window_id(surface)),
        });
    }

    pub fn broadcast_toplevel_focus_cleared(&mut self) {
        self.ipc.broadcast(&ShellEvent::WindowFocusCleared);
    }

    pub fn broadcast_toggle_launcher(&mut self) {
        self.ipc.broadcast(&ShellEvent::ToggleLauncher);
    }

    pub fn broadcast_desktop_context_menu(&mut self, x: i32, y: i32) {
        self.ipc.broadcast(&ShellEvent::DesktopContextMenu { x, y });
    }
}

#[cfg(test)]
mod tests {
    use smithay::utils::Transform;

    use crate::state::{OutputGeometry, OutputRegistration, OutputRegistry};

    use super::{build_output_workspace_snapshot, OutputId};

    fn reg(name: &str, x: i32, y: i32) -> OutputRegistration {
        OutputRegistration {
            name: name.to_string(),
            geometry: OutputGeometry {
                x,
                y,
                width: 1920,
                height: 1080,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
        }
    }

    #[test]
    fn output_workspace_snapshot_for_two_outputs_sets_flags() {
        let mut registry = OutputRegistry::new();
        let left = registry.upsert(reg("eDP-1", 0, 0));
        let right = registry.upsert(reg("HDMI-A-1", 1920, 0));

        let (focused_output_id, outputs) = build_output_workspace_snapshot(
            registry.list(),
            Some(right),
            |id| {
                if id == left {
                    1
                } else {
                    3
                }
            },
            |id| registry.modes_for_id(id).to_vec(),
        );

        assert_eq!(focused_output_id, Some(right.0));
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].output_id, left.0);
        assert_eq!(outputs[0].output_name.as_deref(), Some("eDP-1"));
        assert!(outputs[0].primary);
        assert!(!outputs[0].focused);
        assert_eq!(outputs[0].active_workspace, 2);
        assert_eq!(outputs[0].x, 0);
        assert_eq!(outputs[0].y, 0);
        assert_eq!(outputs[0].width, 1920);
        assert_eq!(outputs[0].height, 1080);
        assert_eq!(outputs[0].scale_millis, 1000);
        assert_eq!(outputs[0].transform.as_deref(), Some("Normal"));
        assert_eq!(outputs[0].refresh_millihz, Some(60_000));
        assert!(outputs[0].modes.is_empty());

        assert_eq!(outputs[1].output_id, right.0);
        assert_eq!(outputs[1].output_name.as_deref(), Some("HDMI-A-1"));
        assert!(!outputs[1].primary);
        assert!(outputs[1].focused);
        assert_eq!(outputs[1].active_workspace, 4);
        assert_eq!(outputs[1].x, 1920);
    }

    #[test]
    fn output_workspace_snapshot_empty_registry_is_safe() {
        let registry = OutputRegistry::new();
        let (focused_output_id, outputs) = build_output_workspace_snapshot(
            registry.list(),
            Some(OutputId(1)),
            |_| 0,
            |id| registry.modes_for_id(id).to_vec(),
        );
        assert_eq!(focused_output_id, None);
        assert!(outputs.is_empty());
    }
}
