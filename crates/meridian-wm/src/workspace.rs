use smithay::{
    desktop::Window,
    utils::{Logical, Rectangle},
};

use crate::tiling::{SplitDir, TilingLayout};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceMode {
    Floating,
    Tiling,
}

pub struct WmWorkspace {
    pub mode: WorkspaceMode,
    pub tiling: TilingLayout,
    /// Windows that stay floating even when the workspace is in Tiling mode
    /// (dialogs, pop-overs, etc.).
    floating_windows: Vec<Window>,
}

impl WmWorkspace {
    pub fn new() -> Self {
        Self {
            mode: WorkspaceMode::Floating,
            tiling: TilingLayout::new(),
            floating_windows: Vec::new(),
        }
    }

    // ── Mode ──────────────────────────────────────────────────────────────────

    pub fn toggle_mode(&mut self) -> WorkspaceMode {
        self.mode = match self.mode {
            WorkspaceMode::Floating => WorkspaceMode::Tiling,
            WorkspaceMode::Tiling => WorkspaceMode::Floating,
        };
        self.mode
    }

    // ── Window management ─────────────────────────────────────────────────────

    /// Add a window to the tiling tree next to `focused` (or at the end).
    /// No-op if the window is marked as individually floating.
    pub fn add_tiled(&mut self, window: Window, focused: Option<&Window>) {
        if self.floating_windows.iter().any(|w| w == &window) {
            return;
        }
        self.tiling.add(window, focused);
    }

    /// Remove a window from both the tiling tree and the floating set.
    pub fn remove_window(&mut self, window: &Window) {
        self.tiling.remove(window);
        self.floating_windows.retain(|w| w != window);
    }

    /// Mark or unmark a window as individually floating within a tiling workspace.
    /// Floating windows are excluded from tile layout but still rendered on top.
    pub fn set_floating(&mut self, window: &Window, floating: bool) {
        if floating {
            self.tiling.remove(window);
            if !self.floating_windows.iter().any(|w| w == window) {
                self.floating_windows.push(window.clone());
            }
        } else {
            self.floating_windows.retain(|w| w != window);
            // Re-insert into tiling tree (at end since we have no focused context)
            self.tiling.add(window.clone(), None);
        }
    }

    pub fn is_floating(&self, window: &Window) -> bool {
        self.floating_windows.iter().any(|w| w == window)
    }

    /// Rebuild the tiling tree from scratch using the given window list.
    /// Preserves the floating-window set; already-floating windows are skipped.
    pub fn rebuild_tiling_from(&mut self, windows: impl Iterator<Item = Window>) {
        self.tiling = TilingLayout::new();
        for w in windows {
            if !self.floating_windows.iter().any(|fw| fw == &w) {
                self.tiling.add(w, None);
            }
        }
    }

    // ── Layout ────────────────────────────────────────────────────────────────

    /// Compute tiled window positions.  Returns empty when mode is Floating.
    /// `gap` comes from `ThemeConfig::decorations.gap`.
    pub fn compute_tiled(
        &self,
        screen: Rectangle<i32, Logical>,
        gap: i32,
    ) -> Vec<(Window, Rectangle<i32, Logical>)> {
        if self.mode == WorkspaceMode::Floating {
            return Vec::new();
        }
        self.tiling
            .compute_rects(screen, gap)
            .into_iter()
            .filter(|(w, _)| !self.floating_windows.iter().any(|fw| fw == w))
            .collect()
    }

    /// Nudge the focused window's split ratio.
    pub fn resize_focused(&mut self, window: &Window, dir: SplitDir, delta: f32) {
        self.tiling.adjust_split(window, dir, delta);
    }

    pub fn force_split(&mut self, dir: SplitDir) {
        self.tiling.next_split = dir;
    }
}

impl Default for WmWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{WmWorkspace, WorkspaceMode};
    use crate::SplitDir;

    #[test]
    fn new_sets_expected_defaults() {
        let workspace = WmWorkspace::new();

        assert_eq!(workspace.mode, WorkspaceMode::Floating);
        assert!(workspace.tiling.is_empty());
        assert_eq!(workspace.tiling.next_split, SplitDir::Horizontal);
    }

    #[test]
    fn toggle_mode_switches_between_floating_and_tiling() {
        let mut workspace = WmWorkspace::new();

        let first = workspace.toggle_mode();
        assert_eq!(first, WorkspaceMode::Tiling);
        assert_eq!(workspace.mode, WorkspaceMode::Tiling);

        let second = workspace.toggle_mode();
        assert_eq!(second, WorkspaceMode::Floating);
        assert_eq!(workspace.mode, WorkspaceMode::Floating);
    }
}
