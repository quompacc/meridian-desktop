use std::{collections::HashMap, ffi::OsString, time::Instant};

use meridian_config::{KeybindConfig, OutputEntry, ThemeManager};
use meridian_wm::WmWorkspace;
use smithay::{
    desktop::{PopupManager, Window},
    input::{pointer::CursorImageStatus, Seat, SeatState},
    output::Output,
    reexports::calloop::{LoopHandle, LoopSignal},
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    reexports::wayland_server::{
        backend::GlobalId, protocol::wl_surface::WlSurface, DisplayHandle,
    },
    utils::{Logical, Point, Rectangle, Size},
    wayland::{
        compositor::CompositorState,
        dmabuf::{DmabufFeedback, DmabufGlobal, DmabufState},
        drm_syncobj::DrmSyncobjState,
        fractional_scale::FractionalScaleManagerState,
        idle_inhibit::IdleInhibitManagerState,
        idle_notify::IdleNotifierState,
        input_method::InputMethodManagerState,
        output::OutputManagerState,
        presentation::PresentationState,
        seat::WaylandFocus,
        selection::{data_device::DataDeviceState, primary_selection::PrimarySelectionState},
        session_lock::SessionLockManagerState,
        shell::{
            wlr_layer::WlrLayerShellState,
            xdg::{self, XdgShellState},
        },
        shm::ShmState,
        text_input::TextInputManagerState,
        viewporter::ViewporterState,
        xdg_activation::XdgActivationState,
        xwayland_shell::XWaylandShellState,
    },
    xwayland::X11Wm,
};
use wayland_protocols_wlr::output_power_management::v1::server::zwlr_output_power_v1::ZwlrOutputPowerV1;

use smithay::wayland::{
    image_capture_source::{ImageCaptureSourceState, OutputCaptureSourceState},
    image_copy_capture::{Frame as CaptureFrame, ImageCopyCaptureState, Session as CaptureSession},
};

use crate::{
    backend::drm::DrmBackend, decoration::DecorationManager, wallpaper::WallpaperManager,
    workspace::WorkspaceManager,
};

mod client;
mod handlers;
mod idle;
mod ipc;
mod layout;
mod lock;
#[cfg(test)]
mod output_hotplug_tests;
mod output_layout;
mod output_power;
mod output_registry;
#[cfg(test)]
mod session_lock_tests;
mod setup;
mod utils;
mod workspace_output_state;

pub use idle::IdleInhibitorSet;
pub use lock::{LockManager, LockPhase};
pub use output_layout::{
    detect_output_reload_diff, parse_output_transform, ConnectedOutput, OutputLayout,
    OutputPlacement, OutputPosition, OutputReloadDiff, ResolvedOutput,
};
pub use output_power::{OutputPowerManager, OutputPowerMode};
pub use output_registry::{
    OutputGeometry, OutputId, OutputInfo, OutputModeInfo, OutputReconfigure, OutputRegistration,
    OutputRegistry,
};
pub use workspace_output_state::WorkspaceOutputState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaximizeRestoreGeometry {
    pub client_loc: Point<i32, Logical>,
    pub client_size: Option<Size<i32, Logical>>,
}

impl MaximizeRestoreGeometry {
    pub fn new(client_loc: Point<i32, Logical>, client_size: Option<Size<i32, Logical>>) -> Self {
        Self {
            client_loc,
            client_size,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HalfSnapDirection {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowSnapState {
    Half(HalfSnapDirection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HalfSnapRestoreGeometry {
    pub client_loc: Point<i32, Logical>,
    pub client_size: Option<Size<i32, Logical>>,
}

impl HalfSnapRestoreGeometry {
    pub fn new(client_loc: Point<i32, Logical>, client_size: Option<Size<i32, Logical>>) -> Self {
        Self {
            client_loc,
            client_size,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HalfSnapPlacement {
    pub client_loc: Point<i32, Logical>,
    pub client_size: Size<i32, Logical>,
}

#[derive(Debug, Clone)]
pub struct MinimizedWindowEntry {
    pub window: Window,
    pub workspace: usize,
    pub restore_loc: Point<i32, Logical>,
}

#[derive(Debug, Clone)]
pub struct XwaylandOrDiagConfigureRequest {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub w: Option<u32>,
    pub h: Option<u32>,
    pub reorder: Option<String>,
    pub above_hint: Option<u32>,
    pub configure_called: bool,
    pub configure_ok: bool,
    pub configure_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct XwaylandOrDiagConfigureNotify {
    pub geometry: Rectangle<i32, Logical>,
    pub above_hint: Option<u32>,
    pub space_position_changed: bool,
}

#[derive(Debug, Clone)]
pub struct XwaylandOrDiagPointerEvent {
    pub phase: &'static str,
    pub target_window_id: u32,
    pub target_kind: &'static str,
    pub focus_changed: bool,
    pub focus_change_reason: &'static str,
}

#[derive(Debug, Clone)]
pub struct XwaylandOrDiagReleaseCandidate {
    pub window_id: u32,
    pub window_type: Option<String>,
    pub geometry: Rectangle<i32, Logical>,
    pub map_location: Option<Point<i32, Logical>>,
    pub elapsed_since_map_ms: u128,
    pub pointer_inside_geometry: bool,
}

#[derive(Debug, Clone)]
pub struct XwaylandOrDiagReleaseState {
    pub pointer_location: Point<f64, Logical>,
    pub surface_under: Option<String>,
    pub target_kind: &'static str,
    pub target_x11_window_id: Option<u32>,
    pub keyboard_focus: Option<String>,
    pub recent_candidates: Vec<XwaylandOrDiagReleaseCandidate>,
    pub retarget_triggered: bool,
    pub retarget_selected_window_id: Option<u32>,
    pub retarget_reason: String,
    pub final_dispatch_target_kind: &'static str,
}

#[derive(Debug, Clone)]
pub struct XwaylandOrDiagEntry {
    pub window_id: u32,
    pub mapped_window_id: Option<u32>,
    pub announce_at: Instant,
    pub map_at: Option<Instant>,
    pub title: String,
    pub class: String,
    pub instance: String,
    pub window_type: Option<String>,
    pub transient_for: Option<u32>,
    pub transient_for_mapped: Option<u32>,
    pub is_popup: bool,
    pub last_geometry: Rectangle<i32, Logical>,
    pub last_map_location: Option<Point<i32, Logical>>,
    pub last_configure_request: Option<XwaylandOrDiagConfigureRequest>,
    pub last_configure_notify: Option<XwaylandOrDiagConfigureNotify>,
    pub last_pointer_event: Option<XwaylandOrDiagPointerEvent>,
    pub last_release_diag: Option<XwaylandOrDiagReleaseState>,
}

pub(crate) fn remember_maximize_restore_geometry(
    map: &mut HashMap<String, MaximizeRestoreGeometry>,
    window_key: String,
    geometry: MaximizeRestoreGeometry,
) {
    map.entry(window_key).or_insert(geometry);
}

pub(crate) fn take_maximize_restore_geometry(
    map: &mut HashMap<String, MaximizeRestoreGeometry>,
    surface: &WlSurface,
) -> Option<MaximizeRestoreGeometry> {
    map.remove(&window_id(surface))
}

pub(crate) fn restore_client_loc_or_fallback(
    geometry: Option<MaximizeRestoreGeometry>,
    fallback: Point<i32, Logical>,
) -> Point<i32, Logical> {
    geometry.map_or(fallback, |entry| entry.client_loc)
}

pub(crate) fn maximized_client_loc_from_output(
    output_loc: Point<i32, Logical>,
    decoration_offset: (i32, i32),
) -> Point<i32, Logical> {
    Point::from((
        output_loc.x + decoration_offset.0,
        output_loc.y + decoration_offset.1,
    ))
}

// Temporary fixed bottom reservation for normal-window workarea.
// Keep this local so we can later replace it with layer-shell exclusive-zone derived workarea.
pub(crate) const NORMAL_WINDOW_BOTTOM_RESERVED_PX: i32 = 36;

pub(crate) fn normal_window_workarea_from_output_geometry(
    output_geometry: OutputGeometry,
) -> OutputGeometry {
    OutputGeometry {
        x: output_geometry.x,
        y: output_geometry.y,
        width: output_geometry.width,
        height: (output_geometry.height - NORMAL_WINDOW_BOTTOM_RESERVED_PX).max(1),
    }
}

pub(crate) fn normal_window_workarea_from_rect(
    rect: Rectangle<i32, Logical>,
) -> Rectangle<i32, Logical> {
    let workarea = normal_window_workarea_from_output_geometry(OutputGeometry {
        x: rect.loc.x,
        y: rect.loc.y,
        width: rect.size.w,
        height: rect.size.h,
    });
    Rectangle::new(
        (workarea.x, workarea.y).into(),
        (workarea.width, workarea.height).into(),
    )
}

pub(crate) fn half_snap_client_placement_from_output(
    output_geometry: OutputGeometry,
    direction: HalfSnapDirection,
    decoration_offset: (i32, i32),
    decoration_inset: (i32, i32, i32, i32),
) -> HalfSnapPlacement {
    let left_frame_width = output_geometry.width / 2;
    let (frame_x, frame_width) = match direction {
        HalfSnapDirection::Left => (output_geometry.x, left_frame_width),
        HalfSnapDirection::Right => (
            output_geometry.x + left_frame_width,
            output_geometry.width - left_frame_width,
        ),
    };
    let frame_y = output_geometry.y;
    let frame_height = output_geometry.height;
    let (left_inset, top_inset, right_inset, bottom_inset) = decoration_inset;

    HalfSnapPlacement {
        client_loc: Point::from((frame_x + decoration_offset.0, frame_y + decoration_offset.1)),
        client_size: Size::from((
            (frame_width - left_inset - right_inset).max(1),
            (frame_height - top_inset - bottom_inset).max(1),
        )),
    }
}

pub(crate) fn resolve_unmaximize_restore_client_loc(
    geometry: Option<MaximizeRestoreGeometry>,
    decoration_offset: (i32, i32),
) -> (Point<i32, Logical>, bool) {
    let used_fallback = geometry.is_none();
    let fallback_loc = Point::from(decoration_offset);
    (
        restore_client_loc_or_fallback(geometry, fallback_loc),
        used_fallback,
    )
}

pub(crate) fn clear_tiled_toplevel_states(state: &mut smithay::wayland::shell::xdg::ToplevelState) {
    state.states.unset(xdg_toplevel::State::TiledLeft);
    state.states.unset(xdg_toplevel::State::TiledRight);
    state.states.unset(xdg_toplevel::State::TiledTop);
    state.states.unset(xdg_toplevel::State::TiledBottom);
}

#[derive(Debug)]
pub struct ThumbnailRequest {
    pub window_id: String,
    pub max_width: u32,
    pub max_height: u32,
}

pub struct MeridianState {
    pub start_time: Instant,
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, Self>,
    pub loop_signal: LoopSignal,
    pub socket_name: OsString,
    pub seat: Seat<Self>,
    pub workspaces: WorkspaceManager,
    pub outputs: Vec<Output>,
    pub output_layout: OutputLayout,
    pub output_config_entries: Vec<OutputEntry>,
    pub output_registry: OutputRegistry,
    pub workspace_output_state: WorkspaceOutputState,
    pub popups: PopupManager,
    pub theme_manager: ThemeManager,
    pub wallpaper_manager: WallpaperManager,
    pub wm_workspaces: Vec<WmWorkspace>,
    pub ipc: IpcServer,
    pub keybind_config: KeybindConfig,
    pub decoration_manager: DecorationManager,
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub decoration_state: xdg::decoration::XdgDecorationState,
    pub layer_shell_state: WlrLayerShellState,
    pub shm_state: ShmState,
    pub seat_state: SeatState<Self>,
    pub output_manager_state: OutputManagerState,
    pub data_device_state: DataDeviceState,
    pub primary_selection_state: PrimarySelectionState,
    pub xwayland_shell_state: XWaylandShellState,
    pub text_input_manager_state: TextInputManagerState,
    pub input_method_manager_state: InputMethodManagerState,
    pub xdg_activation_state: XdgActivationState,
    // 2026-05-17: Option because the wp_presentation global is intentionally
    // not registered. Plumbing-only mode (global without presented() events
    // in the render loop) makes firefox's caret-blink loop hang because
    // firefox binds the global and waits for vsync events that never come.
    // Re-enable (init = Some(...)) only when render-pipe correctly fires
    // OutputPresentationFeedback::presented(time, refresh, seq, Vsync)
    // with a real monotonic sequence counter and accurate vblank timestamp.
    pub presentation_state: Option<PresentationState>,
    pub fractional_scale_manager_state: FractionalScaleManagerState,
    pub viewporter_state: ViewporterState,
    pub idle_notifier: IdleNotifierState<Self>,
    pub idle_inhibit_state: IdleInhibitManagerState,
    pub idle_inhibitors: IdleInhibitorSet<WlSurface>,
    pub dmabuf_state: DmabufState,
    pub dmabuf_global: Option<DmabufGlobal>,
    pub dmabuf_default_feedback: Option<DmabufFeedback>,
    pub syncobj_state: Option<DrmSyncobjState>,
    pub session_lock_state: SessionLockManagerState,
    pub lock_manager: LockManager,
    pub output_power_manager: OutputPowerManager,
    pub output_power_resources: HashMap<String, Vec<ZwlrOutputPowerV1>>,
    pub output_power_global: GlobalId,
    pub xwm: Option<X11Wm>,
    pub drm_backend: Option<DrmBackend>,
    pub maximize_restore_locations: HashMap<String, MaximizeRestoreGeometry>,
    pub half_snap_restore_locations: HashMap<String, HalfSnapRestoreGeometry>,
    pub active_window_snap_states: HashMap<String, WindowSnapState>,
    pub minimized_windows: HashMap<String, MinimizedWindowEntry>,
    pub xwayland_or_diag: HashMap<u32, XwaylandOrDiagEntry>,
    pub cursor_status: CursorImageStatus,
    pub image_capture_source_state: ImageCaptureSourceState,
    pub output_capture_source_state: OutputCaptureSourceState,
    pub image_copy_capture_state: ImageCopyCaptureState,
    pub screencopy_sessions: Vec<CaptureSession>,
    pub pending_screencopy_frames: Vec<(CaptureFrame, Output)>,
    pub pending_thumbnail_requests: Vec<ThumbnailRequest>,
}

impl MeridianState {
    pub fn resolve_output_layout(&self, connected: &[ConnectedOutput]) -> Vec<ResolvedOutput> {
        let mut resolved = self.output_layout.resolve(connected);
        Self::enforce_at_least_one_enabled(&mut resolved);
        resolved
    }

    pub(crate) fn enforce_at_least_one_enabled(resolved: &mut [ResolvedOutput]) {
        if resolved.is_empty() {
            return;
        }
        if resolved.iter().any(|output| output.enabled) {
            return;
        }

        tracing::warn!(
            "output layout has zero enabled outputs ({} connected) — forcing all to enabled to keep display alive",
            resolved.len()
        );
        for output in resolved.iter_mut() {
            output.enabled = true;
        }

        if !resolved.iter().any(|output| output.primary) {
            if let Some(first) = resolved.first_mut() {
                first.primary = true;
            }
        }
    }

    pub fn clear_window_runtime_state(&mut self, window_key: &str) {
        self.minimized_windows.remove(window_key);
        self.maximize_restore_locations.remove(window_key);
        self.half_snap_restore_locations.remove(window_key);
        self.active_window_snap_states.remove(window_key);
    }

    pub fn keyboard_focus_diag_target(&self) -> Option<String> {
        let keyboard = self.seat.get_keyboard()?;
        let focus_surface = keyboard.current_focus()?;
        let focus_surface_id = window_id(&focus_surface);

        for workspace in 0..self.workspaces.count() {
            if let Some(window) = self
                .workspaces
                .space_at(workspace)
                .elements()
                .find(|window| {
                    window
                        .wl_surface()
                        .map(|surface| surface.into_owned())
                        .as_ref()
                        == Some(&focus_surface)
                })
            {
                if let Some(x11) = window.x11_surface() {
                    return Some(format!(
                        "x11:{} mapped={:?} or={}",
                        x11.window_id(),
                        x11.mapped_window_id(),
                        x11.is_override_redirect()
                    ));
                }
                return Some(format!("wl:{}", focus_surface_id));
            }
        }

        Some(format!("wl:{}", focus_surface_id))
    }
}

pub(crate) use client::ClientState;
pub(crate) use ipc::IpcServer;
pub(crate) use utils::{
    client_compositor_state, toplevel_title, window_app_id, window_id, window_list_entry,
};

#[cfg(test)]
mod tests {
    use smithay::utils::{Logical, Point, Rectangle, Size};

    use super::{
        clear_tiled_toplevel_states, half_snap_client_placement_from_output,
        maximized_client_loc_from_output, normal_window_workarea_from_output_geometry,
        normal_window_workarea_from_rect, remember_maximize_restore_geometry,
        resolve_unmaximize_restore_client_loc, restore_client_loc_or_fallback, HalfSnapDirection,
        MaximizeRestoreGeometry, OutputGeometry, NORMAL_WINDOW_BOTTOM_RESERVED_PX,
    };

    #[test]
    fn capture_known_loc_and_size_preserves_client_size() {
        let mut map = std::collections::HashMap::new();
        let loc: Point<i32, Logical> = (10, 20).into();
        let size: Size<i32, Logical> = (800, 600).into();
        let geometry = MaximizeRestoreGeometry::new(loc, Some(size));
        remember_maximize_restore_geometry(&mut map, "window-a".to_string(), geometry);

        let stored = map.get("window-a").expect("stored geometry");
        assert_eq!(stored.client_loc, loc);
        assert_eq!(stored.client_size, Some(size));
    }

    #[test]
    fn premaximized_entry_allows_missing_client_size() {
        let mut map = std::collections::HashMap::new();
        let loc: Point<i32, Logical> = (2, 34).into();
        let geometry = MaximizeRestoreGeometry::new(loc, None);
        remember_maximize_restore_geometry(&mut map, "window-b".to_string(), geometry);

        let stored = map.get("window-b").expect("stored geometry");
        assert_eq!(stored.client_loc, loc);
        assert_eq!(stored.client_size, None);
    }

    #[test]
    fn existing_entry_is_not_overwritten() {
        let mut map = std::collections::HashMap::new();
        let first = MaximizeRestoreGeometry::new((10, 20).into(), Some((800, 600).into()));
        let second = MaximizeRestoreGeometry::new((30, 40).into(), Some((1024, 768).into()));
        remember_maximize_restore_geometry(&mut map, "window-c".to_string(), first);
        remember_maximize_restore_geometry(&mut map, "window-c".to_string(), second);

        assert_eq!(map.get("window-c"), Some(&first));
    }

    #[test]
    fn missing_restore_entry_uses_fallback_location() {
        let fallback: Point<i32, Logical> = (12, 34).into();
        let resolved = restore_client_loc_or_fallback(None, fallback);
        assert_eq!(resolved, fallback);
    }

    #[test]
    fn maximize_mapping_adds_decoration_offset_to_output_origin() {
        let output_loc: Point<i32, Logical> = (100, 200).into();
        let mapped = maximized_client_loc_from_output(output_loc, (2, 34));
        assert_eq!(mapped, Point::from((102, 234)));
    }

    #[test]
    fn normal_window_workarea_subtracts_bottom_panel_reservation() {
        let output = OutputGeometry {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let workarea = normal_window_workarea_from_output_geometry(output);
        assert_eq!(workarea.x, output.x);
        assert_eq!(workarea.y, output.y);
        assert_eq!(workarea.width, output.width);
        assert_eq!(
            workarea.height,
            output.height - NORMAL_WINDOW_BOTTOM_RESERVED_PX
        );
    }

    #[test]
    fn normal_window_workarea_rect_preserves_origin_and_width() {
        let rect = Rectangle::new((50, 20).into(), (1600, 900).into());
        let workarea = normal_window_workarea_from_rect(rect);
        assert_eq!(workarea.loc, rect.loc);
        assert_eq!(workarea.size.w, rect.size.w);
        assert_eq!(
            workarea.size.h,
            rect.size.h - NORMAL_WINDOW_BOTTOM_RESERVED_PX
        );
    }

    #[test]
    fn unmaximize_restore_uses_stored_geometry_without_fallback() {
        let geometry = Some(MaximizeRestoreGeometry::new(
            (40, 50).into(),
            Some((800, 600).into()),
        ));
        let (resolved, used_fallback) = resolve_unmaximize_restore_client_loc(geometry, (2, 34));
        assert_eq!(resolved, Point::from((40, 50)));
        assert!(!used_fallback);
    }

    #[test]
    fn unmaximize_restore_uses_decoration_offset_when_missing() {
        let (resolved, used_fallback) = resolve_unmaximize_restore_client_loc(None, (2, 34));
        assert_eq!(resolved, Point::from((2, 34)));
        assert!(used_fallback);
    }

    #[test]
    fn clear_tiled_toplevel_states_unsets_only_tiled_bits() {
        use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;

        let mut state = smithay::wayland::shell::xdg::ToplevelState::default();
        state.states.set(xdg_toplevel::State::TiledLeft);
        state.states.set(xdg_toplevel::State::TiledRight);
        state.states.set(xdg_toplevel::State::TiledTop);
        state.states.set(xdg_toplevel::State::TiledBottom);
        state.states.set(xdg_toplevel::State::Maximized);

        clear_tiled_toplevel_states(&mut state);

        assert!(!state.states.contains(xdg_toplevel::State::TiledLeft));
        assert!(!state.states.contains(xdg_toplevel::State::TiledRight));
        assert!(!state.states.contains(xdg_toplevel::State::TiledTop));
        assert!(!state.states.contains(xdg_toplevel::State::TiledBottom));
        assert!(state.states.contains(xdg_toplevel::State::Maximized));
    }

    #[test]
    fn half_snap_left_placement_uses_left_output_half() {
        let output = OutputGeometry {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let placement = half_snap_client_placement_from_output(
            output,
            HalfSnapDirection::Left,
            (0, 0),
            (0, 0, 0, 0),
        );
        assert_eq!(placement.client_loc, Point::from((0, 0)));
        assert_eq!(placement.client_size, Size::from((960, 1080)));
    }

    #[test]
    fn half_snap_right_placement_uses_right_output_half() {
        let output = OutputGeometry {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let placement = half_snap_client_placement_from_output(
            output,
            HalfSnapDirection::Right,
            (0, 0),
            (0, 0, 0, 0),
        );
        assert_eq!(placement.client_loc, Point::from((960, 0)));
        assert_eq!(placement.client_size, Size::from((960, 1080)));
    }

    #[test]
    fn half_snap_nonzero_output_origin_is_preserved() {
        let output = OutputGeometry {
            x: 100,
            y: 50,
            width: 1600,
            height: 900,
        };
        let placement = half_snap_client_placement_from_output(
            output,
            HalfSnapDirection::Left,
            (0, 0),
            (0, 0, 0, 0),
        );
        assert_eq!(placement.client_loc, Point::from((100, 50)));
        assert_eq!(placement.client_size, Size::from((800, 900)));
    }

    #[test]
    fn half_snap_placement_applies_ssd_offset_and_inset() {
        let output = OutputGeometry {
            x: 100,
            y: 50,
            width: 1600,
            height: 900,
        };
        let left = half_snap_client_placement_from_output(
            output,
            HalfSnapDirection::Left,
            (2, 34),
            (2, 34, 2, 2),
        );
        assert_eq!(left.client_loc, Point::from((102, 84)));
        assert_eq!(left.client_size, Size::from((796, 864)));

        let right = half_snap_client_placement_from_output(
            output,
            HalfSnapDirection::Right,
            (2, 34),
            (2, 34, 2, 2),
        );
        assert_eq!(right.client_loc, Point::from((902, 84)));
        assert_eq!(right.client_size, Size::from((796, 864)));
    }

    #[test]
    fn half_snap_odd_output_width_assigns_extra_pixel_to_right_half() {
        let output = OutputGeometry {
            x: 0,
            y: 0,
            width: 1919,
            height: 1080,
        };
        let left = half_snap_client_placement_from_output(
            output,
            HalfSnapDirection::Left,
            (0, 0),
            (0, 0, 0, 0),
        );
        let right = half_snap_client_placement_from_output(
            output,
            HalfSnapDirection::Right,
            (0, 0),
            (0, 0, 0, 0),
        );

        assert_eq!(left.client_size.w, output.width / 2);
        assert_eq!(right.client_size.w, output.width - (output.width / 2));
        assert_eq!(left.client_size.w, 959);
        assert_eq!(right.client_size.w, 960);
        assert_eq!(right.client_size.w, left.client_size.w + 1);
    }
}
