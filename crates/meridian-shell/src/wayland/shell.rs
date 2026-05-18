use std::{cell::RefCell, time::Instant};

use meridian_config::ThemeConfig;
use meridian_ipc::OutputWorkspaceState;
use smithay_client_toolkit::{
    output::OutputState,
    registry::RegistryState,
    seat::SeatState,
    shell::wlr_layer::LayerSurface,
    shm::{
        slot::{Buffer, SlotPool},
        Shm,
    },
};
use wayland_client::protocol::{wl_keyboard, wl_pointer};

use crate::{
    icons::IconCache, launcher, network::NetworkController, panel, panel::PanelWindowEntry,
    panel::PinnedApp, workspaces::WorkspacePopupState, TextRenderer,
};

use super::{calendar::CalendarDisplayPolicy, types::WindowInfo, IpcClient, SurfaceKind};

#[derive(Clone, Copy, Debug)]
pub(crate) enum RepaintReason {
    Ipc,
    Clock,
    LayerConfigure,
    Pointer,
    Keyboard,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum CommitSurfaceKind {
    Panel,
    Launcher,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(crate) enum CommitReason {
    InitialCreate,
    ConfigureAck,
    DrawPanel,
    DrawLauncher,
    FrameCallback,
    EventLoopTick,
    Input,
    UnknownOther,
}

#[derive(Default)]
pub(crate) struct CommitReasonCounts {
    pub(crate) initial_create: u64,
    pub(crate) configure_ack: u64,
    pub(crate) draw_panel: u64,
    pub(crate) draw_launcher: u64,
    pub(crate) frame_callback: u64,
    pub(crate) event_loop_tick: u64,
    pub(crate) input: u64,
    pub(crate) unknown_other: u64,
}

impl CommitReasonCounts {
    pub(crate) fn record(&mut self, reason: CommitReason) {
        match reason {
            CommitReason::InitialCreate => self.initial_create += 1,
            CommitReason::ConfigureAck => self.configure_ack += 1,
            CommitReason::DrawPanel => self.draw_panel += 1,
            CommitReason::DrawLauncher => self.draw_launcher += 1,
            CommitReason::FrameCallback => self.frame_callback += 1,
            CommitReason::EventLoopTick => self.event_loop_tick += 1,
            CommitReason::Input => self.input += 1,
            CommitReason::UnknownOther => self.unknown_other += 1,
        }
    }

    pub(crate) fn total(&self) -> u64 {
        self.initial_create
            + self.configure_ack
            + self.draw_panel
            + self.draw_launcher
            + self.frame_callback
            + self.event_loop_tick
            + self.input
            + self.unknown_other
    }
}

#[derive(Default)]
pub(crate) struct CommitStats {
    pub(crate) panel: CommitReasonCounts,
    pub(crate) launcher: CommitReasonCounts,
}

impl CommitStats {
    pub(crate) fn record(&mut self, surface_kind: CommitSurfaceKind, reason: CommitReason) {
        match surface_kind {
            CommitSurfaceKind::Panel => self.panel.record(reason),
            CommitSurfaceKind::Launcher => self.launcher.record(reason),
        }
    }

    pub(crate) fn total(&self) -> u64 {
        self.panel.total() + self.launcher.total()
    }

    pub(crate) fn has_activity(&self) -> bool {
        self.total() > 0
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Default)]
pub(crate) struct RepaintStats {
    pub(crate) panel_draws: u64,
    pub(crate) launcher_draws: u64,
    pub(crate) panel_ipc: u64,
    pub(crate) panel_clock: u64,
    pub(crate) panel_layer_configure: u64,
    pub(crate) panel_pointer: u64,
    pub(crate) panel_keyboard: u64,
    pub(crate) panel_compositor_frame: u64,
    pub(crate) panel_other: u64,
    pub(crate) launcher_ipc: u64,
    pub(crate) launcher_layer_configure: u64,
    pub(crate) launcher_pointer: u64,
    pub(crate) launcher_keyboard: u64,
    pub(crate) launcher_toggle: u64,
    pub(crate) launcher_compositor_frame: u64,
    pub(crate) launcher_other: u64,
}

impl RepaintStats {
    pub(crate) fn record_panel(&mut self, reason: RepaintReason) {
        self.panel_draws += 1;
        match reason {
            RepaintReason::Ipc => self.panel_ipc += 1,
            RepaintReason::Clock => self.panel_clock += 1,
            RepaintReason::LayerConfigure => self.panel_layer_configure += 1,
            RepaintReason::Pointer => self.panel_pointer += 1,
            RepaintReason::Keyboard => self.panel_keyboard += 1,
        }
    }

    pub(crate) fn record_launcher(&mut self, reason: RepaintReason) {
        self.launcher_draws += 1;
        match reason {
            RepaintReason::Ipc => self.launcher_ipc += 1,
            RepaintReason::LayerConfigure => self.launcher_layer_configure += 1,
            RepaintReason::Pointer => self.launcher_pointer += 1,
            RepaintReason::Keyboard => self.launcher_keyboard += 1,
            RepaintReason::Clock => self.launcher_other += 1,
        }
    }

    pub(crate) fn has_activity(&self) -> bool {
        self.panel_draws > 0 || self.launcher_draws > 0
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ThemeRenderSignature {
    pub(crate) font_ui: String,
    pub(crate) colors: [u8; 20],
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PanelRenderSignature {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) active_workspace: u8,
    pub(crate) occupied_state_available: bool,
    pub(crate) occupied_workspaces: [bool; 9],
    pub(crate) focused_title: Option<String>,
    pub(crate) window_entries: Vec<PanelWindowEntry>,
    pub(crate) clock: String,
    pub(crate) network_icon: &'static str,
    pub(crate) network_popup_open: bool,
    pub(crate) theme: ThemeRenderSignature,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct LauncherRenderSignature {
    pub(crate) open: bool,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) query: String,
    pub(crate) mode: crate::launcher::LauncherMode,
    pub(crate) view: crate::launcher::LauncherView,
    pub(crate) app_sections_hash: u64,
    pub(crate) hover_app_index: Option<usize>,
    pub(crate) tile_scroll_y: i32,
    pub(crate) sidebar_category: crate::launcher::SidebarCategory,
    pub(crate) pending_action_confirmation: Option<crate::launcher::LauncherAction>,
    pub(crate) selected_index: usize,
    pub(crate) visible_apps_len: usize,
    pub(crate) visible_apps_hash: u64,
    pub(crate) theme: ThemeRenderSignature,
}

#[derive(Default)]
pub(crate) struct SurfaceRenderStats {
    pub(crate) renders: u64,
    pub(crate) skips: u64,
    pub(crate) commits: u64,
}

#[derive(Default)]
pub(crate) struct ShellRenderStats {
    pub(crate) panel: SurfaceRenderStats,
    pub(crate) launcher: SurfaceRenderStats,
}

impl ShellRenderStats {
    pub(crate) fn has_activity(&self) -> bool {
        self.panel.renders > 0
            || self.panel.skips > 0
            || self.panel.commits > 0
            || self.launcher.renders > 0
            || self.launcher.skips > 0
            || self.launcher.commits > 0
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

pub(crate) struct MeridianShell {
    pub(crate) registry_state: RegistryState,
    pub(crate) seat_state: SeatState,
    pub(crate) output_state: OutputState,
    pub(crate) shm: Shm,
    pub(crate) panel: LayerSurface,
    pub(crate) launcher_layer: LayerSurface,
    pub(crate) calendar_layer: LayerSurface,
    pub(crate) workspace_layer: LayerSurface,
    pub(crate) network_layer: LayerSurface,
    pub(crate) panel_configured: bool,
    pub(crate) launcher_configured: bool,
    pub(crate) calendar_configured: bool,
    pub(crate) workspace_configured: bool,
    pub(crate) network_configured: bool,
    pub(crate) panel_buffer: Option<Buffer>,
    pub(crate) launcher_buffer: Option<Buffer>,
    pub(crate) calendar_buffer: Option<Buffer>,
    pub(crate) workspace_buffer: Option<Buffer>,
    pub(crate) network_buffer: Option<Buffer>,
    pub(crate) pool: SlotPool,
    pub(crate) width: u32,
    pub(crate) launcher_width: u32,
    pub(crate) launcher_height: u32,
    pub(crate) calendar_width: u32,
    pub(crate) calendar_height: u32,
    pub(crate) workspace_width: u32,
    pub(crate) workspace_height: u32,
    pub(crate) network_width: u32,
    pub(crate) network_height: u32,
    pub(crate) keyboard: Option<wl_keyboard::WlKeyboard>,
    pub(crate) keyboard_focus: SurfaceKind,
    pub(crate) pointer: Option<wl_pointer::WlPointer>,
    pub(crate) pointer_position: (f64, f64),
    pub(crate) pointer_surface: SurfaceKind,
    pub(crate) theme_name: String,
    pub(crate) theme: ThemeConfig,
    pub(crate) font: RefCell<Option<TextRenderer>>,
    pub(crate) icon_cache: IconCache,
    pub(crate) network_controller: NetworkController,
    pub(crate) ipc: IpcClient,
    pub(crate) panel_state: panel::PanelState,
    pub(crate) pinned_apps: Vec<PinnedApp>,
    pub(crate) launcher_state: launcher::LauncherState,
    pub(crate) workspace_state: WorkspacePopupState,
    pub(crate) focused_window_id: Option<String>,
    pub(crate) focused_title: Option<String>,
    pub(crate) windows: Vec<WindowInfo>,
    pub(crate) active_workspace: u8,
    pub(crate) focused_output_id: Option<u32>,
    pub(crate) output_workspaces: Vec<OutputWorkspaceState>,
    pub(crate) output_workspace_state_available: bool,
    pub(crate) workspace_window_counts: [u16; 9],
    pub(crate) occupied_workspaces: [bool; 9],
    pub(crate) occupied_state_available: bool,
    pub(crate) workspace_state_received: bool,
    pub(crate) workspace_indicator_dirty: bool,
    pub(crate) workspace_ipc_unavailable_logged: bool,
    pub(crate) occupied_unavailable_logged: bool,
    pub(crate) panel_dirty: bool,
    pub(crate) launcher_dirty: bool,
    pub(crate) calendar_dirty: bool,
    pub(crate) workspace_dirty: bool,
    pub(crate) network_dirty: bool,
    pub(crate) calendar_popup_open: bool,
    pub(crate) workspace_popup_open: bool,
    pub(crate) network_popup_open: bool,
    pub(crate) calendar_display_policy: CalendarDisplayPolicy,
    pub(crate) panel_last_signature: Option<PanelRenderSignature>,
    pub(crate) launcher_last_signature: Option<LauncherRenderSignature>,
    pub(crate) repaint_stats: RepaintStats,
    pub(crate) repaint_stats_enabled: bool,
    pub(crate) last_repaint_stats_log: Instant,
    pub(crate) commit_stats: CommitStats,
    pub(crate) commit_stats_enabled: bool,
    pub(crate) last_commit_stats_log: Instant,
    pub(crate) render_stats: ShellRenderStats,
    pub(crate) render_stats_enabled: bool,
    pub(crate) last_render_stats_log: Instant,
    pub(crate) commit_info_until: Instant,
    pub(crate) last_clock: String,
    pub(crate) last_tick: Instant,
    pub(crate) exit: bool,
}

#[cfg(test)]
mod tests {
    use super::{LauncherRenderSignature, ThemeRenderSignature};
    use crate::launcher::{LauncherAction, LauncherMode, LauncherView, SidebarCategory};

    fn base_launcher_signature() -> LauncherRenderSignature {
        LauncherRenderSignature {
            open: true,
            width: 720,
            height: 520,
            query: String::new(),
            mode: LauncherMode::Apps,
            view: LauncherView::TileStart,
            app_sections_hash: 0,
            hover_app_index: None,
            tile_scroll_y: 0,
            sidebar_category: SidebarCategory::System,
            pending_action_confirmation: None,
            selected_index: 0,
            visible_apps_len: 1,
            visible_apps_hash: 42,
            theme: ThemeRenderSignature {
                font_ui: "Sans".to_string(),
                colors: [0; 20],
            },
        }
    }

    #[test]
    fn launcher_signature_changes_with_pending_action_confirmation() {
        let without_confirmation = base_launcher_signature();
        let mut with_confirmation = base_launcher_signature();
        with_confirmation.pending_action_confirmation = Some(LauncherAction::ExitMeridian);

        assert_ne!(without_confirmation, with_confirmation);
    }

    #[test]
    fn launcher_signature_changes_with_view() {
        let tile_start = base_launcher_signature();
        let mut all_apps = base_launcher_signature();
        all_apps.view = LauncherView::AllApps;

        assert_ne!(tile_start, all_apps);
    }

    #[test]
    fn launcher_signature_changes_with_hover_app_index() {
        let without = base_launcher_signature();
        let mut with = base_launcher_signature();
        with.hover_app_index = Some(7);
        assert_ne!(without, with);
    }

    #[test]
    fn launcher_signature_changes_with_app_sections_hash() {
        let zero = base_launcher_signature();
        let mut other = base_launcher_signature();
        other.app_sections_hash = 0xdead_beef;
        assert_ne!(zero, other);
    }

    #[test]
    fn launcher_signature_changes_with_tile_scroll() {
        let a = base_launcher_signature();
        let mut b = base_launcher_signature();
        b.tile_scroll_y = 80;
        assert_ne!(a, b);
    }
}
