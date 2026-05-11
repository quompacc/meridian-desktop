use std::time::{Duration, Instant};

use meridian_config::{MeridianConfig, ThemeConfig, ThemeManager};
use meridian_ipc::{OutputWorkspaceState, ShellCommand, ShellEvent, WindowSnapshotEntry};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity};
use tracing::{debug, info};
use wayland_client::QueueHandle;

use crate::{launcher, TextRenderer};

use super::{time, types::WindowInfo, ClickAction, CommitReason, MeridianShell, RepaintReason};

fn workspace_idx(workspace: u8) -> usize {
    workspace.saturating_sub(1).min(8) as usize
}

fn apply_workspace_changed(active_workspace: &mut u8, next_workspace_raw: u8) {
    *active_workspace = next_workspace_raw.clamp(1, 9);
}

fn normalize_workspace_1_based(workspace: usize) -> usize {
    workspace.clamp(1, 9)
}

fn select_panel_active_workspace(
    legacy_active_workspace: u8,
    output_workspace_state_available: bool,
    focused_output_id: Option<u32>,
    output_workspaces: &[OutputWorkspaceState],
) -> u8 {
    if !output_workspace_state_available || output_workspaces.is_empty() {
        return legacy_active_workspace.clamp(1, 9);
    }

    if let Some(workspace) = focused_output_id.and_then(|id| {
        output_workspaces
            .iter()
            .find(|state| state.output_id == id)
            .map(|state| state.active_workspace)
    }) {
        return normalize_workspace_1_based(workspace) as u8;
    }

    if let Some(workspace) = output_workspaces
        .iter()
        .find(|state| state.focused)
        .map(|state| state.active_workspace)
    {
        return normalize_workspace_1_based(workspace) as u8;
    }

    if let Some(workspace) = output_workspaces
        .iter()
        .find(|state| state.primary)
        .map(|state| state.active_workspace)
    {
        return normalize_workspace_1_based(workspace) as u8;
    }

    if let Some(workspace) = output_workspaces
        .first()
        .map(|state| state.active_workspace)
    {
        return normalize_workspace_1_based(workspace) as u8;
    }

    legacy_active_workspace.clamp(1, 9)
}

fn apply_output_workspace_snapshot_state(
    focused_output_id: &mut Option<u32>,
    output_workspaces: &mut Vec<OutputWorkspaceState>,
    output_workspace_state_available: &mut bool,
    workspace_indicator_dirty: &mut bool,
    next_focused_output_id: Option<u32>,
    next_output_workspaces: Vec<OutputWorkspaceState>,
) {
    *focused_output_id = next_focused_output_id;
    *output_workspaces = next_output_workspaces
        .into_iter()
        .map(|mut state| {
            state.active_workspace = normalize_workspace_1_based(state.active_workspace);
            state
        })
        .collect();
    *output_workspace_state_available = true;
    *workspace_indicator_dirty = true;
}

fn apply_output_workspace_changed_state(
    focused_output_id: &mut Option<u32>,
    output_workspaces: &mut Vec<OutputWorkspaceState>,
    output_workspace_state_available: &mut bool,
    workspace_indicator_dirty: &mut bool,
    output_id: u32,
    output_name: Option<String>,
    workspace: usize,
    focused: bool,
) {
    let workspace = normalize_workspace_1_based(workspace);

    if let Some(existing) = output_workspaces
        .iter_mut()
        .find(|state| state.output_id == output_id)
    {
        existing.active_workspace = workspace;
        existing.focused = focused;
        if output_name.is_some() {
            existing.output_name = output_name;
        }
    } else {
        // Unknown output on changed-event: add a minimal entry and let the next
        // full snapshot reconcile primary/name/focus details.
        output_workspaces.push(OutputWorkspaceState {
            output_id,
            output_name,
            active_workspace: workspace,
            primary: false,
            focused,
        });
    }

    if focused {
        *focused_output_id = Some(output_id);
    } else if focused_output_id.is_some_and(|id| id == output_id) {
        *focused_output_id = None;
    }

    *output_workspace_state_available = true;
    *workspace_indicator_dirty = true;
}

fn apply_window_opened_state(windows: &mut Vec<WindowInfo>, id: String, title: String) {
    if let Some(window) = windows.iter_mut().find(|w| w.id == id) {
        window.title = title;
    } else {
        windows.push(WindowInfo { id, title });
    }
}

fn apply_window_closed_state(windows: &mut Vec<WindowInfo>, id: &str) {
    windows.retain(|w| w.id != id);
}

fn apply_full_window_snapshot(
    active_workspace: &mut u8,
    windows: &mut Vec<WindowInfo>,
    workspace_window_counts: &mut [u16; 9],
    snapshot_active_workspace: u8,
    snapshot_windows: Vec<WindowSnapshotEntry>,
) {
    *active_workspace = snapshot_active_workspace.clamp(1, 9);
    *workspace_window_counts = [0; 9];
    windows.clear();

    for window in snapshot_windows {
        let idx = workspace_idx(window.workspace);
        workspace_window_counts[idx] = workspace_window_counts[idx].saturating_add(1);
        windows.push(WindowInfo {
            id: window.id,
            title: window.title,
        });
    }
}

fn compute_occupied_workspaces(workspace_window_counts: &[u16; 9]) -> [bool; 9] {
    let mut occupied = [false; 9];
    for (i, count) in workspace_window_counts.iter().enumerate() {
        occupied[i] = *count > 0;
    }
    occupied
}

fn panel_theme_signature(theme: &ThemeConfig) -> (String, [u8; 20]) {
    (
        theme.fonts.ui.clone(),
        [
            theme.colors.background.r,
            theme.colors.background.g,
            theme.colors.background.b,
            theme.colors.background.a,
            theme.colors.surface.r,
            theme.colors.surface.g,
            theme.colors.surface.b,
            theme.colors.surface.a,
            theme.colors.accent.r,
            theme.colors.accent.g,
            theme.colors.accent.b,
            theme.colors.accent.a,
            theme.colors.text.r,
            theme.colors.text.g,
            theme.colors.text.b,
            theme.colors.text.a,
            theme.colors.border.r,
            theme.colors.border.g,
            theme.colors.border.b,
            theme.colors.border.a,
        ],
    )
}

fn resolve_shell_theme_from_config(
    config: &MeridianConfig,
) -> Result<(String, ThemeConfig), String> {
    let mut theme_manager = ThemeManager::new();
    let requested_theme = if config.general.theme.trim().is_empty() {
        "default"
    } else {
        config.general.theme.trim()
    };
    theme_manager
        .set_theme(requested_theme)
        .map_err(|err| format!("theme load failed: {}", err))?;

    if let Some(cursor) = &config.cursor {
        theme_manager.current_mut().config.cursor.theme = cursor.theme.clone();
        theme_manager.current_mut().config.cursor.size = cursor.size;
    }
    if config.wallpaper.is_some() {
        theme_manager.current_mut().config.wallpaper = config.wallpaper_override();
    }

    Ok((
        theme_manager.current().name.clone(),
        theme_manager.current().config.clone(),
    ))
}

impl MeridianShell {
    pub(crate) fn tick_commit_stats(&mut self) {
        self.maybe_log_commit_stats(Instant::now());
    }

    pub(crate) fn panel_active_workspace(&self) -> u8 {
        select_panel_active_workspace(
            self.active_workspace,
            self.output_workspace_state_available,
            self.focused_output_id,
            &self.output_workspaces,
        )
    }

    pub(crate) fn tick(&mut self, qh: &QueueHandle<Self>) {
        let now = Instant::now();
        if now.duration_since(self.last_tick) >= Duration::from_secs(1) {
            self.last_tick = now;
            let clock = time::formatted_time();
            if clock != self.last_clock {
                self.last_clock = clock;
                self.draw_panel(qh, RepaintReason::Clock);
            }
        }
        self.maybe_log_repaint_stats(now);
        self.maybe_log_commit_stats(now);
        self.maybe_log_render_stats(now);

        if self.ipc.should_reconnect() {
            self.ipc.reconnect();
        }

        if !self.workspace_state_received
            && !self.ipc.is_connected()
            && !self.workspace_ipc_unavailable_logged
        {
            debug!("IPC workspace state unavailable; using fallback workspace 1");
            self.workspace_ipc_unavailable_logged = true;
        }

        if !self.occupied_state_available && !self.occupied_unavailable_logged {
            debug!("window snapshot unavailable; occupied state fallback active-only");
            self.occupied_unavailable_logged = true;
        }
    }

    pub(crate) fn poll_ipc(&mut self) -> bool {
        let mut changed = false;
        for event in self.ipc.poll() {
            self.apply_ipc_event(event);
            changed = true;
        }
        changed
    }

    fn apply_ipc_event(&mut self, event: ShellEvent) {
        match event {
            ShellEvent::WorkspaceChanged { workspace } => {
                let old = self.active_workspace;
                if !self.output_workspace_state_available {
                    debug!("legacy workspace fallback used: workspace={}", workspace);
                }
                apply_workspace_changed(&mut self.active_workspace, workspace);
                let next = self.active_workspace;
                debug!("workspace state received: active_workspace={}", next);
                if old != next {
                    debug!("active workspace changed: old={} new={}", old, next);
                    self.workspace_indicator_dirty = true;
                }
                self.workspace_state_received = true;
                self.workspace_ipc_unavailable_logged = false;
            }
            ShellEvent::WindowSnapshot {
                active_workspace,
                windows,
            } => {
                let old = self.active_workspace;
                debug!(
                    "full window snapshot received: active_workspace={} windows={}",
                    active_workspace,
                    windows.len()
                );
                apply_full_window_snapshot(
                    &mut self.active_workspace,
                    &mut self.windows,
                    &mut self.workspace_window_counts,
                    active_workspace,
                    windows,
                );
                if old != self.active_workspace {
                    debug!(
                        "active workspace changed: old={} new={}",
                        old, self.active_workspace
                    );
                    self.workspace_indicator_dirty = true;
                }
                self.workspace_state_received = true;
                self.workspace_ipc_unavailable_logged = false;
                self.occupied_state_available = true;
                self.occupied_unavailable_logged = false;
                self.update_occupied_workspaces();
                self.update_focused_title();
            }
            ShellEvent::OutputWorkspaceChanged {
                output_id,
                output_name,
                workspace,
                focused,
            } => {
                debug!(
                    "output workspace changed received: output_id={} output_name={:?} workspace={} focused={}",
                    output_id,
                    output_name,
                    workspace,
                    focused
                );
                apply_output_workspace_changed_state(
                    &mut self.focused_output_id,
                    &mut self.output_workspaces,
                    &mut self.output_workspace_state_available,
                    &mut self.workspace_indicator_dirty,
                    output_id,
                    output_name,
                    workspace,
                    focused,
                );
                debug!(
                    "output workspace state available: focused_output_id={:?} outputs={}",
                    self.focused_output_id,
                    self.output_workspaces.len()
                );
                self.workspace_state_received = true;
                self.workspace_ipc_unavailable_logged = false;
            }
            ShellEvent::OutputWorkspaceSnapshot {
                focused_output_id,
                outputs,
            } => {
                debug!(
                    "output workspace snapshot received: focused_output_id={:?} outputs={}",
                    focused_output_id,
                    outputs.len()
                );
                apply_output_workspace_snapshot_state(
                    &mut self.focused_output_id,
                    &mut self.output_workspaces,
                    &mut self.output_workspace_state_available,
                    &mut self.workspace_indicator_dirty,
                    focused_output_id,
                    outputs,
                );
                debug!(
                    "output workspace state available: focused_output_id={:?} outputs={}",
                    self.focused_output_id,
                    self.output_workspaces.len()
                );
                self.workspace_state_received = true;
                self.workspace_ipc_unavailable_logged = false;
            }
            ShellEvent::WindowOpened { id, title } => {
                apply_window_opened_state(&mut self.windows, id, title);
                self.update_focused_title();
            }
            ShellEvent::WindowClosed { id } => {
                apply_window_closed_state(&mut self.windows, &id);
                if self.focused_window_id.as_deref() == Some(id.as_str()) {
                    self.focused_window_id = None;
                    self.focused_title = None;
                }
            }
            ShellEvent::WindowFocused { id } => {
                self.focused_window_id = Some(id);
                self.update_focused_title();
            }
            ShellEvent::ConfigReloaded { success } => {
                debug!("ConfigReloaded {{ success: {} }}", success);
                self.handle_config_reloaded(success);
            }
            ShellEvent::ToggleLauncher => {
                self.toggle_launcher();
            }
        }
    }

    fn handle_config_reloaded(&mut self, success: bool) {
        debug!("shell config reload requested");
        if !success {
            tracing::warn!("shell config reload failed; keeping previous config");
            return;
        }

        let mut config = MeridianConfig::default();
        if let Err(err) = config.reload() {
            tracing::warn!(
                "shell config reload failed; keeping previous config: {}",
                err
            );
            return;
        }

        match resolve_shell_theme_from_config(&config) {
            Ok((theme_name, new_theme)) => {
                let old_sig = panel_theme_signature(&self.theme);
                let new_sig = panel_theme_signature(&new_theme);
                let theme_changed = old_sig != new_sig || self.theme_name != theme_name;

                if theme_changed {
                    debug!(
                        "shell theme changed: old={} new={}",
                        self.theme_name, theme_name
                    );
                }

                let font_changed = self.theme.fonts.ui != new_theme.fonts.ui;
                self.theme_name = theme_name;
                self.theme = new_theme;

                if font_changed {
                    if let Some(renderer) = TextRenderer::new(&self.theme.fonts.ui, 13) {
                        *self.font.borrow_mut() = Some(renderer);
                    } else {
                        tracing::warn!(
                            "shell font reload failed for {:?}; keeping previous renderer",
                            self.theme.fonts.ui
                        );
                    }
                }

                self.panel_dirty = true;
                debug!("panel marked dirty after reload");
                self.launcher_state.apps = launcher::DesktopApp::load_system();
                debug!("shell config reload succeeded");
            }
            Err(err) => {
                tracing::warn!(
                    "shell config reload failed; keeping previous config: {}",
                    err
                );
            }
        }
    }

    fn update_focused_title(&mut self) {
        self.focused_title = self
            .focused_window_id
            .as_deref()
            .and_then(|id| self.windows.iter().find(|w| w.id == id))
            .map(|w| w.title.clone());
    }

    fn toggle_launcher(&mut self) {
        let open_before = self.launcher_state.open;
        self.launcher_state.toggle();
        let open_after = self.launcher_state.open;
        if self.launcher_state.open {
            self.launcher_layer
                .set_anchor(Anchor::BOTTOM | Anchor::LEFT);
            self.launcher_layer
                .set_margin(0, 0, crate::PANEL_HEIGHT as i32, 8);
            self.launcher_layer.set_exclusive_zone(0);
            self.launcher_layer
                .set_size(crate::LAUNCHER_WIDTH, crate::LAUNCHER_HEIGHT);
            self.launcher_layer
                .set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
            tracing::debug!("launcher focus request: keyboard_interactivity=Exclusive");
            self.launcher_last_signature = None;
            self.launcher_width = crate::LAUNCHER_WIDTH;
            self.launcher_height = crate::LAUNCHER_HEIGHT;
        } else {
            self.launcher_layer
                .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
            tracing::debug!("launcher focus release: keyboard_interactivity=OnDemand");
        }
        // Force a panel re-commit on launcher toggle to avoid stale/missing panel attachment.
        self.panel_last_signature = None;
        self.launcher_dirty = true;
        self.panel_dirty = true;
        tracing::debug!(
            "toggle_launcher: open_before={} open_after={} panel_configured={} launcher_configured={} launcher_size={}x{} panel_dirty={} launcher_dirty={} keyboard_focus={:?}",
            open_before,
            open_after,
            self.panel_configured,
            self.launcher_configured,
            self.launcher_width,
            self.launcher_height,
            self.panel_dirty,
            self.launcher_dirty,
            self.keyboard_focus
        );
    }

    pub(crate) fn handle_panel_click(&mut self, qh: &QueueHandle<Self>, action: ClickAction) {
        match action {
            ClickAction::SwitchWorkspace(workspace) => {
                if self.active_workspace != workspace {
                    debug!(
                        "active workspace changed: old={} new={} (panel click)",
                        self.active_workspace, workspace
                    );
                    self.workspace_indicator_dirty = true;
                }
                self.active_workspace = workspace;
                self.ipc.send(&ShellCommand::SwitchWorkspace { workspace });
                self.draw_panel(qh, RepaintReason::Pointer);
            }
            ClickAction::LaunchApp(index) => {
                self.launcher_state.launch_app(index, &mut self.ipc);
            }
            ClickAction::SelectLauncherCategory(_) => {}
            ClickAction::ToggleLauncher => {
                self.toggle_launcher();
                self.draw_panel(qh, RepaintReason::Pointer);
                if self.launcher_state.open {
                    self.draw_launcher(qh, RepaintReason::Pointer);
                } else {
                    self.unmap_launcher(CommitReason::Input);
                }
            }
        }
    }

    pub(crate) fn handle_launcher_click(&mut self, qh: &QueueHandle<Self>, action: ClickAction) {
        match action {
            ClickAction::LaunchApp(index) => {
                self.launcher_state.set_selected_index(index);
                self.launcher_state
                    .launch_app(self.launcher_state.selected_index, &mut self.ipc);
            }
            ClickAction::SelectLauncherCategory(raw) => {
                if self.launcher_state.set_sidebar_category_from_click(raw) {
                    self.draw_launcher(qh, RepaintReason::Pointer);
                }
            }
            ClickAction::SwitchWorkspace(_) => {}
            ClickAction::ToggleLauncher => {}
        }
    }

    fn update_occupied_workspaces(&mut self) {
        let next = compute_occupied_workspaces(&self.workspace_window_counts);
        if next != self.occupied_workspaces {
            self.occupied_workspaces = next;
            self.workspace_indicator_dirty = true;
            debug!(
                "occupied workspaces recalculated from snapshot: {:?}",
                self.occupied_workspaces
            );
        }
    }

    fn maybe_log_repaint_stats(&mut self, now: Instant) {
        if !self.repaint_stats_enabled {
            return;
        }
        if now.duration_since(self.last_repaint_stats_log) < Duration::from_secs(1) {
            return;
        }
        self.last_repaint_stats_log = now;
        if !self.repaint_stats.has_activity() {
            return;
        }
        info!(
            "shell repaint summary: panel_draws={} launcher_draws={} panel(ipc={} clock={} layer={} pointer={} keyboard={} frame={} other={}) launcher(ipc={} layer={} pointer={} keyboard={} toggle={} frame={} other={})",
            self.repaint_stats.panel_draws,
            self.repaint_stats.launcher_draws,
            self.repaint_stats.panel_ipc,
            self.repaint_stats.panel_clock,
            self.repaint_stats.panel_layer_configure,
            self.repaint_stats.panel_pointer,
            self.repaint_stats.panel_keyboard,
            self.repaint_stats.panel_compositor_frame,
            self.repaint_stats.panel_other,
            self.repaint_stats.launcher_ipc,
            self.repaint_stats.launcher_layer_configure,
            self.repaint_stats.launcher_pointer,
            self.repaint_stats.launcher_keyboard,
            self.repaint_stats.launcher_toggle,
            self.repaint_stats.launcher_compositor_frame,
            self.repaint_stats.launcher_other
        );
        self.repaint_stats.reset();
    }

    fn maybe_log_commit_stats(&mut self, now: Instant) {
        if !self.commit_stats_enabled {
            return;
        }
        if now.duration_since(self.last_commit_stats_log) < Duration::from_secs(1) {
            return;
        }
        self.last_commit_stats_log = now;
        if !self.commit_stats.has_activity() {
            return;
        }
        info!(
            "shell commit summary: total={} panel(initial_create={} configure_ack={} draw_panel={} draw_launcher={} frame_callback={} event_loop_tick={} input={} other={}) launcher(initial_create={} configure_ack={} draw_panel={} draw_launcher={} frame_callback={} event_loop_tick={} input={} other={})",
            self.commit_stats.total(),
            self.commit_stats.panel.initial_create,
            self.commit_stats.panel.configure_ack,
            self.commit_stats.panel.draw_panel,
            self.commit_stats.panel.draw_launcher,
            self.commit_stats.panel.frame_callback,
            self.commit_stats.panel.event_loop_tick,
            self.commit_stats.panel.input,
            self.commit_stats.panel.unknown_other,
            self.commit_stats.launcher.initial_create,
            self.commit_stats.launcher.configure_ack,
            self.commit_stats.launcher.draw_panel,
            self.commit_stats.launcher.draw_launcher,
            self.commit_stats.launcher.frame_callback,
            self.commit_stats.launcher.event_loop_tick,
            self.commit_stats.launcher.input,
            self.commit_stats.launcher.unknown_other
        );
        self.commit_stats.reset();
    }

    fn maybe_log_render_stats(&mut self, now: Instant) {
        if !self.render_stats_enabled {
            return;
        }
        if now.duration_since(self.last_render_stats_log) < Duration::from_secs(1) {
            return;
        }
        self.last_render_stats_log = now;
        if !self.render_stats.has_activity() {
            return;
        }
        info!(
            "shell render summary: panel(renders={} skips={} commits={}) launcher(renders={} skips={} commits={})",
            self.render_stats.panel.renders,
            self.render_stats.panel.skips,
            self.render_stats.panel.commits,
            self.render_stats.launcher.renders,
            self.render_stats.launcher.skips,
            self.render_stats.launcher.commits
        );
        self.render_stats.reset();
    }
}

#[cfg(test)]
mod tests {
    use meridian_config::{GeneralConfig, MeridianConfig, WallpaperConfig, WallpaperMode};
    use meridian_ipc::OutputWorkspaceState;

    use super::{
        apply_full_window_snapshot, apply_output_workspace_changed_state,
        apply_output_workspace_snapshot_state, apply_window_closed_state,
        apply_window_opened_state, apply_workspace_changed, compute_occupied_workspaces,
        panel_theme_signature, resolve_shell_theme_from_config, select_panel_active_workspace,
        WindowInfo, WindowSnapshotEntry,
    };

    #[test]
    fn workspace_changed_clamps_workspace_range() {
        let mut active = 1u8;
        apply_workspace_changed(&mut active, 2);
        assert_eq!(active, 2);
        apply_workspace_changed(&mut active, 99);
        assert_eq!(active, 9);
    }

    #[test]
    fn full_snapshot_recalculates_counts_and_active_workspace() {
        let mut active = 1u8;
        let mut windows = Vec::new();
        let mut counts = [0u16; 9];
        apply_full_window_snapshot(
            &mut active,
            &mut windows,
            &mut counts,
            2,
            vec![
                WindowSnapshotEntry {
                    workspace: 1,
                    id: "id-1".into(),
                    title: "A".into(),
                },
                WindowSnapshotEntry {
                    workspace: 3,
                    id: "id-2".into(),
                    title: "B".into(),
                },
                WindowSnapshotEntry {
                    workspace: 3,
                    id: "id-3".into(),
                    title: "C".into(),
                },
            ],
        );
        assert_eq!(active, 2);
        assert_eq!(counts[0], 1);
        assert_eq!(counts[2], 2);
        assert_eq!(windows.len(), 3);
        let occupied = compute_occupied_workspaces(&counts);
        assert!(occupied[0]);
        assert!(occupied[2]);
        assert!(!occupied[1]);
    }

    #[test]
    fn empty_snapshot_marks_all_workspaces_empty() {
        let mut active = 5u8;
        let mut windows = vec![WindowInfo {
            id: "stale".into(),
            title: "stale".into(),
        }];
        let mut counts = [3u16; 9];
        apply_full_window_snapshot(&mut active, &mut windows, &mut counts, 1, Vec::new());

        assert_eq!(active, 1);
        assert!(windows.is_empty());
        assert_eq!(counts, [0; 9]);
        assert_eq!(compute_occupied_workspaces(&counts), [false; 9]);
    }

    #[test]
    fn snapshot_with_one_window_marks_single_workspace_occupied() {
        let mut active = 1u8;
        let mut windows = Vec::new();
        let mut counts = [0u16; 9];
        apply_full_window_snapshot(
            &mut active,
            &mut windows,
            &mut counts,
            1,
            vec![WindowSnapshotEntry {
                workspace: 1,
                id: "id-1".into(),
                title: "A".into(),
            }],
        );

        let occupied = compute_occupied_workspaces(&counts);
        assert!(occupied[0]);
        assert!(occupied[1..].iter().all(|v| !v));
    }

    #[test]
    fn snapshot_workspace_values_out_of_range_are_clamped_safely() {
        let mut active = 1u8;
        let mut windows = Vec::new();
        let mut counts = [0u16; 9];
        apply_full_window_snapshot(
            &mut active,
            &mut windows,
            &mut counts,
            42, // out of range active workspace
            vec![
                WindowSnapshotEntry {
                    workspace: 0, // underflow case
                    id: "id-1".into(),
                    title: "A".into(),
                },
                WindowSnapshotEntry {
                    workspace: 42, // overflow case
                    id: "id-2".into(),
                    title: "B".into(),
                },
            ],
        );

        assert_eq!(active, 9);
        assert_eq!(counts[0], 1);
        assert_eq!(counts[8], 1);
        let occupied = compute_occupied_workspaces(&counts);
        assert!(occupied[0]);
        assert!(occupied[8]);
        assert!(occupied[1..8].iter().all(|v| !v));
    }

    #[test]
    fn window_opened_updates_or_inserts_without_crash() {
        let mut windows = vec![WindowInfo {
            id: "id-1".into(),
            title: "old".into(),
        }];
        apply_window_opened_state(&mut windows, "id-1".into(), "new".into());
        assert_eq!(windows[0].title, "new");

        apply_window_opened_state(&mut windows, "id-2".into(), "B".into());
        assert_eq!(windows.len(), 2);
    }

    #[test]
    fn window_closed_is_safe_for_unknown_id() {
        let mut windows = vec![WindowInfo {
            id: "id-1".into(),
            title: "A".into(),
        }];
        apply_window_closed_state(&mut windows, "missing");
        assert_eq!(windows.len(), 1);
    }

    #[test]
    fn window_closed_removes_existing_window() {
        let mut windows = vec![WindowInfo {
            id: "id-1".into(),
            title: "A".into(),
        }];
        apply_window_closed_state(&mut windows, "id-1");
        assert!(windows.is_empty());
    }

    #[test]
    fn resolve_shell_theme_from_config_applies_cursor_and_wallpaper_overrides() {
        let mut config = MeridianConfig::default();
        config.general = GeneralConfig {
            theme: "default".to_string(),
        };
        config.cursor = Some(meridian_config::CursorConfig {
            theme: "default".to_string(),
            size: 30,
        });
        config.wallpaper = Some(WallpaperConfig {
            path: "".to_string(),
            mode: WallpaperMode::Tile,
        });

        let (_name, theme) = resolve_shell_theme_from_config(&config).expect("resolve theme");
        assert_eq!(theme.cursor.size, 30);
        assert_eq!(theme.cursor.theme, "default");
        assert_eq!(
            theme.wallpaper.as_ref().map(|w| w.mode),
            Some(WallpaperMode::Tile)
        );
    }

    #[test]
    fn resolve_shell_theme_from_config_fails_for_unknown_theme() {
        let mut config = MeridianConfig::default();
        config.general = GeneralConfig {
            theme: "definitely-not-a-theme".to_string(),
        };
        assert!(resolve_shell_theme_from_config(&config).is_err());
    }

    #[test]
    fn panel_theme_signature_changes_when_theme_changes() {
        let config = MeridianConfig::default();
        let (_name, mut theme) = resolve_shell_theme_from_config(&config).expect("resolve theme");
        let sig_a = panel_theme_signature(&theme);
        theme.colors.accent = meridian_config::Color::rgb(0, 0, 0);
        let sig_b = panel_theme_signature(&theme);
        assert_ne!(sig_a, sig_b);
    }

    #[test]
    fn panel_theme_signature_changes_when_border_changes() {
        let config = MeridianConfig::default();
        let (_name, mut theme) = resolve_shell_theme_from_config(&config).expect("resolve theme");
        let sig_a = panel_theme_signature(&theme);
        theme.colors.border = meridian_config::Color::rgb(0, 0, 0);
        let sig_b = panel_theme_signature(&theme);
        assert_ne!(sig_a, sig_b);
    }

    #[test]
    fn output_workspace_snapshot_with_two_outputs_is_stored() {
        let mut focused_output_id = None;
        let mut output_workspaces = Vec::new();
        let mut output_workspace_state_available = false;
        let mut workspace_indicator_dirty = false;

        apply_output_workspace_snapshot_state(
            &mut focused_output_id,
            &mut output_workspaces,
            &mut output_workspace_state_available,
            &mut workspace_indicator_dirty,
            Some(2),
            vec![
                OutputWorkspaceState {
                    output_id: 1,
                    output_name: Some("eDP-1".to_string()),
                    active_workspace: 2,
                    primary: true,
                    focused: false,
                },
                OutputWorkspaceState {
                    output_id: 2,
                    output_name: Some("HDMI-A-1".to_string()),
                    active_workspace: 4,
                    primary: false,
                    focused: true,
                },
            ],
        );

        assert_eq!(focused_output_id, Some(2));
        assert_eq!(output_workspaces.len(), 2);
        assert!(output_workspace_state_available);
        assert!(workspace_indicator_dirty);
    }

    #[test]
    fn output_workspace_changed_updates_known_output() {
        let mut focused_output_id = Some(1);
        let mut output_workspaces = vec![OutputWorkspaceState {
            output_id: 1,
            output_name: Some("eDP-1".to_string()),
            active_workspace: 1,
            primary: true,
            focused: true,
        }];
        let mut output_workspace_state_available = false;
        let mut workspace_indicator_dirty = false;

        apply_output_workspace_changed_state(
            &mut focused_output_id,
            &mut output_workspaces,
            &mut output_workspace_state_available,
            &mut workspace_indicator_dirty,
            1,
            Some("eDP-1".to_string()),
            3,
            true,
        );

        assert_eq!(output_workspaces.len(), 1);
        assert_eq!(output_workspaces[0].active_workspace, 3);
        assert_eq!(focused_output_id, Some(1));
        assert!(output_workspace_state_available);
        assert!(workspace_indicator_dirty);
    }

    #[test]
    fn output_workspace_changed_unknown_output_is_added_safely() {
        let mut focused_output_id = None;
        let mut output_workspaces = Vec::new();
        let mut output_workspace_state_available = false;
        let mut workspace_indicator_dirty = false;

        apply_output_workspace_changed_state(
            &mut focused_output_id,
            &mut output_workspaces,
            &mut output_workspace_state_available,
            &mut workspace_indicator_dirty,
            7,
            None,
            5,
            false,
        );

        assert_eq!(output_workspaces.len(), 1);
        assert_eq!(output_workspaces[0].output_id, 7);
        assert_eq!(output_workspaces[0].active_workspace, 5);
        assert_eq!(output_workspaces[0].output_name, None);
        assert!(!output_workspaces[0].focused);
        assert!(output_workspace_state_available);
        assert!(workspace_indicator_dirty);
    }

    #[test]
    fn output_workspace_changed_clamps_workspace_and_handles_focus_drop() {
        let mut focused_output_id = Some(3);
        let mut output_workspaces = vec![OutputWorkspaceState {
            output_id: 3,
            output_name: Some("DP-1".to_string()),
            active_workspace: 2,
            primary: false,
            focused: true,
        }];
        let mut output_workspace_state_available = false;
        let mut workspace_indicator_dirty = false;

        apply_output_workspace_changed_state(
            &mut focused_output_id,
            &mut output_workspaces,
            &mut output_workspace_state_available,
            &mut workspace_indicator_dirty,
            3,
            None,
            42,
            false,
        );

        assert_eq!(output_workspaces[0].active_workspace, 9);
        assert_eq!(focused_output_id, None);
        assert!(output_workspace_state_available);
        assert!(workspace_indicator_dirty);
    }

    #[test]
    fn output_workspace_snapshot_clamps_workspace_values() {
        let mut focused_output_id = None;
        let mut output_workspaces = Vec::new();
        let mut output_workspace_state_available = false;
        let mut workspace_indicator_dirty = false;

        apply_output_workspace_snapshot_state(
            &mut focused_output_id,
            &mut output_workspaces,
            &mut output_workspace_state_available,
            &mut workspace_indicator_dirty,
            None,
            vec![OutputWorkspaceState {
                output_id: 1,
                output_name: Some("eDP-1".to_string()),
                active_workspace: 0,
                primary: true,
                focused: false,
            }],
        );

        assert_eq!(output_workspaces[0].active_workspace, 1);
        assert!(output_workspace_state_available);
        assert!(workspace_indicator_dirty);
    }

    #[test]
    fn legacy_workspace_changed_still_works_with_and_without_output_aware_state() {
        let mut active = 1u8;
        apply_workspace_changed(&mut active, 3);
        assert_eq!(active, 3);

        // Legacy update remains valid even if output-aware state exists;
        // this must not mutate output-aware structures.
        let output_workspaces = vec![OutputWorkspaceState {
            output_id: 1,
            output_name: Some("eDP-1".to_string()),
            active_workspace: 2,
            primary: true,
            focused: true,
        }];
        let before = output_workspaces.clone();
        apply_workspace_changed(&mut active, 4);
        assert_eq!(active, 4);
        assert_eq!(output_workspaces, before);
    }

    #[test]
    fn panel_active_workspace_prefers_focused_output_id() {
        let active = select_panel_active_workspace(
            1,
            true,
            Some(2),
            &[
                OutputWorkspaceState {
                    output_id: 1,
                    output_name: Some("eDP-1".to_string()),
                    active_workspace: 3,
                    primary: true,
                    focused: false,
                },
                OutputWorkspaceState {
                    output_id: 2,
                    output_name: Some("HDMI-A-1".to_string()),
                    active_workspace: 5,
                    primary: false,
                    focused: true,
                },
            ],
        );
        assert_eq!(active, 5);
    }

    #[test]
    fn panel_active_workspace_falls_back_to_focused_flag() {
        let active = select_panel_active_workspace(
            1,
            true,
            Some(99),
            &[OutputWorkspaceState {
                output_id: 1,
                output_name: Some("eDP-1".to_string()),
                active_workspace: 4,
                primary: true,
                focused: true,
            }],
        );
        assert_eq!(active, 4);
    }

    #[test]
    fn panel_active_workspace_falls_back_to_primary_output() {
        let active = select_panel_active_workspace(
            2,
            true,
            None,
            &[
                OutputWorkspaceState {
                    output_id: 1,
                    output_name: Some("eDP-1".to_string()),
                    active_workspace: 6,
                    primary: true,
                    focused: false,
                },
                OutputWorkspaceState {
                    output_id: 2,
                    output_name: Some("HDMI-A-1".to_string()),
                    active_workspace: 3,
                    primary: false,
                    focused: false,
                },
            ],
        );
        assert_eq!(active, 6);
    }

    #[test]
    fn panel_active_workspace_falls_back_to_first_output() {
        let active = select_panel_active_workspace(
            2,
            true,
            None,
            &[
                OutputWorkspaceState {
                    output_id: 10,
                    output_name: Some("left".to_string()),
                    active_workspace: 7,
                    primary: false,
                    focused: false,
                },
                OutputWorkspaceState {
                    output_id: 11,
                    output_name: Some("right".to_string()),
                    active_workspace: 1,
                    primary: false,
                    focused: false,
                },
            ],
        );
        assert_eq!(active, 7);
    }

    #[test]
    fn panel_active_workspace_falls_back_to_legacy_when_unavailable() {
        let active = select_panel_active_workspace(
            8,
            false,
            Some(2),
            &[OutputWorkspaceState {
                output_id: 2,
                output_name: Some("HDMI-A-1".to_string()),
                active_workspace: 3,
                primary: false,
                focused: true,
            }],
        );
        assert_eq!(active, 8);
    }

    #[test]
    fn panel_active_workspace_normalizes_out_of_range() {
        let active = select_panel_active_workspace(
            1,
            true,
            Some(1),
            &[OutputWorkspaceState {
                output_id: 1,
                output_name: Some("eDP-1".to_string()),
                active_workspace: 99,
                primary: true,
                focused: true,
            }],
        );
        assert_eq!(active, 9);
    }
}
