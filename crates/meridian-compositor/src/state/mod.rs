use std::{collections::HashMap, ffi::OsString, time::Instant};

use meridian_config::{KeybindConfig, ThemeManager};
use meridian_wm::WmWorkspace;
use smithay::{
    desktop::PopupManager,
    input::{Seat, SeatState},
    output::Output,
    reexports::calloop::{LoopHandle, LoopSignal},
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    reexports::wayland_server::{protocol::wl_surface::WlSurface, DisplayHandle},
    utils::{Logical, Point, Size},
    wayland::{
        compositor::CompositorState,
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::{
            wlr_layer::WlrLayerShellState,
            xdg::{self, XdgShellState},
        },
        shm::ShmState,
        xwayland_shell::XWaylandShellState,
    },
    xwayland::X11Wm,
};

use crate::{
    backend::drm::DrmBackend, decoration::DecorationManager, wallpaper::WallpaperManager,
    workspace::WorkspaceManager,
};

mod client;
mod handlers;
mod ipc;
mod layout;
mod output_registry;
mod setup;
mod utils;
mod workspace_output_state;

pub use output_registry::{
    OutputGeometry, OutputId, OutputInfo, OutputReconfigure, OutputRegistration, OutputRegistry,
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

#[cfg_attr(not(test), allow(dead_code))]
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

pub struct MeridianState {
    pub start_time: Instant,
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, Self>,
    pub loop_signal: LoopSignal,
    pub socket_name: OsString,
    pub seat: Seat<Self>,
    pub workspaces: WorkspaceManager,
    pub outputs: Vec<Output>,
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
    pub xwayland_shell_state: XWaylandShellState,
    pub xwm: Option<X11Wm>,
    pub drm_backend: Option<DrmBackend>,
    pub maximize_restore_locations: HashMap<String, MaximizeRestoreGeometry>,
    pub half_snap_restore_locations: HashMap<String, HalfSnapRestoreGeometry>,
    pub active_window_snap_states: HashMap<String, WindowSnapState>,
}

pub(crate) use client::ClientState;
pub(crate) use ipc::IpcServer;
pub(crate) use utils::{client_compositor_state, toplevel_title, window_id};

#[cfg(test)]
mod tests {
    use smithay::utils::{Logical, Point, Size};

    use super::{
        clear_tiled_toplevel_states, half_snap_client_placement_from_output,
        maximized_client_loc_from_output, remember_maximize_restore_geometry,
        resolve_unmaximize_restore_client_loc, restore_client_loc_or_fallback, HalfSnapDirection,
        MaximizeRestoreGeometry, OutputGeometry,
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
