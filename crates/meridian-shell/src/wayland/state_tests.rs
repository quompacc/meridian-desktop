use meridian_config::{GeneralConfig, MeridianConfig, WallpaperConfig, WallpaperMode};
use meridian_ipc::OutputWorkspaceState;

use super::{
    apply_full_window_snapshot, apply_output_workspace_changed_state,
    apply_output_workspace_snapshot_state, apply_window_closed_state, apply_window_opened_state,
    apply_workspace_changed, clear_stale_focused_window_id, compute_occupied_workspaces,
    panel_global_activation_point, panel_theme_signature, resolve_shell_theme_from_config,
    select_local_capture_output_name, select_panel_active_workspace, OutputWorkspaceChangedInput,
    WindowInfo, WindowSnapshotEntry,
};

include!("state_tests/workspaces.rs");
include!("state_tests/output_events.rs");
