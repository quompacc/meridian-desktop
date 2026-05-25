use std::{cell::RefCell, time::Instant};

use meridian_config::ThemeConfig;
use meridian_ipc::OutputWorkspaceState;
use smithay_client_toolkit::{
    output::OutputState,
    reexports::calloop::channel as cchannel,
    registry::RegistryState,
    seat::SeatState,
    shell::wlr_layer::LayerSurface,
    shm::{
        slot::{Buffer, SlotPool},
        Shm,
    },
};
use wayland_client::protocol::{wl_keyboard, wl_pointer};
use wayland_protocols::ext::{
    image_capture_source::v1::client::ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
    image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1,
};

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
pub(crate) enum CommitReason {
    InitialCreate,
    ConfigureAck,
    DrawPanel,
    DrawLauncher,
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
    pub(crate) colors: [u8; 44],
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
    pub(crate) audio_label: String,
    pub(crate) audio_icon: &'static str,
    pub(crate) status_notifier_items: Vec<String>,
    pub(crate) network_popup_open: bool,
    pub(crate) audio_popup_open: bool,
    pub(crate) hover_widget_path: Option<Vec<usize>>,
    pub(crate) theme: ThemeRenderSignature,
    pub(crate) pinned_apps: Vec<String>,
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
    pub(crate) desktop_layer: LayerSurface,
    pub(crate) desktop_menu_layer: LayerSurface,
    pub(crate) panel: LayerSurface,
    pub(crate) launcher_layer: LayerSurface,
    pub(crate) calendar_layer: LayerSurface,
    pub(crate) workspace_layer: LayerSurface,
    pub(crate) network_layer: LayerSurface,
    pub(crate) notification_layer: LayerSurface,
    pub(crate) thumbnail_layer: LayerSurface,
    pub(crate) desktop_configured: bool,
    pub(crate) desktop_menu_configured: bool,
    pub(crate) panel_configured: bool,
    pub(crate) launcher_configured: bool,
    pub(crate) calendar_configured: bool,
    pub(crate) workspace_configured: bool,
    pub(crate) network_configured: bool,
    pub(crate) notification_configured: bool,
    pub(crate) thumbnail_configured: bool,
    pub(crate) thumbnail_popup_open: bool,
    pub(crate) desktop_menu_open: bool,
    pub(crate) panel_buffer: Option<Buffer>,
    #[allow(dead_code)]
    pub(crate) desktop_buffer: Option<Buffer>,
    pub(crate) desktop_menu_buffer: Option<Buffer>,
    pub(crate) launcher_buffer: Option<Buffer>,
    pub(crate) calendar_buffer: Option<Buffer>,
    pub(crate) workspace_buffer: Option<Buffer>,
    pub(crate) network_buffer: Option<Buffer>,
    pub(crate) notification_buffer: Option<Buffer>,
    pub(crate) thumbnail_buffer: Option<Buffer>,
    pub(crate) pool: SlotPool,
    pub(crate) width: u32,
    pub(crate) desktop_width: u32,
    pub(crate) desktop_height: u32,
    pub(crate) desktop_menu_width: u32,
    pub(crate) desktop_menu_height: u32,
    pub(crate) launcher_width: u32,
    pub(crate) launcher_height: u32,
    pub(crate) launcher_is_fullscreen: bool,
    pub(crate) launcher_visual_x: i32,
    pub(crate) launcher_visual_y: i32,
    pub(crate) calendar_width: u32,
    pub(crate) calendar_height: u32,
    pub(crate) workspace_width: u32,
    pub(crate) workspace_height: u32,
    pub(crate) network_width: u32,
    pub(crate) network_height: u32,
    pub(crate) audio_width: u32,
    pub(crate) audio_height: u32,
    pub(crate) status_notifier_menu_width: u32,
    pub(crate) status_notifier_menu_height: u32,
    pub(crate) notification_width: u32,
    pub(crate) notification_height: u32,
    pub(crate) thumbnail_width: u32,
    pub(crate) thumbnail_height: u32,
    pub(crate) thumbnail_dirty: bool,
    // Hover tracking
    pub(crate) thumbnail_hover_app_idx: Option<usize>,
    pub(crate) thumbnail_hover_since: Option<std::time::Instant>,
    // Current popup window IDs (ordered for display)
    pub(crate) thumbnail_popup_window_ids: Vec<String>,
    // Cache: window_id -> (width, height, xrgb8888 bytes)
    pub(crate) thumbnail_cache: std::collections::HashMap<String, (u32, u32, Vec<u8>)>,
    // Icon center x for the currently open popup, used to recenter on resize
    pub(crate) thumbnail_icon_center: Option<i32>,
    // Power-button arming for confirm-twice destructive actions.
    // Stores button id + arm timestamp. None = no button is armed.
    pub(crate) armed_power: Option<(String, std::time::Instant)>,
    /// Queue of in-flight notifications. v1 renders only the front entry;
    /// stacking + cascade animation is A1.3+ polish.
    pub(crate) notifications: std::collections::VecDeque<crate::notifications::Notification>,
    pub(crate) notification_dirty: bool,
    pub(crate) status_notifier_items: Vec<crate::status_notifier::StatusNotifierItem>,
    pub(crate) status_notifier_tx: Option<cchannel::Sender<crate::status_notifier::DbusEvent>>,
    pub(crate) status_notifier_menu: Option<crate::status_notifier::StatusNotifierMenuState>,
    pub(crate) status_notifier_menu_open: bool,
    pub(crate) status_notifier_menu_entries: Vec<crate::status_notifier::DbusMenuEntry>,
    pub(crate) settings_category: crate::settings_view::SettingsCategory,
    pub(crate) settings_pinned_adding: bool,
    pub(crate) printer_snapshot: crate::printers::PrinterSnapshot,
    pub(crate) audio_snapshot: crate::audio::AudioSnapshot,
    pub(crate) keyboard: Option<wl_keyboard::WlKeyboard>,
    pub(crate) keyboard_focus: SurfaceKind,
    pub(crate) pointer: Option<wl_pointer::WlPointer>,
    pub(crate) pointer_position: (f64, f64),
    pub(crate) pointer_surface: SurfaceKind,
    pub(crate) available_themes: Vec<String>,
    pub(crate) theme_name: String,
    pub(crate) available_wallpapers: Vec<meridian_config::WallpaperEntry>,
    pub(crate) wallpaper_thumbnails: Vec<Option<(u32, u32, Vec<u8>)>>,
    pub(crate) wallpaper_picker_rx: Option<std::sync::mpsc::Receiver<String>>,
    pub(crate) wallpaper_path: Option<String>,
    pub(crate) wallpaper_mode: meridian_config::WallpaperMode,
    pub(crate) theme: ThemeConfig,
    pub(crate) font: RefCell<Option<TextRenderer>>,
    pub(crate) icon_cache: IconCache,
    pub(crate) network_controller: NetworkController,
    pub(crate) ipc: IpcClient,
    pub(crate) panel_state: panel::PanelState,
    pub(crate) pinned_apps: Vec<PinnedApp>,
    pub(crate) launcher_state: launcher::LauncherState,
    pub(crate) workspace_state: WorkspacePopupState,
    pub(crate) workspace_hover_idx: Option<usize>,
    pub(crate) focused_window_id: Option<String>,
    pub(crate) focused_title: Option<String>,
    pub(crate) windows: Vec<WindowInfo>,
    pub(crate) active_workspace: u8,
    pub(crate) focused_output_id: Option<u32>,
    pub(crate) output_workspaces: Vec<OutputWorkspaceState>,
    pub(crate) output_workspace_state_available: bool,
    pub(crate) display_mode_dropdown_open: Option<usize>,
    pub(crate) workspace_window_counts: [u16; 9],
    pub(crate) occupied_workspaces: [bool; 9],
    pub(crate) occupied_state_available: bool,
    pub(crate) workspace_state_received: bool,
    pub(crate) workspace_indicator_dirty: bool,
    pub(crate) workspace_ipc_unavailable_logged: bool,
    pub(crate) occupied_unavailable_logged: bool,
    pub(crate) panel_dirty: bool,
    /// Login->desktop panel entrance: start time (set on the first
    /// configured draw) plus a done-latch so it plays exactly once.
    pub(crate) panel_intro_start: Option<std::time::Instant>,
    pub(crate) panel_intro_done: bool,
    pub(crate) launcher_dirty: bool,
    pub(crate) ui_preview_widget_state: Option<(meridian_ui::WidgetPath, meridian_ui::WidgetState)>,
    pub(crate) panel_widget_state: Option<(meridian_ui::WidgetPath, meridian_ui::WidgetState)>,
    pub(crate) app_view_open: bool,
    pub(crate) hovered_app_card_idx: Option<usize>,
    pub(crate) app_view_scroll_y: i32,
    pub(crate) launcher_settings_open: bool,
    pub(crate) app_view_category: crate::app_view::AppCategory,
    pub(crate) context_menu: Option<crate::context_menu::ContextMenuState>,
    pub(crate) desktop_context_menu: Option<crate::context_menu::DesktopContextMenuState>,
    pub(crate) hidden_execs: std::collections::HashSet<String>,
    pub(crate) search_query: String,
    pub(crate) calendar_dirty: bool,
    pub(crate) workspace_dirty: bool,
    pub(crate) network_dirty: bool,
    pub(crate) audio_dirty: bool,
    pub(crate) calendar_popup_open: bool,
    pub(crate) workspace_popup_open: bool,
    pub(crate) network_popup_open: bool,
    pub(crate) audio_popup_open: bool,
    pub(crate) calendar_display_policy: CalendarDisplayPolicy,
    pub(crate) panel_last_signature: Option<PanelRenderSignature>,
    pub(crate) panel_click_zones_snapshot: Option<String>,
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
    pub(crate) screencopy_manager: Option<ExtImageCopyCaptureManagerV1>,
    pub(crate) capture_source_manager: Option<ExtOutputImageCaptureSourceManagerV1>,
    pub(crate) screenshot_capture: Option<super::screencopy::ScreenshotCapture>,
}
