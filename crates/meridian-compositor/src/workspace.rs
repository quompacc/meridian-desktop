use smithay::{
    desktop::{Space, Window},
    output::Output,
    utils::{Logical, Point},
};

pub struct WorkspaceManager {
    spaces: Vec<Space<Window>>,
    pub active: usize,
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceManager {
    pub fn new() -> Self {
        Self {
            spaces: (0..9).map(|_| Space::default()).collect(),
            active: 0,
        }
    }

    pub fn count(&self) -> usize {
        self.spaces.len()
    }

    pub fn active_space(&self) -> &Space<Window> {
        &self.spaces[self.active]
    }

    pub fn active_space_mut(&mut self) -> &mut Space<Window> {
        &mut self.spaces[self.active]
    }

    pub fn space_at(&self, idx: usize) -> &Space<Window> {
        &self.spaces[idx]
    }

    pub fn space_at_mut(&mut self, idx: usize) -> &mut Space<Window> {
        &mut self.spaces[idx]
    }

    fn can_target_workspace(&self, idx: usize) -> bool {
        idx < self.spaces.len() && idx != self.active
    }

    /// Switch active workspace. Returns (old, new) if a switch occurred.
    pub fn try_switch(&mut self, idx: usize) -> Option<(usize, usize)> {
        if !self.can_target_workspace(idx) {
            return None;
        }
        let old = self.active;
        self.active = idx;
        Some((old, idx))
    }

    /// Move a window from the active workspace to `target`.
    pub fn move_window_to(&mut self, window: Window, target: usize) {
        if !self.can_target_workspace(target) {
            return;
        }
        let active = self.active;
        let loc: Point<i32, Logical> = self.spaces[active]
            .element_location(&window)
            .unwrap_or_default();
        self.spaces[active].unmap_elem(&window);
        self.spaces[target].map_element(window, loc, false);
    }

    /// Remap all tracked outputs from `old` workspace to `new` workspace.
    pub fn remap_outputs(&mut self, outputs: &[Output], old: usize, new: usize) {
        for output in outputs {
            self.spaces[old].unmap_output(output);
            self.spaces[new].map_output(output, (0, 0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WorkspaceManager;

    #[test]
    fn try_switch_ignores_invalid_target() {
        let mut manager = WorkspaceManager::new();
        let old = manager.active;
        assert_eq!(manager.try_switch(99), None);
        assert_eq!(manager.active, old);
    }

    #[test]
    fn try_switch_ignores_current_workspace() {
        let mut manager = WorkspaceManager::new();
        let old = manager.active;
        assert_eq!(manager.try_switch(old), None);
        assert_eq!(manager.active, old);
    }

    #[test]
    fn try_switch_updates_active_workspace_on_valid_target() {
        let mut manager = WorkspaceManager::new();
        assert_eq!(manager.active, 0);
        assert_eq!(manager.try_switch(2), Some((0, 2)));
        assert_eq!(manager.active, 2);
    }

    #[test]
    fn move_guards_share_same_target_validation() {
        let manager = WorkspaceManager::new();
        assert!(!manager.can_target_workspace(0));
        assert!(!manager.can_target_workspace(99));
        assert!(manager.can_target_workspace(1));
    }
}
