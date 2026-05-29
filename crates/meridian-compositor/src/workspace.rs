use smithay::{
    desktop::{space::SpaceElement, Space, Window},
    output::Output,
    utils::{Logical, Point},
};

/// Per-workspace window spaces plus the active index.
///
/// Generic over the space element so the workspace/window-lifecycle logic can
/// be unit-tested with a mock element (the production type is `Window`, which
/// needs a live Wayland/X11 surface and cannot be built in tests). Only the
/// active workspace is rendered; all of them are searched when a window must
/// be located regardless of which workspace is currently shown.
pub struct WorkspaceManager<E: SpaceElement = Window> {
    spaces: Vec<Space<E>>,
    pub active: usize,
}

impl<E: SpaceElement + PartialEq> Default for WorkspaceManager<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: SpaceElement + PartialEq> WorkspaceManager<E> {
    pub fn new() -> Self {
        Self {
            spaces: (0..9).map(|_| Space::default()).collect(),
            active: 0,
        }
    }

    pub fn count(&self) -> usize {
        self.spaces.len()
    }

    pub fn active_space(&self) -> &Space<E> {
        &self.spaces[self.active]
    }

    pub fn active_space_mut(&mut self) -> &mut Space<E> {
        &mut self.spaces[self.active]
    }

    pub fn space_at(&self, idx: usize) -> &Space<E> {
        &self.spaces[idx]
    }

    pub fn space_at_mut(&mut self, idx: usize) -> &mut Space<E> {
        &mut self.spaces[idx]
    }

    /// Locate an element and its workspace index by predicate, searching
    /// **every** workspace (not just the active one). Window-lifecycle cleanup
    /// must use this so a window that lives on a non-active workspace is not
    /// missed (audit XW-1: active-only search left taskbar ghosts).
    pub fn find_element_workspace<F>(&self, pred: F) -> Option<(usize, &E)>
    where
        F: Fn(&E) -> bool,
    {
        (0..self.spaces.len()).find_map(|ws| {
            // `elements()` yields `&E`, so `find` hands the closure `&&E`;
            // deref once for `pred`.
            self.spaces[ws]
                .elements()
                .find(|candidate| pred(&**candidate))
                .map(|e| (ws, e))
        })
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
    pub fn move_window_to(&mut self, window: E, target: usize) {
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
    use smithay::{
        desktop::space::SpaceElement,
        output::Output,
        utils::{IsAlive, Logical, Point, Rectangle},
    };

    /// Minimal SpaceElement for testing workspace/window bookkeeping without a
    /// live Wayland surface (which the real `Window` requires).
    #[derive(Clone, PartialEq, Debug)]
    struct MockWindow {
        id: u32,
    }

    impl IsAlive for MockWindow {
        fn alive(&self) -> bool {
            true
        }
    }

    impl SpaceElement for MockWindow {
        fn bbox(&self) -> Rectangle<i32, Logical> {
            Rectangle::new((0, 0).into(), (10, 10).into())
        }
        fn is_in_input_region(&self, _point: &Point<f64, Logical>) -> bool {
            false
        }
        fn set_activate(&self, _activated: bool) {}
        fn output_enter(&self, _output: &Output, _overlap: Rectangle<i32, Logical>) {}
        fn output_leave(&self, _output: &Output) {}
    }

    fn manager() -> WorkspaceManager<MockWindow> {
        WorkspaceManager::new()
    }

    #[test]
    fn try_switch_ignores_invalid_target() {
        let mut manager = manager();
        let old = manager.active;
        assert_eq!(manager.try_switch(99), None);
        assert_eq!(manager.active, old);
    }

    #[test]
    fn try_switch_ignores_current_workspace() {
        let mut manager = manager();
        let old = manager.active;
        assert_eq!(manager.try_switch(old), None);
        assert_eq!(manager.active, old);
    }

    #[test]
    fn try_switch_updates_active_workspace_on_valid_target() {
        let mut manager = manager();
        assert_eq!(manager.active, 0);
        assert_eq!(manager.try_switch(2), Some((0, 2)));
        assert_eq!(manager.active, 2);
    }

    #[test]
    fn move_guards_share_same_target_validation() {
        let manager = manager();
        assert!(!manager.can_target_workspace(0));
        assert!(!manager.can_target_workspace(99));
        assert!(manager.can_target_workspace(1));
    }

    // ── Window-lifecycle search/move semantics (audit XW-1 / GR-1 kernel) ──

    #[test]
    fn find_element_workspace_finds_window_on_non_active_workspace() {
        let mut m = manager();
        // Map a window into workspace 3 while active is 0.
        m.space_at_mut(3)
            .map_element(MockWindow { id: 7 }, (0, 0), false);
        // The all-workspace search must find it (active-only would miss it —
        // the bug XW-1 fixed).
        let found = m.find_element_workspace(|w| w.id == 7);
        assert_eq!(found.map(|(ws, w)| (ws, w.id)), Some((3, 7)));
    }

    #[test]
    fn find_element_workspace_is_none_for_absent_or_empty() {
        let mut m = manager();
        assert!(m.find_element_workspace(|_| true).is_none());
        m.space_at_mut(1)
            .map_element(MockWindow { id: 1 }, (0, 0), false);
        assert!(m.find_element_workspace(|w| w.id == 99).is_none());
    }

    #[test]
    fn move_window_to_relocates_and_leaves_no_duplicate() {
        let mut m = manager();
        let w = MockWindow { id: 1 };
        m.active_space_mut().map_element(w.clone(), (5, 5), false); // workspace 0
        assert_eq!(
            m.find_element_workspace(|x| x.id == 1).map(|(ws, _)| ws),
            Some(0)
        );
        m.move_window_to(w, 2);
        // Now present in workspace 2 only — not duplicated across workspaces.
        assert_eq!(
            m.find_element_workspace(|x| x.id == 1).map(|(ws, _)| ws),
            Some(2)
        );
        assert!(m.space_at(0).elements().next().is_none());
    }

    #[test]
    fn move_window_to_rejects_active_and_out_of_range() {
        let mut m = manager();
        let w = MockWindow { id: 5 };
        m.active_space_mut().map_element(w.clone(), (0, 0), false);
        m.move_window_to(w.clone(), 0); // active -> no-op
        m.move_window_to(w, 99); // out of range -> no-op
        assert_eq!(
            m.find_element_workspace(|x| x.id == 5).map(|(ws, _)| ws),
            Some(0)
        );
    }
}
