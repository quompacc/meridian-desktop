use std::time::{Duration, Instant};

use meridian_config::{MeridianConfig, ThemeConfig, ThemeManager};
use meridian_ipc::{OutputWorkspaceState, ShellCommand, ShellEvent, WindowSnapshotEntry};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity};
use smithay_client_toolkit::shell::WaylandSurface;
use tracing::{debug, info};
use wayland_client::QueueHandle;

use crate::{launcher, status_notifier, status_notifier_popup};

use super::{
    time, types::WindowInfo, ClickAction, CommitReason, CommitSurfaceKind, MeridianShell,
    RepaintReason,
};

fn workspace_idx(workspace: u8) -> usize {
    workspace.saturating_sub(1).min(8) as usize
}

fn normalize_workspace_1_based_u8(workspace: u8) -> u8 {
    workspace.clamp(1, 9)
}

fn apply_workspace_changed(active_workspace: &mut u8, next_workspace_raw: u8) {
    *active_workspace = normalize_workspace_1_based_u8(next_workspace_raw);
}

fn panel_global_activation_point(
    pointer_position: (f64, f64),
    output_height: Option<i32>,
) -> status_notifier::ActivationPoint {
    let x = pointer_position.0.round() as i32;
    let local_y = pointer_position.1.round() as i32;
    let y = output_height
        .map(|height| {
            height
                .saturating_sub(crate::PANEL_SURFACE_HEIGHT as i32)
                .saturating_add(local_y)
        })
        .unwrap_or(local_y);
    status_notifier::ActivationPoint { x, y }
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
            ..Default::default()
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

fn panel_theme_signature(theme: &ThemeConfig) -> (String, [u8; 44]) {
    let colors = [
        theme.colors.background,
        theme.colors.surface,
        theme.colors.surface_alt,
        theme.colors.accent,
        theme.colors.accent_alt,
        theme.colors.text,
        theme.colors.text_dim,
        theme.colors.border,
        theme.colors.error,
        theme.colors.warning,
        theme.colors.success,
    ];
    let mut bytes = [0; 44];
    for (idx, color) in colors.iter().enumerate() {
        let offset = idx * 4;
        bytes[offset] = color.r;
        bytes[offset + 1] = color.g;
        bytes[offset + 2] = color.b;
        bytes[offset + 3] = color.a;
    }
    (theme.fonts.ui.clone(), bytes)
}

fn resolve_shell_theme_from_config(
    config: &MeridianConfig,
) -> Result<(String, ThemeConfig, Vec<String>), String> {
    let mut theme_manager = ThemeManager::new();
    let requested_theme = if config.general.theme.trim().is_empty() {
        "dark"
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
        theme_manager.available_themes(),
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
    windows
        .iter()
        .any(|w| w.workspace == workspace && app_matches_window(&program_base, &label_lower, w))
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
        .filter(|w| w.workspace == workspace && app_matches_window(&program_base, &label_lower, w))
        .map(|w| w.id.clone())
        .collect()
}

struct WallpaperPickerCommand {
    program: &'static str,
    args: &'static [&'static str],
}

fn wallpaper_picker_command() -> WallpaperPickerCommand {
    const MERIDIAN_ARGS: &[&str] = &["--title", "Choose Wallpaper"];
    const ZENITY_ARGS: &[&str] = &[
        "--file-selection",
        "--title=Choose Wallpaper",
        "--file-filter=Images | *.jpg *.jpeg *.png *.webp",
    ];

    if std::path::Path::new("/usr/local/bin/meridian-file-picker").is_file() {
        return WallpaperPickerCommand {
            program: "/usr/local/bin/meridian-file-picker",
            args: MERIDIAN_ARGS,
        };
    }
    WallpaperPickerCommand {
        program: "/usr/bin/zenity",
        args: ZENITY_ARGS,
    }
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
fn first_minimized_pinned_app_window_id(
    app: &crate::panel::PinnedApp,
    windows: &[WindowInfo],
    active_workspace: u8,
) -> Option<String> {
    let program_base = std::path::Path::new(&app.program)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&app.program)
        .to_lowercase();
    let label_lower = app.label.to_lowercase();

    windows
        .iter()
        .find(|w| {
            w.workspace == active_workspace
                && w.minimized
                && app_matches_window(&program_base, &label_lower, w)
        })
        .map(|w| w.id.clone())
}

include!("state/timers.rs");
include!("state/ipc_events.rs");
include!("state/popups.rs");
include!("state/audio_and_network_popups.rs");
include!("state/shell_actions.rs");
include!("state/panel_actions.rs");

fn hidden_apps_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    format!("{home}/.config/meridian/hidden_apps.txt")
}

fn load_wallpaper_thumbnail(path: &str, max_w: u32, max_h: u32) -> Option<(u32, u32, Vec<u8>)> {
    let img = image::open(path).ok()?;
    let thumb = img.thumbnail(max_w, max_h);
    let rgba = thumb.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let premul: Vec<u8> = rgba
        .into_raw()
        .chunks_exact(4)
        .flat_map(|c| {
            let a = c[3] as u16;
            [
                ((c[0] as u16 * a) / 255) as u8,
                ((c[1] as u16 * a) / 255) as u8,
                ((c[2] as u16 * a) / 255) as u8,
                c[3],
            ]
        })
        .collect();
    Some((w, h, premul))
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
