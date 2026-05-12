use std::{ffi::OsString, io::Error, sync::Arc, time::Instant};

use meridian_config::{MeridianConfig, ThemeManager};
use meridian_wm::WmWorkspace;
use smithay::{
    desktop::PopupManager,
    input::SeatState,
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, Mode, PostAction},
        wayland_server::Display,
    },
    wayland::{
        compositor::CompositorState,
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::{wlr_layer::WlrLayerShellState, xdg::XdgShellState},
        shm::ShmState,
        socket::ListeningSocketSource,
        xwayland_shell::XWaylandShellState,
    },
};

use crate::{
    decoration::DecorationManager, wallpaper::WallpaperManager, workspace::WorkspaceManager,
};

use super::{
    ClientState, IpcServer, MeridianState, OutputGeometry, OutputId, OutputReconfigure,
    OutputRegistration, OutputRegistry, WorkspaceOutputState,
};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ThemeOverrideChanges {
    pub theme_changed: bool,
    pub cursor_changed: bool,
    pub wallpaper_changed: bool,
}

pub(crate) fn apply_config_overrides(
    theme_manager: &mut ThemeManager,
    meridian_config: &MeridianConfig,
) -> ThemeOverrideChanges {
    let prev_theme_name = theme_manager.current().name.clone();
    let prev_cursor = (
        theme_manager.current().config.cursor.theme.clone(),
        theme_manager.current().config.cursor.size,
    );
    let prev_wallpaper = theme_manager
        .current()
        .config
        .wallpaper
        .as_ref()
        .map(|w| (w.path.clone(), w.mode));

    let requested_theme = if meridian_config.general.theme.trim().is_empty() {
        "default"
    } else {
        meridian_config.general.theme.trim()
    };
    if let Err(err) = theme_manager.set_theme(requested_theme) {
        tracing::warn!(
            "Failed to load theme {:?} from config: {} — keeping current theme {:?}",
            requested_theme,
            err,
            theme_manager.current().name
        );
    }

    if let Some(cursor) = &meridian_config.cursor {
        theme_manager.current_mut().config.cursor.theme = cursor.theme.clone();
        theme_manager.current_mut().config.cursor.size = cursor.size;
    }
    if meridian_config.wallpaper.is_some() {
        theme_manager.current_mut().config.wallpaper = meridian_config.wallpaper_override();
    }

    ThemeOverrideChanges {
        theme_changed: prev_theme_name != theme_manager.current().name,
        cursor_changed: prev_cursor
            != (
                theme_manager.current().config.cursor.theme.clone(),
                theme_manager.current().config.cursor.size,
            ),
        wallpaper_changed: prev_wallpaper
            != theme_manager
                .current()
                .config
                .wallpaper
                .as_ref()
                .map(|w| (w.path.clone(), w.mode)),
    }
}

impl MeridianState {
    pub fn mark_all_outputs_dirty(&mut self, reason: &str) {
        let Some(drm) = self.drm_backend.as_mut() else {
            return;
        };
        let mut marked = 0usize;
        for output in drm.outputs.iter_mut() {
            drm.dirty_stats
                .record_dirty_mark_event(output.output_id, reason);
            if !output.needs_repaint {
                output.needs_repaint = true;
                drm.dirty_stats.record_dirty_set(output.output_id);
                marked += 1;
            }
        }
        if marked > 0 {
            tracing::trace!(
                "marked all outputs dirty: reason={} count={}",
                reason,
                marked
            );
        }
    }

    pub fn mark_output_dirty(&mut self, output_id: OutputId, reason: &str) {
        let Some(drm) = self.drm_backend.as_mut() else {
            return;
        };
        drm.dirty_stats.record_dirty_mark_event(output_id, reason);
        if let Some(output) = drm
            .outputs
            .iter_mut()
            .find(|output| output.output_id == output_id)
        {
            if !output.needs_repaint {
                output.needs_repaint = true;
                drm.dirty_stats.record_dirty_set(output_id);
                tracing::trace!(
                    "marked output dirty: reason={} output_id={} output={}",
                    reason,
                    output_id.0,
                    output.output.name()
                );
            }
        }
    }

    pub fn mark_output_dirty_by_name(&mut self, output_name: &str, reason: &str) {
        let Some(drm) = self.drm_backend.as_mut() else {
            return;
        };
        if let Some(output) = drm
            .outputs
            .iter_mut()
            .find(|output| output.output.name() == output_name)
        {
            drm.dirty_stats
                .record_dirty_mark_event(output.output_id, reason);
            if !output.needs_repaint {
                output.needs_repaint = true;
                drm.dirty_stats.record_dirty_set(output.output_id);
                tracing::trace!(
                    "marked output dirty: reason={} output_id={} output={}",
                    reason,
                    output.output_id.0,
                    output_name
                );
            }
        }
    }

    pub fn new(
        event_loop: &mut EventLoop<'static, Self>,
        display: Display<Self>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let display_handle = display.handle();

        let compositor_state = CompositorState::new::<Self>(&display_handle);
        let xdg_shell_state = XdgShellState::new::<Self>(&display_handle);
        let decoration_state = smithay::wayland::shell::xdg::decoration::XdgDecorationState::new::<
            Self,
        >(&display_handle);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&display_handle);
        let shm_state = ShmState::new::<Self>(&display_handle, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&display_handle);
        let data_device_state = DataDeviceState::new::<Self>(&display_handle);
        let xwayland_shell_state = XWaylandShellState::new::<Self>(&display_handle);

        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(&display_handle, "seat-0");
        seat.add_keyboard(Default::default(), 200, 25).unwrap();
        seat.add_pointer();

        let loop_handle = event_loop.handle();
        let socket_name = Self::init_wayland_listener(display, event_loop)?;
        let loop_signal = event_loop.get_signal();

        let meridian_config = MeridianConfig::load();
        let mut theme_manager = ThemeManager::new();
        let _ = apply_config_overrides(&mut theme_manager, &meridian_config);

        let mut wallpaper_manager = WallpaperManager::new();
        wallpaper_manager.apply_theme(theme_manager.current());

        Ok(Self {
            start_time: Instant::now(),
            display_handle,
            loop_handle,
            loop_signal,
            socket_name,
            seat,
            workspaces: WorkspaceManager::new(),
            outputs: Vec::new(),
            output_registry: OutputRegistry::new(),
            workspace_output_state: WorkspaceOutputState::default(),
            popups: PopupManager::default(),
            theme_manager,
            wallpaper_manager,
            wm_workspaces: (0..9).map(|_| WmWorkspace::new()).collect(),
            ipc: IpcServer::new(),
            keybind_config: meridian_config.keybinds,
            decoration_manager: DecorationManager::new(),
            compositor_state,
            xdg_shell_state,
            decoration_state,
            layer_shell_state,
            shm_state,
            seat_state,
            output_manager_state,
            data_device_state,
            xwayland_shell_state,
            xwm: None,
            drm_backend: None,
            maximize_restore_locations: std::collections::HashMap::new(),
            half_snap_restore_locations: std::collections::HashMap::new(),
            active_window_snap_states: std::collections::HashMap::new(),
        })
    }

    fn post_output_state_change(
        &mut self,
        action: &str,
        output_id: Option<OutputId>,
        output_name: Option<&str>,
    ) {
        self.sync_outputs_with_workspace_state();
        self.mark_all_outputs_dirty("output-state-change");
        tracing::debug!(
            "output hotplug state changed: action={} output_id={:?} output_name={:?}",
            action,
            output_id.map(|id| id.0),
            output_name
        );
        self.reconcile_layer_shell_outputs_after_output_change(action, output_name);
        tracing::debug!(
            "layer-shell recovery reconciled after output change: action={} output_id={:?} output_name={:?}",
            action,
            output_id.map(|id| id.0),
            output_name
        );
        self.broadcast_output_workspace_snapshot();
        tracing::debug!(
            "output workspace snapshot broadcasted after output change: action={} output_id={:?} output_name={:?}",
            action,
            output_id.map(|id| id.0),
            output_name
        );
    }

    pub fn handle_output_added_or_updated(&mut self, registration: OutputRegistration) -> OutputId {
        let existed = self.output_registry.contains_name(&registration.name);
        let id = self.output_registry.upsert(registration.clone());
        if let Some(info) = self.output_registry.by_id(id) {
            tracing::info!(
                "output {}: id={} name={} primary={} geometry=({},{} {}x{}) scale={} transform={:?} refresh={:?}",
                if existed { "reconfigured" } else { "registered" },
                info.id.0,
                info.name,
                info.primary,
                info.geometry.x,
                info.geometry.y,
                info.geometry.width,
                info.geometry.height,
                info.scale,
                info.transform,
                info.refresh_millihz
            );
        } else {
            tracing::info!(
                "output {} fallback: id={} name={} geometry=({},{} {}x{})",
                if existed {
                    "reconfigured"
                } else {
                    "registered"
                },
                id.0,
                registration.name,
                registration.geometry.x,
                registration.geometry.y,
                registration.geometry.width,
                registration.geometry.height
            );
        }
        if let Some(primary) = self.output_registry.primary() {
            tracing::debug!(
                "output primary/fallback: id={} name={}",
                primary.id.0,
                primary.name
            );
        }
        self.post_output_state_change(
            if existed {
                "output-updated"
            } else {
                "output-added"
            },
            Some(id),
            Some(&registration.name),
        );
        id
    }

    pub fn handle_output_removed(&mut self, id: OutputId) -> bool {
        let Some(removed) = self.output_registry.remove_by_id(id) else {
            return false;
        };
        tracing::info!(
            "output removed: id={} name={} geometry=({},{} {}x{})",
            removed.id.0,
            removed.name,
            removed.geometry.x,
            removed.geometry.y,
            removed.geometry.width,
            removed.geometry.height
        );
        self.post_output_state_change("output-removed", Some(id), Some(&removed.name));
        true
    }

    pub fn handle_output_reconfigured(
        &mut self,
        id: OutputId,
        reconfigure: OutputReconfigure,
    ) -> bool {
        if !self.output_registry.reconfigure_by_id(id, reconfigure) {
            return false;
        }
        if let Some(info) = self.output_registry.by_id(id) {
            tracing::info!(
                "output reconfigured: id={} name={} primary={} geometry=({},{} {}x{}) scale={} transform={:?} refresh={:?}",
                info.id.0,
                info.name,
                info.primary,
                info.geometry.x,
                info.geometry.y,
                info.geometry.width,
                info.geometry.height,
                info.scale,
                info.transform,
                info.refresh_millihz
            );
        }
        let output_name = self.output_registry.by_id(id).map(|info| info.name.clone());
        self.post_output_state_change("output-reconfigured", Some(id), output_name.as_deref());
        true
    }

    pub fn register_output_info(&mut self, registration: OutputRegistration) -> OutputId {
        self.handle_output_added_or_updated(registration)
    }

    pub fn output_geometry_for_registry(x: i32, y: i32, width: i32, height: i32) -> OutputGeometry {
        OutputGeometry {
            x,
            y,
            width,
            height,
        }
    }

    fn init_wayland_listener(
        display: Display<Self>,
        event_loop: &mut EventLoop<Self>,
    ) -> Result<OsString, Box<dyn std::error::Error>> {
        let listening_socket = ListeningSocketSource::new_auto().map_err(|err| {
            Error::other(format!("failed to create wayland listening socket: {err}"))
        })?;
        let socket_name = listening_socket.socket_name().to_os_string();
        let loop_handle = event_loop.handle();

        loop_handle
            .insert_source(listening_socket, move |client_stream, _, state| {
                if let Err(err) = state
                    .display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                {
                    tracing::warn!("failed to insert wayland client: {}", err);
                }
            })
            .map_err(|err| {
                Error::other(format!("failed to initialize wayland socket source: {err}"))
            })?;

        loop_handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    unsafe { display.get_mut().dispatch_clients(state) }.map_err(|err| {
                        Error::other(format!("failed to dispatch wayland clients: {err}"))
                    })?;
                    Ok(PostAction::Continue)
                },
            )
            .map_err(|err| {
                Error::other(format!("failed to insert display into event loop: {err}"))
            })?;

        Ok(socket_name)
    }
}
