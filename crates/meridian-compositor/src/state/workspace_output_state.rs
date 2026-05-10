use std::collections::{HashMap, HashSet};

use super::{OutputId, OutputRegistry};

#[derive(Debug, Default, Clone)]
pub struct WorkspaceOutputState {
    focused_output: Option<OutputId>,
    active_workspace_by_output: HashMap<OutputId, usize>,
}

impl WorkspaceOutputState {
    pub fn raw_focused_output(&self) -> Option<OutputId> {
        self.focused_output
    }

    pub fn has_stale_focused_output(&self, registry: &OutputRegistry) -> bool {
        self.focused_output
            .is_some_and(|id| registry.by_id(id).is_none())
    }

    fn fallback_output_id(registry: &OutputRegistry) -> Option<OutputId> {
        registry
            .primary()
            .or_else(|| registry.first())
            .map(|info| info.id)
    }

    pub fn focused_output(&self, registry: &OutputRegistry) -> Option<OutputId> {
        self.focused_output
            .filter(|id| registry.by_id(*id).is_some())
            .or_else(|| Self::fallback_output_id(registry))
    }

    pub fn set_focused_output(
        &mut self,
        requested: Option<OutputId>,
        registry: &OutputRegistry,
    ) -> bool {
        let resolved = requested
            .filter(|id| registry.by_id(*id).is_some())
            .or_else(|| Self::fallback_output_id(registry));
        if self.focused_output == resolved {
            return false;
        }
        self.focused_output = resolved;
        true
    }

    pub fn active_workspace_for_output(
        &self,
        output: Option<OutputId>,
        registry: &OutputRegistry,
        global_active: usize,
    ) -> usize {
        let resolved = output
            .filter(|id| registry.by_id(*id).is_some())
            .or_else(|| self.focused_output(registry));
        resolved
            .and_then(|id| self.active_workspace_by_output.get(&id).copied())
            .unwrap_or(global_active)
    }

    pub fn set_active_workspace_for_output(
        &mut self,
        output: Option<OutputId>,
        workspace: usize,
        registry: &OutputRegistry,
        workspace_count: usize,
    ) -> bool {
        if workspace >= workspace_count {
            return false;
        }
        let resolved = output
            .filter(|id| registry.by_id(*id).is_some())
            .or_else(|| self.focused_output(registry));
        let Some(id) = resolved else {
            return false;
        };
        if self
            .active_workspace_by_output
            .get(&id)
            .copied()
            .is_some_and(|existing| existing == workspace)
        {
            return false;
        }
        self.active_workspace_by_output.insert(id, workspace);
        true
    }

    pub fn sync_outputs_with_workspace_state(
        &mut self,
        registry: &OutputRegistry,
        global_active: usize,
        workspace_count: usize,
    ) -> bool {
        let mut changed = false;

        let valid_ids: HashSet<OutputId> = registry.list().iter().map(|info| info.id).collect();
        let prev_len = self.active_workspace_by_output.len();
        self.active_workspace_by_output
            .retain(|id, _| valid_ids.contains(id));
        let removed_mappings = prev_len.saturating_sub(self.active_workspace_by_output.len());
        if removed_mappings > 0 {
            tracing::debug!(
                "stale output workspace mapping removed: removed={} remaining={}",
                removed_mappings,
                self.active_workspace_by_output.len()
            );
            changed = true;
        }

        let default_workspace = if workspace_count == 0 {
            0
        } else {
            global_active.min(workspace_count - 1)
        };

        for info in registry.list() {
            self.active_workspace_by_output
                .entry(info.id)
                .or_insert(default_workspace);
        }

        for workspace in self.active_workspace_by_output.values_mut() {
            if *workspace >= workspace_count {
                *workspace = default_workspace;
                changed = true;
            }
        }

        let previous_focus = self.focused_output;
        let previous_focus_was_stale =
            previous_focus.is_some_and(|id| registry.by_id(id).is_none());
        if registry.list().is_empty() {
            if previous_focus.is_some() {
                tracing::debug!("focused output cleared because registry empty");
            }
            self.focused_output = None;
        } else if previous_focus_was_stale || self.focused_output.is_none() {
            self.focused_output = Self::fallback_output_id(registry);
            if previous_focus_was_stale && self.focused_output != previous_focus {
                tracing::debug!(
                    "focused output fallback after output removal: old={:?} new={:?}",
                    previous_focus,
                    self.focused_output
                );
            }
        }
        if self.focused_output != previous_focus {
            changed = true;
        }

        changed
    }
}

#[cfg(test)]
mod tests {
    use smithay::utils::Transform;

    use crate::state::{OutputGeometry, OutputReconfigure, OutputRegistration, OutputRegistry};

    use super::{OutputId, WorkspaceOutputState};

    fn reg(name: &str, x: i32, y: i32, width: i32, height: i32) -> OutputRegistration {
        OutputRegistration {
            name: name.to_string(),
            geometry: OutputGeometry {
                x,
                y,
                width,
                height,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
        }
    }

    fn reconfigure_primary(primary: bool) -> OutputReconfigure {
        OutputReconfigure {
            geometry: OutputGeometry {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
            primary: Some(primary),
        }
    }

    #[test]
    fn single_output_initializes_focused_output() {
        let mut registry = OutputRegistry::new();
        let first = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
        let mut state = WorkspaceOutputState::default();
        state.sync_outputs_with_workspace_state(&registry, 0, 9);
        assert_eq!(state.focused_output(&registry), Some(first));
    }

    #[test]
    fn sync_creates_active_workspace_mapping_for_outputs() {
        let mut registry = OutputRegistry::new();
        let first = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
        let second = registry.upsert(reg("HDMI-A-1", 1920, 0, 1920, 1080));
        let mut state = WorkspaceOutputState::default();
        state.sync_outputs_with_workspace_state(&registry, 2, 9);
        assert_eq!(
            state.active_workspace_for_output(Some(first), &registry, 0),
            2
        );
        assert_eq!(
            state.active_workspace_for_output(Some(second), &registry, 0),
            2
        );
    }

    #[test]
    fn unknown_output_fallback_is_safe() {
        let mut registry = OutputRegistry::new();
        let first = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
        let mut state = WorkspaceOutputState::default();
        state.sync_outputs_with_workspace_state(&registry, 4, 9);
        assert_eq!(
            state.active_workspace_for_output(Some(OutputId(9999)), &registry, 0),
            4
        );
        state.set_focused_output(Some(OutputId(9999)), &registry);
        assert_eq!(state.focused_output(&registry), Some(first));
    }

    #[test]
    fn set_get_active_workspace_per_output() {
        let mut registry = OutputRegistry::new();
        let first = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
        let mut state = WorkspaceOutputState::default();
        state.sync_outputs_with_workspace_state(&registry, 0, 9);
        assert!(state.set_active_workspace_for_output(Some(first), 5, &registry, 9));
        assert_eq!(
            state.active_workspace_for_output(Some(first), &registry, 0),
            5
        );
    }

    #[test]
    fn invalid_target_for_output_mapping_is_ignored() {
        let mut registry = OutputRegistry::new();
        let first = registry.upsert(reg("eDP-1", 0, 0, 1920, 1080));
        let mut state = WorkspaceOutputState::default();
        state.sync_outputs_with_workspace_state(&registry, 0, 9);
        assert!(!state.set_active_workspace_for_output(Some(first), 99, &registry, 9));
        assert_eq!(
            state.active_workspace_for_output(Some(first), &registry, 0),
            0
        );
    }

    #[test]
    fn focused_output_mapping_is_used_for_current_workspace_read() {
        let mut registry = OutputRegistry::new();
        let first = registry.upsert(reg("left", 0, 0, 1920, 1080));
        let second = registry.upsert(reg("right", 1920, 0, 1920, 1080));
        let mut state = WorkspaceOutputState::default();
        state.sync_outputs_with_workspace_state(&registry, 0, 9);
        state.set_focused_output(Some(second), &registry);
        state.set_active_workspace_for_output(Some(second), 6, &registry, 9);
        assert_eq!(
            state.active_workspace_for_output(None, &registry, 0),
            6,
            "focused output mapping should be preferred"
        );
        assert_ne!(first, second);
    }

    #[test]
    fn missing_focused_output_falls_back_to_global_active() {
        let mut registry = OutputRegistry::new();
        registry.upsert(reg("only", 0, 0, 1920, 1080));
        let state = WorkspaceOutputState::default();
        assert_eq!(
            state.active_workspace_for_output(None, &registry, 4),
            4,
            "without focused output the global active fallback must be used"
        );
    }

    #[test]
    fn missing_mapping_falls_back_to_global_active() {
        let mut registry = OutputRegistry::new();
        let first = registry.upsert(reg("left", 0, 0, 1920, 1080));
        let mut state = WorkspaceOutputState::default();
        state.set_focused_output(Some(first), &registry);
        assert_eq!(
            state.active_workspace_for_output(Some(first), &registry, 3),
            3,
            "missing per-output mapping should fallback to global active"
        );
    }

    #[test]
    fn sync_removes_stale_output_mapping() {
        let registry = OutputRegistry::new();
        let mut state = WorkspaceOutputState::default();
        state.active_workspace_by_output.insert(OutputId(77), 3);
        state.set_focused_output(Some(OutputId(77)), &registry);
        state.sync_outputs_with_workspace_state(&registry, 0, 9);
        assert!(state.active_workspace_by_output.is_empty());
        assert_eq!(state.focused_output(&registry), None);
    }

    #[test]
    fn removed_focused_output_falls_back_to_primary() {
        let mut registry = OutputRegistry::new();
        let first = registry.upsert(reg("first", 0, 0, 1920, 1080));
        let second = registry.upsert(reg("second", 1920, 0, 1920, 1080));
        let third = registry.upsert(reg("third", 3840, 0, 1920, 1080));
        assert!(registry.reconfigure_by_id(first, reconfigure_primary(false)));
        assert!(registry.reconfigure_by_id(second, reconfigure_primary(true)));

        let mut state = WorkspaceOutputState::default();
        state.sync_outputs_with_workspace_state(&registry, 1, 9);
        state.set_focused_output(Some(third), &registry);
        registry.remove_by_id(third);
        state.sync_outputs_with_workspace_state(&registry, 1, 9);
        assert_eq!(state.focused_output(&registry), Some(second));
    }

    #[test]
    fn removed_focused_output_falls_back_to_first_when_no_primary() {
        let mut registry = OutputRegistry::new();
        let first = registry.upsert(reg("first", 0, 0, 1920, 1080));
        let second = registry.upsert(reg("second", 1920, 0, 1920, 1080));
        let third = registry.upsert(reg("third", 3840, 0, 1920, 1080));
        assert!(registry.reconfigure_by_id(first, reconfigure_primary(false)));
        assert!(registry.reconfigure_by_id(second, reconfigure_primary(false)));

        let mut state = WorkspaceOutputState::default();
        state.sync_outputs_with_workspace_state(&registry, 1, 9);
        state.set_focused_output(Some(third), &registry);
        registry.remove_by_id(third);
        state.sync_outputs_with_workspace_state(&registry, 1, 9);
        assert_eq!(state.focused_output(&registry), Some(first));
    }

    #[test]
    fn all_outputs_removed_clears_focused_output() {
        let mut registry = OutputRegistry::new();
        let first = registry.upsert(reg("only", 0, 0, 1920, 1080));
        let mut state = WorkspaceOutputState::default();
        state.sync_outputs_with_workspace_state(&registry, 0, 9);
        state.set_focused_output(Some(first), &registry);
        registry.remove_by_id(first);
        state.sync_outputs_with_workspace_state(&registry, 0, 9);
        assert_eq!(state.focused_output(&registry), None);
    }

    #[test]
    fn reconfigure_keeps_focused_output_and_mapping_for_same_id() {
        let mut registry = OutputRegistry::new();
        let first = registry.upsert(reg("first", 0, 0, 1920, 1080));
        let mut state = WorkspaceOutputState::default();
        state.sync_outputs_with_workspace_state(&registry, 2, 9);
        state.set_focused_output(Some(first), &registry);
        assert!(state.set_active_workspace_for_output(Some(first), 6, &registry, 9));

        let changed = registry.reconfigure_by_id(
            first,
            OutputReconfigure {
                geometry: OutputGeometry {
                    x: 10,
                    y: 20,
                    width: 2560,
                    height: 1440,
                },
                scale: 1.5,
                transform: Transform::Normal,
                refresh_millihz: Some(75_000),
                primary: Some(true),
            },
        );
        assert!(changed);
        state.sync_outputs_with_workspace_state(&registry, 2, 9);
        assert_eq!(state.focused_output(&registry), Some(first));
        assert_eq!(
            state.active_workspace_for_output(Some(first), &registry, 0),
            6
        );
    }

    #[test]
    fn add_new_output_gets_global_active_mapping_and_focus_stays_stable() {
        let mut registry = OutputRegistry::new();
        let first = registry.upsert(reg("first", 0, 0, 1920, 1080));
        let mut state = WorkspaceOutputState::default();
        state.sync_outputs_with_workspace_state(&registry, 1, 9);
        state.set_focused_output(Some(first), &registry);
        assert!(state.set_active_workspace_for_output(Some(first), 5, &registry, 9));

        let second = registry.upsert(reg("second", 1920, 0, 1920, 1080));
        state.sync_outputs_with_workspace_state(&registry, 3, 9);
        assert_eq!(state.focused_output(&registry), Some(first));
        assert_eq!(
            state.active_workspace_for_output(Some(second), &registry, 0),
            3
        );
    }
}
