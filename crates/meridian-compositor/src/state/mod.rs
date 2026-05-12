use std::{collections::HashMap, ffi::OsString, time::Instant};

use meridian_config::{KeybindConfig, ThemeManager};
use meridian_wm::WmWorkspace;
use smithay::{
    desktop::PopupManager,
    input::{Seat, SeatState},
    output::Output,
    reexports::calloop::{LoopHandle, LoopSignal},
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
}

pub(crate) use client::ClientState;
pub(crate) use ipc::IpcServer;
pub(crate) use utils::{client_compositor_state, toplevel_title, window_id};

#[cfg(test)]
mod tests {
    use smithay::utils::{Logical, Point, Size};

    use super::{
        remember_maximize_restore_geometry, restore_client_loc_or_fallback, MaximizeRestoreGeometry,
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
}
