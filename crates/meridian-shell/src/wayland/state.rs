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

fn normalize_workspace_1_based_u8(workspace: u8) -> u8 {
    workspace.clamp(1, 9)
}

fn apply_workspace_changed(active_workspace: &mut u8, next_workspace_raw: u8) {
    *active_workspace = normalize_workspace_1_based_u8(next_workspace_raw);
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

struct OutputWorkspaceChangedInput {
    output_id: u32,
    output_name: Option<String>,
    workspace: usize,
    focused: bool,
}

fn apply_output_workspace_changed_state(
    focused_output_id: &mut Option<u32>,
    output_workspaces: &mut Vec<OutputWorkspaceState>,
    output_workspace_state_available: &mut bool,
    workspace_indicator_dirty: &mut bool,
    input: OutputWorkspaceChangedInput,
) {
    let OutputWorkspaceChangedInput {
        output_id,
        output_name,
        workspace,
        focused,
    } = input;
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

fn apply_window_opened_state(
    windows: &mut Vec<WindowInfo>,
    id: String,
    title: String,
    workspace: Option<u8>,
) {
    let workspace = workspace.map(normalize_workspace_1_based_u8);
    if let Some(window) = windows.iter_mut().find(|w| w.id == id) {
        window.title = title;
        if let Some(workspace) = workspace {
            window.workspace = workspace;
        }
    } else {
        windows.push(WindowInfo {
            id,
            title,
            workspace: workspace.unwrap_or(1),
            minimized: false,
            app_id: None,
        });
    }
}

fn apply_window_closed_state(windows: &mut Vec<WindowInfo>, id: &str) {
    windows.retain(|w| w.id != id);
}

fn clear_stale_focused_window_id(focused_window_id: &mut Option<String>, windows: &[WindowInfo]) {
    let Some(focused_id) = focused_window_id.as_deref() else {
        return;
    };
    if windows.iter().any(|window| window.id == focused_id) {
        return;
    }
    *focused_window_id = None;
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
            workspace: normalize_workspace_1_based_u8(window.workspace),
            minimized: window.minimized,
            app_id: window.app_id.clone(),
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


pub(crate) fn pinned_app_has_windows_on_workspace(
    app: &crate::panel::PinnedApp,
    windows: &[crate::wayland::types::WindowInfo],
    workspace: u8,
) -> bool {
    let program_base = std::path::Path::new(&app.program)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&app.program)
        .to_lowercase();
    let label_lower = app.label.to_lowercase();
    windows.iter().any(|w| {
        w.workspace == workspace
            && !w.minimized
            && app_matches_window(&program_base, &label_lower, w)
    })
}

pub(crate) fn pinned_app_window_ids(
    app: &crate::panel::PinnedApp,
    windows: &[crate::wayland::types::WindowInfo],
    workspace: u8,
) -> Vec<String> {
    let program_base = std::path::Path::new(&app.program)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&app.program)
        .to_lowercase();
    let label_lower = app.label.to_lowercase();
    windows
        .iter()
        .filter(|w| {
            w.workspace == workspace
                && !w.minimized
                && app_matches_window(&program_base, &label_lower, w)
        })
        .map(|w| w.id.clone())
        .collect()
}

fn app_matches_window(program_base: &str, label_lower: &str, w: &WindowInfo) -> bool {
    if let Some(ref app_id) = w.app_id {
        let aid = app_id.to_lowercase();
        aid == program_base
            || aid.ends_with(&format!(".{}", program_base))
            || aid == label_lower
            || aid.ends_with(&format!(".{}", label_lower))
    } else {
        let t = w.title.to_lowercase();
        (!program_base.is_empty() && t.contains(program_base))
            || (!label_lower.is_empty() && t.contains(label_lower))
    }
}

// Returns (first_unfocused_id, any_window_exists).
// Drives focus-or-launch: focus an unfocused window, or launch when
// the app is already in focus (second click = new instance).
fn pinned_app_window_status(
    app: &crate::panel::PinnedApp,
    windows: &[WindowInfo],
    active_workspace: u8,
    focused_window_id: Option<&str>,
) -> (Option<String>, bool) {
    let program_base = std::path::Path::new(&app.program)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&app.program)
        .to_lowercase();
    let label_lower = app.label.to_lowercase();
    let mut first_unfocused: Option<String> = None;
    let mut any = false;
    for w in windows.iter().filter(|w| w.workspace == active_workspace) {
        if !app_matches_window(&program_base, &label_lower, w) {
            continue;
        }
        any = true;
        if focused_window_id.map_or(true, |fid| fid != w.id) && first_unfocused.is_none() {
            first_unfocused = Some(w.id.clone());
        }
    }
    (first_unfocused, any)
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
                if self.calendar_popup_open {
                    self.draw_calendar_popup(qh, RepaintReason::Clock);
                }
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

        // Thumbnail hover delay: open popup if 400ms passed while hovering a pinned app
        if !self.thumbnail_popup_open {
            if let (Some(idx), Some(since)) = (self.thumbnail_hover_app_idx, self.thumbnail_hover_since) {
                let elapsed = since.elapsed().as_millis();
                if elapsed >= crate::THUMBNAIL_HOVER_DELAY_MS {
                    if let Some(app) = self.pinned_apps.get(idx).cloned() {
                        let ws = self.panel_active_workspace();
                        let window_ids = crate::wayland::state::pinned_app_window_ids(&app, &self.windows, ws);
                        if !window_ids.is_empty() {
                            // Wait until prefetched thumbs land (or open after
                            // timeout) so the popup starts at the correct width
                            // instead of opening at max-placeholder size and
                            // snapping smaller a tick later.
                            let all_cached = window_ids.iter()
                                .take(crate::THUMBNAIL_MAX_WINDOWS)
                                .all(|id| self.thumbnail_cache.contains_key(id.as_str()));
                            let timed_out = elapsed >= crate::THUMBNAIL_OPEN_TIMEOUT_MS;
                            if all_cached || timed_out {
                                let icon_center = self.panel_state.clicks.iter()
                                    .find(|z| matches!(z.action, crate::ClickAction::LaunchPinnedApp(i) if i == idx))
                                    .map(|z| z.rect.x + z.rect.w / 2);
                                self.open_thumbnail_popup(qh, &window_ids, icon_center);
                            }
                        }
                    }
                }
            }
        }

        // Resize + redraw thumbnail popup when new thumbnails arrive
        if self.thumbnail_dirty && self.thumbnail_popup_open {
            self.refresh_thumbnail_popup(qh);
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
                clear_stale_focused_window_id(&mut self.focused_window_id, &self.windows);
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
                    OutputWorkspaceChangedInput {
                        output_id,
                        output_name,
                        workspace,
                        focused,
                    },
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
                apply_window_opened_state(
                    &mut self.windows,
                    id,
                    title,
                    Some(self.active_workspace),
                );
                self.update_focused_title();
            }
            ShellEvent::WindowClosed { id } => {
                apply_window_closed_state(&mut self.windows, &id);
                clear_stale_focused_window_id(&mut self.focused_window_id, &self.windows);
                self.update_focused_title();
            }
            ShellEvent::WindowFocused { id } => {
                self.focused_window_id = Some(id);
                self.update_focused_title();
            }
            ShellEvent::WindowFocusCleared => {
                self.focused_window_id = None;
                self.update_focused_title();
            }
            ShellEvent::ConfigReloaded { success } => {
                debug!("ConfigReloaded {{ success: {} }}", success);
                self.handle_config_reloaded(success);
            }
            ShellEvent::ToggleLauncher => {
                self.toggle_launcher();
            }
            ShellEvent::WindowThumbnail { id, path, width, height } => {
                match std::fs::read(&path) {
                    Ok(data) => {
                        let _ = std::fs::remove_file(&path);
                        tracing::debug!("thumbnail received: id={} {}x{} bytes={}", id, width, height, data.len());
                        self.thumbnail_cache.insert(id, (width, height, data));
                        if self.thumbnail_popup_open {
                            self.thumbnail_dirty = true;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("thumbnail: read failed {}: {}", path, e);
                    }
                }
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
        if !open_before && self.calendar_popup_open {
            self.close_calendar_popup(CommitReason::Input);
        }
        if !open_before && self.workspace_popup_open {
            self.close_workspace_popup(CommitReason::Input);
        }
        if !open_before && self.network_popup_open {
            self.close_network_popup(CommitReason::Input);
        }
        self.launcher_state.toggle();
        let open_after = self.launcher_state.open;
        if self.launcher_state.open {
            // Full-screen transparent surface so outside-clicks reach the shell.
            self.launcher_layer
                .set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
            self.launcher_layer.set_margin(0, 0, 0, 0);
            self.launcher_layer.set_exclusive_zone(-1);
            self.launcher_layer.set_size(0, 0);
            self.launcher_layer
                .set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
            tracing::debug!("launcher focus request: keyboard_interactivity=Exclusive (fullscreen)");
            self.launcher_is_fullscreen = true;
            self.launcher_state.reshuffle();
        } else {
            self.launcher_is_fullscreen = false;
            self.launcher_settings_open = false;
            self.app_view_open = false;
            self.settings_category = crate::settings_view::SettingsCategory::default();
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

    pub(super) fn toggle_calendar_popup(&mut self, reason: CommitReason) {
        if self.calendar_popup_open {
            self.close_calendar_popup(reason);
            return;
        }

        if self.network_popup_open {
            self.close_network_popup(reason);
        }
        if self.workspace_popup_open {
            self.close_workspace_popup(reason);
        }

        if self.launcher_state.open {
            self.launcher_state.close();
            self.launcher_layer
                .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
            self.unmap_launcher(reason);
        }

        self.calendar_popup_open = true;
        self.calendar_layer
            .set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
        self.calendar_layer
            .set_margin(0, 12, crate::SHELL_POPUP_BOTTOM_MARGIN, 0);
        self.calendar_layer.set_exclusive_zone(0);
        self.calendar_layer
            .set_size(crate::CALENDAR_POPUP_WIDTH, crate::CALENDAR_POPUP_HEIGHT);
        self.calendar_layer
            .set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        self.calendar_width = crate::CALENDAR_POPUP_WIDTH;
        self.calendar_height = crate::CALENDAR_POPUP_HEIGHT;
        self.calendar_dirty = true;
        tracing::debug!(
            "toggle_calendar_popup: open_after={} configured={} size={}x{} keyboard_focus={:?}",
            self.calendar_popup_open,
            self.calendar_configured,
            self.calendar_width,
            self.calendar_height,
            self.keyboard_focus
        );
    }

    pub(crate) fn close_calendar_popup(&mut self, reason: CommitReason) -> bool {
        if !self.calendar_popup_open {
            return false;
        }
        self.calendar_popup_open = false;
        self.calendar_layer
            .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        self.unmap_calendar_popup(reason);
        tracing::debug!(
            "close_calendar_popup: open_after={} configured={} keyboard_focus={:?}",
            self.calendar_popup_open,
            self.calendar_configured,
            self.keyboard_focus
        );
        true
    }

    pub(super) fn toggle_workspace_popup(&mut self, reason: CommitReason) {
        if self.workspace_popup_open {
            self.close_workspace_popup(reason);
            return;
        }

        if self.launcher_state.open {
            self.launcher_state.close();
            self.launcher_layer
                .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
            self.unmap_launcher(reason);
        }

        if self.calendar_popup_open {
            self.close_calendar_popup(reason);
        }
        if self.network_popup_open {
            self.close_network_popup(reason);
        }

        self.workspace_popup_open = true;
        self.workspace_layer
            .set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
        self.workspace_layer
            .set_margin(0, 160, crate::SHELL_POPUP_BOTTOM_MARGIN, 0);
        self.workspace_layer.set_exclusive_zone(0);
        self.workspace_layer
            .set_size(crate::WORKSPACE_POPUP_WIDTH, crate::WORKSPACE_POPUP_HEIGHT);
        self.workspace_layer
            .set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        self.workspace_width = crate::WORKSPACE_POPUP_WIDTH;
        self.workspace_height = crate::WORKSPACE_POPUP_HEIGHT;
        self.workspace_dirty = true;
        tracing::debug!(
            "toggle_workspace_popup: open_after={} configured={} size={}x{} keyboard_focus={:?}",
            self.workspace_popup_open,
            self.workspace_configured,
            self.workspace_width,
            self.workspace_height,
            self.keyboard_focus
        );
    }

    pub(crate) fn close_workspace_popup(&mut self, reason: CommitReason) -> bool {
        if !self.workspace_popup_open {
            return false;
        }
        self.workspace_popup_open = false;
        self.workspace_layer
            .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        self.unmap_workspace_popup(reason);
        tracing::debug!(
            "close_workspace_popup: open_after={} configured={} keyboard_focus={:?}",
            self.workspace_popup_open,
            self.workspace_configured,
            self.keyboard_focus
        );
        true
    }

    pub(super) fn toggle_network_popup(&mut self, reason: CommitReason) {
        if self.network_popup_open {
            self.close_network_popup(reason);
            return;
        }

        if self.launcher_state.open {
            self.launcher_state.close();
            self.launcher_layer
                .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
            self.unmap_launcher(reason);
        }
        if self.calendar_popup_open {
            self.close_calendar_popup(reason);
        }
        if self.workspace_popup_open {
            self.close_workspace_popup(reason);
        }

        self.network_popup_open = true;
        self.network_layer
            .set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
        self.network_layer.set_margin(
            0,
            crate::NETWORK_POPUP_RIGHT_MARGIN,
            crate::SHELL_POPUP_BOTTOM_MARGIN,
            0,
        );
        self.network_layer.set_exclusive_zone(0);
        self.network_layer
            .set_size(crate::NETWORK_POPUP_WIDTH, crate::NETWORK_POPUP_HEIGHT);
        self.network_layer
            .set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        self.network_width = crate::NETWORK_POPUP_WIDTH;
        self.network_height = crate::NETWORK_POPUP_HEIGHT;
        self.network_dirty = true;
        tracing::debug!(
            "toggle_network_popup: open_after={} configured={} size={}x{} keyboard_focus={:?}",
            self.network_popup_open,
            self.network_configured,
            self.network_width,
            self.network_height,
            self.keyboard_focus
        );
    }

    pub(crate) fn close_network_popup(&mut self, reason: CommitReason) -> bool {
        if !self.network_popup_open {
            return false;
        }
        self.network_popup_open = false;
        self.network_layer
            .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        self.unmap_network_popup(reason);
        tracing::debug!(
            "close_network_popup: open_after={} configured={} keyboard_focus={:?}",
            self.network_popup_open,
            self.network_configured,
            self.keyboard_focus
        );
        true
    }

    pub(crate) fn open_thumbnail_popup(
        &mut self,
        qh: &QueueHandle<Self>,
        window_ids: &[String],
        icon_center: Option<i32>,
    ) {
        self.thumbnail_popup_window_ids = window_ids.iter()
            .take(crate::THUMBNAIL_MAX_WINDOWS)
            .cloned()
            .collect();
        self.thumbnail_icon_center = icon_center;

        // Always request fresh thumbnails — cached data may reflect a previous
        // window state (size change, content scroll, etc.). Stale cache shows
        // the user an outdated preview which looks like a crop bug.
        for id in window_ids.iter().take(crate::THUMBNAIL_MAX_WINDOWS) {
            let cmd = meridian_ipc::ShellCommand::CaptureWindowThumbnail {
                id: id.clone(),
                max_width: crate::THUMBNAIL_THUMB_W,
                max_height: crate::THUMBNAIL_THUMB_H,
            };
            let _ = self.ipc.send(&cmd);
        }

        let popup_w = crate::thumbnail_popup::popup_width_for(&self.thumbnail_cache, &self.thumbnail_popup_window_ids);
        let left_margin = icon_center
            .map(|c| (c - popup_w as i32 / 2).max(0))
            .unwrap_or(0);

        self.thumbnail_popup_open = true;
        self.thumbnail_layer.set_anchor(
            smithay_client_toolkit::shell::wlr_layer::Anchor::BOTTOM
                | smithay_client_toolkit::shell::wlr_layer::Anchor::LEFT,
        );
        self.thumbnail_layer.set_margin(
            0,
            0,
            crate::SHELL_POPUP_BOTTOM_MARGIN,
            left_margin,
        );
        self.thumbnail_layer.set_exclusive_zone(0);
        self.thumbnail_layer
            .set_size(popup_w, crate::THUMBNAIL_POPUP_HEIGHT);
        self.thumbnail_layer
            .set_keyboard_interactivity(smithay_client_toolkit::shell::wlr_layer::KeyboardInteractivity::None);
        self.thumbnail_width = popup_w;
        self.thumbnail_height = crate::THUMBNAIL_POPUP_HEIGHT;
        self.thumbnail_dirty = true;
        self.draw_thumbnail_popup(qh, crate::wayland::RepaintReason::Pointer);
    }

    /// Recompute popup width from current cache. If size changed, request a
    /// resize from the compositor (which will fire a configure event that
    /// triggers redraw with the actually-granted size). Otherwise just redraw.
    pub(crate) fn refresh_thumbnail_popup(&mut self, qh: &QueueHandle<Self>) {
        if !self.thumbnail_popup_open {
            return;
        }
        let new_w = crate::thumbnail_popup::popup_width_for(&self.thumbnail_cache, &self.thumbnail_popup_window_ids);
        if new_w != self.thumbnail_width {
            let left_margin = self.thumbnail_icon_center
                .map(|c| (c - new_w as i32 / 2).max(0))
                .unwrap_or(0);
            self.thumbnail_layer.set_margin(0, 0, crate::SHELL_POPUP_BOTTOM_MARGIN, left_margin);
            self.thumbnail_layer.set_size(new_w, crate::THUMBNAIL_POPUP_HEIGHT);
            self.thumbnail_width = new_w;
        }
        self.draw_thumbnail_popup(qh, crate::wayland::RepaintReason::Ipc);
    }

    pub(crate) fn close_thumbnail_popup(&mut self, reason: crate::wayland::CommitReason) {
        if !self.thumbnail_popup_open {
            return;
        }
        self.thumbnail_popup_open = false;
        self.thumbnail_popup_window_ids.clear();
        self.unmap_thumbnail_popup(reason);
    }

    pub(crate) fn close_launcher_after_launch(
        &mut self,
        qh: &QueueHandle<Self>,
        reason: RepaintReason,
    ) {
        if !self.launcher_state.open {
            return;
        }
        self.launcher_state.close();
        self.launcher_settings_open = false;
        self.settings_pinned_adding = false;
        self.app_view_open = false;
        self.launcher_layer
            .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        self.unmap_launcher(CommitReason::Input);
        self.draw_panel(qh, reason);
    }

    pub(crate) fn apply_wallpaper(
        &mut self,
        qh: &QueueHandle<Self>,
        path: String,
        mode: meridian_config::WallpaperMode,
    ) {
        meridian_config::MeridianConfig::save_wallpaper(&path, mode);
        self.wallpaper_path = Some(path);
        self.wallpaper_mode = mode;
        self.ipc.send(&meridian_ipc::ShellCommand::ReloadConfig);
        tracing::info!("Wallpaper applied: path={:?} mode={:?}", self.wallpaper_path, mode);
        if self.launcher_settings_open {
            self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
        }
    }

    pub(crate) fn save_pinned_apps(&self) {
        let configs: Vec<meridian_config::PinnedAppConfig> = self.pinned_apps.iter().map(|a| {
            meridian_config::PinnedAppConfig {
                label: a.label.clone(),
                program: a.program.clone(),
                icon: a.icon_name.clone(),
            }
        }).collect();
        meridian_config::MeridianConfig::save_pinned_apps(&configs);
    }


    pub(crate) fn load_wallpaper_thumbnails(&mut self) {
        self.wallpaper_thumbnails = self.available_wallpapers.iter().map(|entry| {
            load_wallpaper_thumbnail(&entry.thumbnail_path, 96, 54)
        }).collect();
        tracing::debug!("loaded {} wallpaper thumbnails", self.wallpaper_thumbnails.len());
    }

    pub(crate) fn spawn_file_picker(&mut self) {
        if self.wallpaper_picker_rx.is_some() { return; }
        let (tx, rx) = std::sync::mpsc::channel();
        self.wallpaper_picker_rx = Some(rx);
        let wayland_display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-1".into());
        std::thread::spawn(move || {
            let out = std::process::Command::new("/usr/bin/zenity")
                .args(["--file-selection", "--title=Choose Wallpaper",
                       "--file-filter=Images | *.jpg *.jpeg *.png *.webp"])
                .env("WAYLAND_DISPLAY", &wayland_display)
                .env("GDK_BACKEND", "wayland")
                .output();
            match out {
                Ok(o) if o.status.success() => {
                    let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if !path.is_empty() { let _ = tx.send(path); }
                }
                Ok(o) => tracing::warn!("zenity exited {:?}: {}", o.status,
                    String::from_utf8_lossy(&o.stderr).trim()),
                Err(e) => tracing::warn!("zenity spawn failed: {}", e),
            }
        });
    }

    pub(crate) fn poll_wallpaper_picker(&mut self) -> Option<String> {
        let rx = self.wallpaper_picker_rx.as_ref()?;
        match rx.try_recv() {
            Ok(path) => { self.wallpaper_picker_rx = None; Some(path) }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => { self.wallpaper_picker_rx = None; None }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
        }
    }
    pub(crate) fn apply_theme(&mut self, qh: &QueueHandle<Self>, name: String) {
        let mut theme_manager = meridian_config::ThemeManager::new();
        if let Err(e) = theme_manager.set_theme(&name) {
            tracing::warn!("apply_theme: failed to load {:?}: {}", name, e);
            return;
        }
        self.theme = theme_manager.current().config.clone();
        self.theme_name = name.clone();
        meridian_config::MeridianConfig::save_theme(&name);
        tracing::info!("Theme applied: {}", name);
        self.panel_dirty = true;
        self.draw_panel(qh, crate::wayland::RepaintReason::Pointer);
        if self.launcher_settings_open {
            self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
        }
    }

    pub(crate) fn handle_panel_click(&mut self, qh: &QueueHandle<Self>, action: ClickAction) {
        if self.calendar_popup_open && !matches!(action, ClickAction::Clock) {
            self.close_calendar_popup(CommitReason::Input);
        }
        if self.workspace_popup_open && !matches!(action, ClickAction::ToggleWorkspacePopup) {
            self.close_workspace_popup(CommitReason::Input);
        }
        if self.network_popup_open && !matches!(action, ClickAction::ToggleNetworkPopup) {
            self.close_network_popup(CommitReason::Input);
        }

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
            ClickAction::FocusWindow(id) => {
                self.ipc.send(&ShellCommand::FocusWindow { id });
            }
            ClickAction::LaunchPinnedApp(idx) => {
                if let Some(app) = self.pinned_apps.get(idx).cloned() {
                    let ws = self.panel_active_workspace();
                    let (unfocused_id, any_window) = pinned_app_window_status(&app, &self.windows, ws, self.focused_window_id.as_deref());
                    if let Some(id) = unfocused_id {
                        // A window exists but is not focused: bring it to front.
                        self.ipc.send(&ShellCommand::FocusWindow { id });
                    } else if any_window {
                        // All windows are already focused: open a new instance.
                        let command = ShellCommand::LaunchApp {
                            program: app.program,
                            args: app.args,
                            terminal: app.terminal,
                        };
                        let _ = self.ipc.send(&command);
                    } else {
                        // No window at all: launch.
                        let command = ShellCommand::LaunchApp {
                            program: app.program,
                            args: app.args,
                            terminal: app.terminal,
                        };
                        if !self.ipc.send(&command) {
                            tracing::warn!("IPC unavailable, pinned app launch skipped: {}", idx);
                        }
                    }
                }
            }
            ClickAction::LaunchApp(index) => {
                self.launcher_state.launch_app(index, &mut self.ipc);
            }
            ClickAction::LauncherAction { action, .. } => {
                let _result = self.launcher_state.trigger_action(action, &mut self.ipc);
            }
            ClickAction::SetLauncherView(_) => {}
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
            ClickAction::ToggleWorkspacePopup => {
                self.toggle_workspace_popup(CommitReason::Input);
                self.draw_panel(qh, RepaintReason::Pointer);
                if self.workspace_popup_open {
                    self.draw_workspace_popup(qh, RepaintReason::Pointer);
                }
            }
            ClickAction::ToggleNetworkPopup => {
                self.toggle_network_popup(CommitReason::Input);
                self.draw_panel(qh, RepaintReason::Pointer);
                if self.network_popup_open {
                    self.draw_network_popup(qh, RepaintReason::Pointer);
                }
            }
            ClickAction::Clock => {
                self.toggle_calendar_popup(CommitReason::Input);
                self.draw_panel(qh, RepaintReason::Pointer);
                if self.calendar_popup_open {
                    self.draw_calendar_popup(qh, RepaintReason::Pointer);
                }
            }
            ClickAction::TakeScreenshot => {
                if self.screenshot_capture.is_some() {
                    return;
                }
                let (Some(mgr), Some(src_mgr)) = (
                    self.screencopy_manager.as_ref(),
                    self.capture_source_manager.as_ref(),
                ) else {
                    tracing::warn!("screenshot: ext_image_copy_capture not available");
                    return;
                };

                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                let dir = std::path::PathBuf::from(&home)
                    .join("Pictures")
                    .join("Screenshots");
                let _ = std::fs::create_dir_all(&dir);
                let secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let path = dir.join(format!("meridian-{}.png", secs));

                let Some(wl_output) = self.output_state.outputs().next() else {
                    tracing::warn!("screenshot: no output available");
                    return;
                };

                use wayland_protocols::ext::{
                    image_capture_source::v1::client::ext_image_capture_source_v1::ExtImageCaptureSourceV1,
                    image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::Options,
                };
                let capture_source: ExtImageCaptureSourceV1 =
                    src_mgr.create_source(&wl_output, qh, ());
                let session = mgr.create_session(
                    &capture_source,
                    Options::empty(),
                    qh,
                    (),
                );
                capture_source.destroy();

                self.screenshot_capture =
                    Some(crate::wayland::screencopy::ScreenshotCapture {
                        session,
                        path,
                        width: 0,
                        height: 0,
                        format: None,
                        constraints_done: false,
                        pool: None,
                        buffer: None,
                        frame: None,
                        fd: None,
                        mapped_ptr: std::ptr::null_mut(),
                        mapped_len: 0,
                    });
            }
            ClickAction::ToggleSettings => {
                self.launcher_settings_open = true;
                self.app_view_open = false;
                self.toggle_launcher();
            }
        }
    }

    pub(crate) fn handle_workspace_click(&mut self, qh: &QueueHandle<Self>, action: ClickAction) {
        if let ClickAction::SwitchWorkspace(workspace) = action {
            if self.active_workspace != workspace {
                debug!(
                    "active workspace changed: old={} new={} (workspace popup click)",
                    self.active_workspace, workspace
                );
                self.workspace_indicator_dirty = true;
            }
            self.active_workspace = workspace;
            self.ipc.send(&ShellCommand::SwitchWorkspace { workspace });
            self.close_workspace_popup(CommitReason::Input);
            self.draw_panel(qh, RepaintReason::Pointer);
        }
    }

    pub(crate) fn handle_launcher_click(&mut self, qh: &QueueHandle<Self>, action: ClickAction) {
        match action {
            ClickAction::LaunchApp(index) => {
                if self.launcher_state.view() == crate::launcher::LauncherView::TileStart {
                    self.launcher_state
                        .launch_app_by_app_index(index, &mut self.ipc);
                } else {
                    self.launcher_state.set_selected_index(index);
                    self.launcher_state
                        .launch_app(self.launcher_state.selected_index, &mut self.ipc);
                }
                self.close_launcher_after_launch(qh, RepaintReason::Pointer);
            }
            ClickAction::LaunchPinnedApp(_) => {}
            ClickAction::LauncherAction { action, index } => {
                self.launcher_state.set_selected_index(index);
                match self.launcher_state.trigger_action(action, &mut self.ipc) {
                    crate::launcher::LauncherActionTriggerResult::Armed => {
                        self.draw_launcher(qh, RepaintReason::Pointer);
                    }
                    crate::launcher::LauncherActionTriggerResult::Sent => {
                        self.close_launcher_after_launch(qh, RepaintReason::Pointer);
                    }
                    crate::launcher::LauncherActionTriggerResult::Failed => {
                        self.draw_launcher(qh, RepaintReason::Pointer);
                    }
                }
            }
            ClickAction::SelectLauncherCategory(raw) => {
                if self.launcher_state.set_sidebar_category_from_click(raw) {
                    self.draw_launcher(qh, RepaintReason::Pointer);
                }
            }
            ClickAction::SetLauncherView(view) => {
                if self.launcher_state.set_view(view) {
                    self.draw_launcher(qh, RepaintReason::Pointer);
                }
            }
            ClickAction::FocusWindow(_) => {}
            ClickAction::SwitchWorkspace(_) => {}
            ClickAction::ToggleLauncher => {}
            ClickAction::ToggleWorkspacePopup => {}
            ClickAction::ToggleNetworkPopup => {}
            ClickAction::Clock => {}
            ClickAction::TakeScreenshot => {}
            ClickAction::ToggleSettings => {
                self.launcher_settings_open = true;
                self.app_view_open = false;
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
        }
    }

    fn update_occupied_workspaces(&mut self) {
        let next = compute_occupied_workspaces(&self.workspace_window_counts);
        if next != self.occupied_workspaces {
            self.occupied_workspaces = next;
            self.workspace_indicator_dirty = true;
            if self.workspace_popup_open {
                self.workspace_dirty = true;
            }
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
        apply_window_opened_state, apply_workspace_changed, clear_stale_focused_window_id,
        compute_occupied_workspaces, panel_theme_signature, resolve_shell_theme_from_config,
        select_panel_active_workspace, OutputWorkspaceChangedInput, WindowInfo,
        WindowSnapshotEntry,
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
                    minimized: false,
                    app_id: None,
                },
                WindowSnapshotEntry {
                    workspace: 3,
                    id: "id-2".into(),
                    title: "B".into(),
                    minimized: false,
                    app_id: None,
                },
                WindowSnapshotEntry {
                    workspace: 3,
                    id: "id-3".into(),
                    title: "C".into(),
                    minimized: true,
                    app_id: None,
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
            workspace: 1,
            minimized: false,
            app_id: None,
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
                minimized: false,
                app_id: None,
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
                    minimized: false,
                    app_id: None,
                },
                WindowSnapshotEntry {
                    workspace: 42, // overflow case
                    id: "id-2".into(),
                    title: "B".into(),
                    minimized: true,
                    app_id: None,
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
            workspace: 2,
            minimized: false,
            app_id: None,
        }];
        apply_window_opened_state(&mut windows, "id-1".into(), "new".into(), None);
        assert_eq!(windows[0].title, "new");
        assert_eq!(windows[0].workspace, 2);

        apply_window_opened_state(&mut windows, "id-2".into(), "B".into(), Some(3));
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[1].workspace, 3);
    }

    #[test]
    fn window_closed_is_safe_for_unknown_id() {
        let mut windows = vec![WindowInfo {
            id: "id-1".into(),
            title: "A".into(),
            workspace: 1,
            minimized: false,
            app_id: None,
        }];
        apply_window_closed_state(&mut windows, "missing");
        assert_eq!(windows.len(), 1);
    }

    #[test]
    fn window_closed_removes_existing_window() {
        let mut windows = vec![WindowInfo {
            id: "id-1".into(),
            title: "A".into(),
            workspace: 1,
            minimized: false,
            app_id: None,
        }];
        apply_window_closed_state(&mut windows, "id-1");
        assert!(windows.is_empty());
    }

    #[test]
    fn full_snapshot_preserves_workspace_on_window_entries() {
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
                    minimized: false,
                    app_id: None,
                },
                WindowSnapshotEntry {
                    workspace: 3,
                    id: "id-2".into(),
                    title: "B".into(),
                    minimized: true,
                    app_id: None,
                },
            ],
        );
        assert_eq!(windows[0].workspace, 1);
        assert_eq!(windows[1].workspace, 3);
        assert!(!windows[0].minimized);
        assert!(windows[1].minimized);
    }

    #[test]
    fn stale_focused_window_id_is_cleared_when_no_window_matches() {
        let windows = vec![WindowInfo {
            id: "id-1".into(),
            title: "A".into(),
            workspace: 1,
            minimized: false,
            app_id: None,
        }];
        let mut focused = Some("missing".to_string());
        clear_stale_focused_window_id(&mut focused, &windows);
        assert_eq!(focused, None);
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
            OutputWorkspaceChangedInput {
                output_id: 1,
                output_name: Some("eDP-1".to_string()),
                workspace: 3,
                focused: true,
            },
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
            OutputWorkspaceChangedInput {
                output_id: 7,
                output_name: None,
                workspace: 5,
                focused: false,
            },
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
            OutputWorkspaceChangedInput {
                output_id: 3,
                output_name: None,
                workspace: 42,
                focused: false,
            },
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


fn load_wallpaper_thumbnail(path: &str, max_w: u32, max_h: u32) -> Option<(u32, u32, Vec<u8>)> {
    let img = image::open(path).ok()?;
    let thumb = img.thumbnail(max_w, max_h);
    let rgba = thumb.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let premul: Vec<u8> = rgba.into_raw().chunks_exact(4).flat_map(|c| {
        let a = c[3] as u16;
        [((c[0] as u16 * a) / 255) as u8,
         ((c[1] as u16 * a) / 255) as u8,
         ((c[2] as u16 * a) / 255) as u8,
         c[3]]
    }).collect();
    Some((w, h, premul))
}
