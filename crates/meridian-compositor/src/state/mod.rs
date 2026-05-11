use std::{collections::HashMap, ffi::OsString, time::Instant};

use meridian_config::{KeybindConfig, ThemeManager};
use meridian_wm::WmWorkspace;
use smithay::{
    desktop::PopupManager,
    input::{Seat, SeatState},
    output::Output,
    reexports::calloop::{LoopHandle, LoopSignal},
    reexports::wayland_server::DisplayHandle,
    utils::{Logical, Point},
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
    pub maximize_restore_locations: HashMap<String, Point<i32, Logical>>,
}

pub(crate) use client::ClientState;
pub(crate) use ipc::IpcServer;
pub(crate) use utils::{client_compositor_state, toplevel_title, window_id};
