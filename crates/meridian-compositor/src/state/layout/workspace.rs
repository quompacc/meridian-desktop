use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, SERIAL_COUNTER},
};

use crate::state::{toplevel_title, window_id, MeridianState, OutputId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MoveRequestGuard {
    InvalidTarget,
    NoFocusedWindow,
    AlreadyOnTarget,
}

fn validate_workspace_move_request(
    target: usize,
    workspace_count: usize,
    source_workspace: Option<usize>,
) -> Result<usize, MoveRequestGuard> {
    if target >= workspace_count {
        return Err(MoveRequestGuard::InvalidTarget);
    }
    let Some(source_workspace) = source_workspace else {
        return Err(MoveRequestGuard::NoFocusedWindow);
    };
    if source_workspace == target {
        return Err(MoveRequestGuard::AlreadyOnTarget);
    }
    Ok(source_workspace)
}

impl MeridianState {
    pub fn current_workspace_index_for_focused_output(&self) -> usize {
        self.active_workspace_for_output(self.focused_output())
    }

    pub fn current_workspace_index(&self) -> usize {
        self.current_workspace_index_for_focused_output()
    }

    pub fn focused_output(&self) -> Option<OutputId> {
        self.workspace_output_state
            .focused_output(&self.output_registry)
    }

    pub fn set_focused_output(&mut self, output: Option<OutputId>) {
        let before = self.focused_output();
        if self
            .workspace_output_state
            .set_focused_output(output, &self.output_registry)
        {
            tracing::debug!(
                "focused output changed: old={:?} new={:?}",
                before,
                self.focused_output()
            );
        } else {
            tracing::debug!("focused output unchanged: current={:?}", before);
        }
    }

    pub fn active_workspace_for_output(&self, output: Option<OutputId>) -> usize {
        self.workspace_output_state.active_workspace_for_output(
            output,
            &self.output_registry,
            self.workspaces.active,
        )
    }

    pub fn set_active_workspace_for_output(&mut self, output: Option<OutputId>, workspace: usize) {
        if self.workspace_output_state.set_active_workspace_for_output(
            output,
            workspace,
            &self.output_registry,
            self.workspaces.count(),
        ) {
            let selected = output.or_else(|| self.focused_output());
            tracing::debug!(
                "active workspace mapping updated: output={:?} workspace={}",
                selected,
                workspace + 1
            );
        }
    }

    pub fn sync_outputs_with_workspace_state(&mut self) {
        let old_focus = self.workspace_output_state.raw_focused_output();
        let stale_before = self
            .workspace_output_state
            .has_stale_focused_output(&self.output_registry);
        if self
            .workspace_output_state
            .sync_outputs_with_workspace_state(
                &self.output_registry,
                self.workspaces.active,
                self.workspaces.count(),
            )
        {
            let new_focus = self.focused_output();
            if stale_before {
                tracing::debug!(
                    "focused output fallback after stale output: old={:?} new={:?}",
                    old_focus,
                    new_focus
                );
            } else {
                tracing::debug!(
                    "workspace output state synced: old_focused_output={:?} new_focused_output={:?}",
                    old_focus,
                    new_focus
                );
            }
        } else {
            tracing::debug!(
                "focused output unchanged: current={:?}",
                self.focused_output()
            );
        }
    }

    pub fn update_focused_output_from_point(
        &mut self,
        point: Point<f64, Logical>,
        source: &str,
        log_when_unchanged: bool,
    ) {
        let selected = self
            .output_registry
            .output_at_point(point.x, point.y)
            .map(|o| o.id);
        let Some(selected) = selected else {
            if log_when_unchanged {
                tracing::debug!(
                    "focused output unchanged: source={} reason=no-output-at-point current={:?}",
                    source,
                    self.focused_output()
                );
            }
            return;
        };

        let current = self.focused_output();
        if current == Some(selected) {
            if log_when_unchanged {
                tracing::debug!(
                    "focused output unchanged: source={} output_id={}",
                    source,
                    selected.0
                );
            }
            return;
        }

        self.set_focused_output(Some(selected));
    }

    pub fn update_focused_output_from_surface(&mut self, surface: &WlSurface, source: &str) {
        let idx = self.current_workspace_index();
        let space = self.workspaces.space_at(idx);
        let selected = self
            .workspaces
            .space_at(idx)
            .elements()
            .find(|window| {
                window
                    .toplevel()
                    .map_or(false, |toplevel| toplevel.wl_surface() == surface)
            })
            .and_then(|window| {
                let location = space.element_location(window)?;
                let geometry = window.geometry();
                let center: Point<f64, Logical> = (
                    (location.x + geometry.loc.x + geometry.size.w / 2) as f64,
                    (location.y + geometry.loc.y + geometry.size.h / 2) as f64,
                )
                    .into();
                self.output_registry
                    .output_at_point(center.x, center.y)
                    .map(|output| output.id)
            });

        if let Some(selected) = selected {
            let current = self.focused_output();
            if current == Some(selected) {
                tracing::debug!(
                    "focused output unchanged: source={} output_id={}",
                    source,
                    selected.0
                );
                return;
            }
            self.set_focused_output(Some(selected));
            return;
        }

        tracing::debug!(
            "focused output unchanged: source={} reason=surface-output-unresolved current={:?}",
            source,
            self.focused_output()
        );
    }

    pub fn switch_workspace(&mut self, idx: usize) {
        let old = self.workspaces.active;
        tracing::debug!(
            "workspace switch requested: old={} requested={}",
            old + 1,
            idx + 1
        );

        if idx >= self.workspaces.count() {
            tracing::debug!(
                "workspace switch ignored: requested={} valid_range=1..={}",
                idx + 1,
                self.workspaces.count()
            );
            return;
        }

        if idx == old {
            tracing::debug!(
                "workspace switch ignored: requested workspace {} is already active",
                idx + 1
            );
            return;
        }

        if let Some((old, new)) = self.workspaces.try_switch(idx) {
            // Phase-3 transition: keep per-output active workspace mapping in sync
            // for the currently focused output while global active remains compatible.
            self.set_active_workspace_for_output(self.focused_output(), new);
            let outputs = self.outputs.clone();
            self.workspaces.remap_outputs(&outputs, old, new);
            self.workspaces.space_at_mut(old).refresh();
            self.workspaces.space_at_mut(new).refresh();
            let serial = SERIAL_COUNTER.next_serial();
            self.set_keyboard_focus_with_decorations(Option::<WlSurface>::None, serial);
            self.broadcast_toplevel_focus_cleared();
            tracing::debug!("workspace switched: old={} new={}", old + 1, new + 1);
            self.mark_all_outputs_dirty("workspace-switch");
            self.broadcast_workspace();
            self.broadcast_window_snapshot();
        }
    }

    pub fn switch_workspace_for_focused_output(&mut self, idx: usize) {
        let old = self.workspaces.active;
        tracing::debug!(
            "focused-output workspace switch requested: old={} requested={}",
            old + 1,
            idx + 1
        );

        if idx >= self.workspaces.count() {
            tracing::debug!(
                "focused-output workspace switch ignored: requested={} valid_range=1..={}",
                idx + 1,
                self.workspaces.count()
            );
            return;
        }

        let focused_output = self.focused_output();
        let focused_output_name = focused_output
            .and_then(|id| self.output_registry.by_id(id).map(|info| info.name.clone()));
        tracing::debug!(
            "focused-output workspace switch context: output_id={:?} output_name={:?}",
            focused_output.map(|id| id.0),
            focused_output_name
        );

        self.set_active_workspace_for_output(focused_output, idx);
        if let Some(output_id) = focused_output {
            self.broadcast_output_workspace_changed(output_id.0, idx);
        }

        if idx == old {
            tracing::debug!(
                "focused-output workspace switch ignored: requested workspace {} is already active",
                idx + 1
            );
            return;
        }

        if let Some((old, new)) = self.workspaces.try_switch(idx) {
            let outputs = self.outputs.clone();
            self.workspaces.remap_outputs(&outputs, old, new);
            self.workspaces.space_at_mut(old).refresh();
            self.workspaces.space_at_mut(new).refresh();
            let serial = SERIAL_COUNTER.next_serial();
            self.set_keyboard_focus_with_decorations(Option::<WlSurface>::None, serial);
            self.broadcast_toplevel_focus_cleared();
            tracing::debug!(
                "compatibility global active updated: old={} new={}",
                old + 1,
                new + 1
            );
            self.mark_all_outputs_dirty("workspace-switch-focused-output");
            self.broadcast_workspace();
            self.broadcast_window_snapshot();
        }
    }

    pub fn move_focused_window_to_workspace_consistent(&mut self, target: usize) {
        let focused_output = self.focused_output();
        let focused_output_name = focused_output
            .and_then(|id| self.output_registry.by_id(id).map(|info| info.name.clone()));
        tracing::debug!(
            "focused-output move requested: target={} focused_output_id={:?} focused_output_name={:?}",
            target + 1,
            focused_output.map(|id| id.0),
            focused_output_name
        );

        let workspace_count = self.workspaces.count();

        let keyboard = match self.seat.get_keyboard() {
            Some(keyboard) => keyboard,
            None => {
                tracing::debug!(
                    "workspace move ignored, no focused window: reason=no-keyboard target={}",
                    target + 1
                );
                return;
            }
        };
        let Some(focus_surface) = keyboard.current_focus() else {
            tracing::debug!(
                "workspace move ignored, no focused window: reason=no-keyboard-focus target={}",
                target + 1
            );
            return;
        };

        let focused_window = (0..workspace_count).find_map(|idx| {
            self.workspaces
                .space_at(idx)
                .elements()
                .find(|window| {
                    window
                        .toplevel()
                        .map_or(false, |toplevel| toplevel.wl_surface() == &focus_surface)
                })
                .cloned()
                .map(|window| (idx, window))
        });

        let source_workspace = match validate_workspace_move_request(
            target,
            workspace_count,
            focused_window.as_ref().map(|(idx, _)| *idx),
        ) {
            Ok(source_workspace) => source_workspace,
            Err(MoveRequestGuard::InvalidTarget) => {
                tracing::debug!(
                    "workspace move ignored, invalid workspace: target={} valid_range=1..={}",
                    target + 1,
                    workspace_count
                );
                return;
            }
            Err(MoveRequestGuard::NoFocusedWindow) => {
                tracing::debug!("workspace move ignored, no focused window");
                return;
            }
            Err(MoveRequestGuard::AlreadyOnTarget) => {
                tracing::debug!(
                    "workspace move ignored, already on workspace: workspace={}",
                    target + 1
                );
                return;
            }
        };

        let Some((_, window)) = focused_window else {
            tracing::debug!("workspace move ignored, no focused window");
            return;
        };

        let (window_id, window_title) = match window.toplevel() {
            Some(toplevel) => (window_id(toplevel.wl_surface()), toplevel_title(&toplevel)),
            None => ("<no-toplevel>".to_string(), "Window".to_string()),
        };

        tracing::debug!(
            "workspace move details: window_id={} title={:?} source={} target={}",
            window_id,
            window_title,
            source_workspace + 1,
            target + 1
        );

        let serial = SERIAL_COUNTER.next_serial();
        self.set_keyboard_focus_with_decorations(Option::<WlSurface>::None, serial);
        self.broadcast_toplevel_focus_cleared();

        let loc: Point<i32, Logical> = self
            .workspaces
            .space_at(source_workspace)
            .element_location(&window)
            .unwrap_or_default();
        self.workspaces
            .space_at_mut(source_workspace)
            .unmap_elem(&window);
        self.workspaces
            .space_at_mut(target)
            .map_element(window, loc, false);

        self.workspaces.space_at_mut(source_workspace).refresh();
        self.workspaces.space_at_mut(target).refresh();

        tracing::debug!(
            "workspace move completed: window_id={} source={} target={}",
            window_id,
            source_workspace + 1,
            target + 1
        );

        self.mark_all_outputs_dirty("workspace-move");
        self.broadcast_workspace();
        self.broadcast_window_snapshot();
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_workspace_move_request, MoveRequestGuard};

    #[test]
    fn move_request_invalid_target_is_ignored() {
        assert_eq!(
            validate_workspace_move_request(9, 9, Some(0)),
            Err(MoveRequestGuard::InvalidTarget)
        );
    }

    #[test]
    fn move_request_without_focused_window_is_ignored() {
        assert_eq!(
            validate_workspace_move_request(1, 9, None),
            Err(MoveRequestGuard::NoFocusedWindow)
        );
    }

    #[test]
    fn move_request_target_equal_source_is_ignored() {
        assert_eq!(
            validate_workspace_move_request(2, 9, Some(2)),
            Err(MoveRequestGuard::AlreadyOnTarget)
        );
    }

    #[test]
    fn move_request_with_valid_target_and_source_is_accepted() {
        assert_eq!(validate_workspace_move_request(3, 9, Some(1)), Ok(1));
    }
}
