use std::{
    ffi::OsString,
    fs,
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::net::{UnixListener, UnixStream},
    process::Command,
    sync::Arc,
    time::Instant,
};

use meridian_ipc::{ShellCommand, ShellEvent};
use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_dispatch2,
    desktop::{
        layer_map_for_output, LayerSurface as DesktopLayerSurface, PopupKind, PopupManager, Window,
        WindowSurfaceType,
    },
    input::{
        dnd::DndGrabHandler,
        pointer::{CursorImageStatus, Focus, GrabStartData as PointerGrabStartData},
        Seat, SeatHandler, SeatState,
    },
    output::Output,
    reexports::{
        calloop::{
            generic::Generic, EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction,
        },
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            backend::{ClientId, DisconnectReason},
            protocol::{
                wl_buffer::WlBuffer, wl_output::WlOutput, wl_seat::WlSeat, wl_surface::WlSurface,
            },
            Client, Display, DisplayHandle, Resource,
        },
    },
    utils::{Logical, Point, Rectangle, Serial, SERIAL_COUNTER},
    wayland::{
        buffer::BufferHandler,
        compositor::{
            get_parent, is_sync_subsurface, with_states, CompositorClientState, CompositorHandler,
            CompositorState,
        },
        output::{OutputHandler, OutputManagerState},
        selection::{
            data_device::{DataDeviceHandler, DataDeviceState, WaylandDndGrabHandler},
            SelectionHandler,
        },
        shell::{
            wlr_layer::{
                Layer as WlrLayer, LayerSurface as WlrLayerSurface, LayerSurfaceData,
                WlrLayerShellHandler, WlrLayerShellState,
            },
            xdg::{
                PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
                XdgToplevelSurfaceData,
            },
        },
        shm::{ShmHandler, ShmState},
        socket::ListeningSocketSource,
        xwayland_shell::XWaylandShellState,
    },
    xwayland::{X11Wm, XWaylandClientData},
};

use meridian_config::{KeybindConfig, MeridianConfig, ThemeManager};
use meridian_wm::{WmWorkspace, WorkspaceMode};
use smithay::wayland::seat::WaylandFocus;

use crate::{
    backend::drm::DrmBackend,
    grabs::{
        move_grab::MoveSurfaceGrab,
        resize_grab::{ResizeEdge, ResizeSurfaceGrab},
    },
    protocols::xdg_shell::handle_commit,
    workspace::WorkspaceManager,
};

impl MeridianState {
    // ── Tiling ────────────────────────────────────────────────────────────────

    /// Return the currently keyboard-focused Window, if any.
    pub fn focused_window(&self) -> Option<Window> {
        let surf = self.seat.get_keyboard()?.current_focus()?;
        self.workspaces
            .active_space()
            .elements()
            .find(|w| w.toplevel().map_or(false, |t| t.wl_surface() == &surf))
            .cloned()
    }

    /// Apply the BSP layout for workspace `idx` to the underlying Space.
    /// Removes stale windows from the tree, sends configure events, and
    /// repositions tiles.
    pub fn tile_workspace(&mut self, idx: usize) {
        let output_rect = self
            .outputs
            .first()
            .and_then(|o| self.workspaces.space_at(idx).output_geometry(o))
            .unwrap_or_else(|| Rectangle::new((0, 0).into(), (1920, 1080).into()));
        let gap = self.theme_manager.current().config.decorations.gap as i32;

        // Remove windows from the BSP tree that are no longer in the Space
        let space_windows: Vec<Window> =
            self.workspaces.space_at(idx).elements().cloned().collect();
        for w in self.wm_workspaces[idx].tiling.windows() {
            if !space_windows.iter().any(|sw| sw == &w) {
                self.wm_workspaces[idx].tiling.remove(&w);
            }
        }

        let assignments = self.wm_workspaces[idx].compute_tiled(output_rect, gap);
        if assignments.is_empty() {
            return;
        }

        let space = self.workspaces.space_at_mut(idx);
        for (window, rect) in assignments {
            if let Some(toplevel) = window.toplevel() {
                toplevel.with_pending_state(|state| {
                    state.size = Some(rect.size);
                    state.states.set(xdg_toplevel::State::TiledLeft);
                    state.states.set(xdg_toplevel::State::TiledRight);
                    state.states.set(xdg_toplevel::State::TiledTop);
                    state.states.set(xdg_toplevel::State::TiledBottom);
                });
                toplevel.send_pending_configure();
            }
            space.map_element(window, rect.loc, false);
        }
    }

    /// Toggle tiling mode for the active workspace.
    pub fn toggle_tiling(&mut self) {
        let active = self.workspaces.active;
        let new_mode = self.wm_workspaces[active].toggle_mode();
        if new_mode == WorkspaceMode::Tiling {
            // Rebuild BSP tree from currently mapped windows
            let windows: Vec<Window> = self.workspaces.active_space().elements().cloned().collect();
            self.wm_workspaces[active].rebuild_tiling_from(windows.into_iter());
            self.tile_workspace(active);
        }
        tracing::info!("Workspace {} → {:?}", active, new_mode);
    }

    // ── Existing helpers ──────────────────────────────────────────────────────

    pub fn surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        let output = self
            .outputs
            .iter()
            .find(|output| {
                self.workspaces
                    .active_space()
                    .output_geometry(output)
                    .map(|geo| geo.to_f64().contains(pos))
                    .unwrap_or(false)
            })
            .or_else(|| self.outputs.first());

        if let Some(output) = output {
            let output_geo = self.workspaces.active_space().output_geometry(output)?;
            let layer_map = layer_map_for_output(output);
            let local = pos - output_geo.loc.to_f64();

            for layer in [WlrLayer::Overlay, WlrLayer::Top] {
                if let Some(surface) = layer_map.layer_under(layer, local) {
                    if let Some(geo) = layer_map.layer_geometry(surface) {
                        return surface
                            .surface_under(local - geo.loc.to_f64(), WindowSurfaceType::ALL)
                            .map(|(s, p)| (s, (p + output_geo.loc + geo.loc).to_f64()));
                    }
                }
            }
        }

        let window_surface =
            self.workspaces
                .active_space()
                .element_under(pos)
                .and_then(|(window, location)| {
                    window
                        .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                        .map(|(s, p)| (s, (p + location).to_f64()))
                });

        if window_surface.is_some() {
            return window_surface;
        }

        if let Some(output) = output {
            let output_geo = self.workspaces.active_space().output_geometry(output)?;
            let layer_map = layer_map_for_output(output);
            let local = pos - output_geo.loc.to_f64();

            for layer in [WlrLayer::Bottom, WlrLayer::Background] {
                if let Some(surface) = layer_map.layer_under(layer, local) {
                    if let Some(geo) = layer_map.layer_geometry(surface) {
                        return surface
                            .surface_under(local - geo.loc.to_f64(), WindowSurfaceType::ALL)
                            .map(|(s, p)| (s, (p + output_geo.loc + geo.loc).to_f64()));
                    }
                }
            }
        }

        None
    }

    pub fn switch_workspace(&mut self, idx: usize) {
        if let Some((old, new)) = self.workspaces.try_switch(idx) {
            let outputs = self.outputs.clone();
            self.workspaces.remap_outputs(&outputs, old, new);
            let serial = SERIAL_COUNTER.next_serial();
            if let Some(kbd) = self.seat.get_keyboard() {
                kbd.set_focus(self, Option::<WlSurface>::None, serial);
            }
            self.broadcast_workspace();
            self.broadcast_window_snapshot();
        }
    }

    pub fn move_focused_window_to_workspace(&mut self, target: usize) {
        let kbd = match self.seat.get_keyboard() {
            Some(k) => k,
            None => return,
        };
        let surface = match kbd.current_focus() {
            Some(s) => s,
            None => return,
        };
        let window = self
            .workspaces
            .active_space()
            .elements()
            .find(|w| w.toplevel().map_or(false, |t| t.wl_surface() == &surface))
            .cloned();
        if let Some(window) = window {
            let serial = SERIAL_COUNTER.next_serial();
            kbd.set_focus(self, Option::<WlSurface>::None, serial);
            self.workspaces.move_window_to(window, target);
            self.broadcast_window_snapshot();
        }
    }

    pub fn poll_ipc(&mut self) {
        let poll = self.ipc.poll();

        if poll.accepted_clients > 0 {
            tracing::info!("accepted {} shell IPC client(s)", poll.accepted_clients);
            self.broadcast_workspace();
            self.broadcast_window_snapshot();
        }

        for command in poll.commands {
            tracing::info!("received shell IPC command: {:?}", command);
            match command {
                ShellCommand::SwitchWorkspace { workspace } => {
                    let idx = usize::from(workspace.saturating_sub(1).min(8));
                    self.switch_workspace(idx);
                }
                ShellCommand::FocusWindow { id } => {
                    self.focus_window_by_id(&id);
                }
                ShellCommand::LaunchApp { command, terminal } => {
                    let Some(command) = launch_command(&command, terminal) else {
                        tracing::warn!(
                            "cannot launch terminal app {:?}: no terminal emulator found",
                            command
                        );
                        continue;
                    };

                    tracing::info!("launching app from shell: {}", command);
                    if let Err(err) = Command::new("sh")
                        .arg("-c")
                        .arg(&command)
                        .env(
                            "WAYLAND_DISPLAY",
                            self.socket_name.to_string_lossy().as_ref(),
                        )
                        .env(
                            "XDG_RUNTIME_DIR",
                            std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
                                format!("/run/user/{}", unsafe { libc::geteuid() })
                            }),
                        )
                        .spawn()
                    {
                        tracing::warn!(
                            "failed to launch app from shell command {:?}: {}",
                            command,
                            err
                        );
                    }
                }
                ShellCommand::ReloadConfig => {
                    self.reload_keybinds();
                }
            }
        }
    }

    pub fn broadcast_workspace(&mut self) {
        self.ipc.broadcast(&ShellEvent::WorkspaceChanged {
            workspace: (self.workspaces.active + 1) as u8,
        });
    }

    pub fn broadcast_window_snapshot(&mut self) {
        let windows: Vec<(String, String)> = self
            .workspaces
            .active_space()
            .elements()
            .filter_map(|window| {
                let toplevel = window.toplevel()?;
                Some((window_id(toplevel.wl_surface()), toplevel_title(&toplevel)))
            })
            .collect();

        for (id, title) in windows {
            self.ipc.broadcast(&ShellEvent::WindowOpened { id, title });
        }
    }

    pub fn broadcast_toplevel_opened(&mut self, surface: &ToplevelSurface) {
        self.ipc.broadcast(&ShellEvent::WindowOpened {
            id: window_id(surface.wl_surface()),
            title: toplevel_title(surface),
        });
    }

    pub fn broadcast_toplevel_closed(&mut self, surface: &ToplevelSurface) {
        self.ipc.broadcast(&ShellEvent::WindowClosed {
            id: window_id(surface.wl_surface()),
        });
    }

    pub fn broadcast_toplevel_focused(&mut self, surface: &WlSurface) {
        self.ipc.broadcast(&ShellEvent::WindowFocused {
            id: window_id(surface),
        });
    }

    pub fn broadcast_toggle_launcher(&mut self) {
        self.ipc.broadcast(&ShellEvent::ToggleLauncher);
    }

    pub fn reload_keybinds(&mut self) {
        let mut config = MeridianConfig::default();
        if let Err(err) = config.reload() {
            tracing::warn!("failed to reload keybinds: {}", err);
            return;
        }
        self.keybind_config = config.keybinds;
        tracing::info!("keybinds reloaded");
    }

    pub fn focus_window_by_id(&mut self, id: &str) {
        let Some(window) = self
            .workspaces
            .active_space()
            .elements()
            .find(|window| {
                window
                    .toplevel()
                    .map(|toplevel| window_id(toplevel.wl_surface()) == id)
                    .unwrap_or(false)
            })
            .cloned()
        else {
            tracing::warn!("focus-window requested unknown id: {}", id);
            return;
        };

        self.workspaces
            .active_space_mut()
            .raise_element(&window, true);

        if let Some(surface) = window.wl_surface().map(|surface| surface.into_owned()) {
            let serial = SERIAL_COUNTER.next_serial();
            if let Some(keyboard) = self.seat.get_keyboard() {
                keyboard.set_focus(self, Some(surface.clone()), serial);
            }
            self.broadcast_toplevel_focused(&surface);
        }

        self.workspaces
            .active_space()
            .elements()
            .for_each(|window| {
                if let Some(toplevel) = window.toplevel() {
                    toplevel.send_pending_configure();
                }
            });
    }
}

pub struct IpcServer {
    listener: Option<UnixListener>,
    clients: Vec<IpcClient>,
}

pub struct IpcPoll {
    pub accepted_clients: usize,
    pub commands: Vec<ShellCommand>,
}

struct IpcClient {
    stream: UnixStream,
    buffer: Vec<u8>,
    alive: bool,
}

impl IpcServer {
    fn new() -> Self {
        let path = meridian_ipc::socket_path();
        if path.exists() {
            if let Err(err) = fs::remove_file(&path) {
                tracing::warn!("failed to remove stale IPC socket {:?}: {}", path, err);
            }
        }

        let listener = match UnixListener::bind(&path) {
            Ok(listener) => {
                if let Err(err) = listener.set_nonblocking(true) {
                    tracing::warn!("failed to set IPC socket nonblocking: {}", err);
                }
                tracing::info!("Meridian IPC listening on {:?}", path);
                Some(listener)
            }
            Err(err) => {
                tracing::warn!("failed to bind IPC socket {:?}: {}", path, err);
                None
            }
        };

        Self {
            listener,
            clients: Vec::new(),
        }
    }

    pub fn poll(&mut self) -> IpcPoll {
        let mut accepted_clients = 0;
        let mut commands = Vec::new();

        if let Some(listener) = &self.listener {
            loop {
                match listener.accept() {
                    Ok((stream, _addr)) => {
                        if let Err(err) = stream.set_nonblocking(true) {
                            tracing::warn!("failed to set IPC client nonblocking: {}", err);
                        }
                        self.clients.push(IpcClient {
                            stream,
                            buffer: Vec::new(),
                            alive: true,
                        });
                        accepted_clients += 1;
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                    Err(err) => {
                        tracing::warn!("failed to accept IPC client: {}", err);
                        break;
                    }
                }
            }
        }

        let mut tmp = [0_u8; 4096];
        for client in &mut self.clients {
            loop {
                match client.stream.read(&mut tmp) {
                    Ok(0) => {
                        client.alive = false;
                        break;
                    }
                    Ok(n) => client.buffer.extend_from_slice(&tmp[..n]),
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                    Err(err) => {
                        tracing::warn!("IPC client read failed: {}", err);
                        client.alive = false;
                        break;
                    }
                }
            }

            while let Some(pos) = client.buffer.iter().position(|byte| *byte == b'\n') {
                let line = client.buffer.drain(..=pos).collect::<Vec<_>>();
                let line = String::from_utf8_lossy(&line);
                match meridian_ipc::decode_command(line.trim()) {
                    Ok(command) => commands.push(command),
                    Err(err) => tracing::warn!("invalid IPC command {:?}: {}", line.trim(), err),
                }
            }
        }

        self.retain_alive();

        IpcPoll {
            accepted_clients,
            commands,
        }
    }

    pub fn broadcast(&mut self, event: &ShellEvent) {
        let Ok(bytes) = meridian_ipc::encode_event(event) else {
            return;
        };

        for client in &mut self.clients {
            if let Err(err) = client.stream.write_all(&bytes) {
                tracing::debug!("IPC client write failed: {}", err);
                client.alive = false;
            }
        }

        self.retain_alive();
    }

    fn retain_alive(&mut self) {
        self.clients.retain_mut(|client| {
            if !client.alive {
                let _ = client.stream.shutdown(Shutdown::Both);
            }
            client.alive
        });
    }
}

fn window_id(surface: &WlSurface) -> String {
    surface.id().to_string()
}

fn toplevel_title(surface: &ToplevelSurface) -> String {
    with_states(surface.wl_surface(), |states| {
        let data = states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .unwrap()
            .lock()
            .unwrap();

        data.title
            .clone()
            .or_else(|| data.app_id.clone())
            .unwrap_or_else(|| "Window".to_string())
    })
}

fn launch_command(command: &str, terminal: bool) -> Option<String> {
    if !terminal {
        return Some(command.to_string());
    }

    let terminal = std::env::var("TERMINAL")
        .ok()
        .filter(|terminal| !terminal.trim().is_empty())
        .or_else(|| {
            [
                "foot",
                "alacritty",
                "kitty",
                "wezterm",
                "ghostty",
                "kgx",
                "konsole",
                "xterm",
            ]
            .into_iter()
            .find(|candidate| command_exists(candidate))
            .map(str::to_string)
        })?;

    Some(format!(
        "{} -e sh -lc {}",
        shell_quote(&terminal),
        shell_quote(command)
    ))
}

fn command_exists(command: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };

    std::env::split_paths(&path).any(|dir| dir.join(command).is_file())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
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
    pub popups: PopupManager,
    pub theme_manager: ThemeManager,
    pub wm_workspaces: Vec<WmWorkspace>,
    pub ipc: IpcServer,
    pub keybind_config: KeybindConfig,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub layer_shell_state: WlrLayerShellState,
    pub shm_state: ShmState,
    pub seat_state: SeatState<Self>,
    pub output_manager_state: OutputManagerState,
    pub data_device_state: DataDeviceState,
    pub xwayland_shell_state: XWaylandShellState,
    pub xwm: Option<X11Wm>,
    pub drm_backend: Option<DrmBackend>,
}

impl MeridianState {
    pub fn new(event_loop: &mut EventLoop<'static, Self>, display: Display<Self>) -> Self {
        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let xwayland_shell_state = XWaylandShellState::new::<Self>(&dh);

        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(&dh, "seat-0");
        seat.add_keyboard(Default::default(), 200, 25).unwrap();
        seat.add_pointer();

        let loop_handle = event_loop.handle();
        let socket_name = Self::init_wayland_listener(display, event_loop);
        let loop_signal = event_loop.get_signal();

        let theme_manager = ThemeManager::new();
        let meridian_config = MeridianConfig::load();

        Self {
            start_time: Instant::now(),
            display_handle: dh,
            loop_handle,
            loop_signal,
            socket_name,
            seat,
            workspaces: WorkspaceManager::new(),
            outputs: Vec::new(),
            popups: PopupManager::default(),
            theme_manager,
            wm_workspaces: (0..9).map(|_| WmWorkspace::new()).collect(),
            ipc: IpcServer::new(),
            keybind_config: meridian_config.keybinds,
            compositor_state,
            xdg_shell_state,
            layer_shell_state,
            shm_state,
            seat_state,
            output_manager_state,
            data_device_state,
            xwayland_shell_state,
            xwm: None,
            drm_backend: None,
        }
    }

    fn init_wayland_listener(display: Display<Self>, event_loop: &mut EventLoop<Self>) -> OsString {
        let listening_socket = ListeningSocketSource::new_auto().unwrap();
        let socket_name = listening_socket.socket_name().to_os_string();
        let loop_handle = event_loop.handle();

        loop_handle
            .insert_source(listening_socket, move |client_stream, _, state| {
                state
                    .display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                    .unwrap();
            })
            .expect("failed to initialize wayland socket");

        loop_handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    unsafe { display.get_mut().dispatch_clients(state).unwrap() };
                    Ok(PostAction::Continue)
                },
            )
            .expect("failed to insert display into event loop");

        socket_name
    }
}

// ── Buffer ────────────────────────────────────────────────────────────────────

impl BufferHandler for MeridianState {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

// ── Compositor ────────────────────────────────────────────────────────────────

impl CompositorHandler for MeridianState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        if let Some(state) = client.get_data::<XWaylandClientData>() {
            return &state.compositor_state;
        }
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);

        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            // Use wl_surface() so X11 windows (no toplevel) are also covered.
            if let Some(window) = self
                .workspaces
                .active_space()
                .elements()
                .find(|w| w.wl_surface().map_or(false, |s| *s == root))
            {
                window.on_commit();
            }
        }

        handle_commit(&mut self.popups, self.workspaces.active_space(), surface);
        crate::grabs::resize_grab::handle_commit(self.workspaces.active_space_mut(), surface);

        if let Some(output) = self.outputs.iter().find(|output| {
            let map = layer_map_for_output(output);
            map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
                .is_some()
        }) {
            let initial_configure_sent = with_states(surface, |states| {
                states
                    .data_map
                    .get::<LayerSurfaceData>()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .initial_configure_sent
            });

            let mut map = layer_map_for_output(output);
            map.arrange();

            if !initial_configure_sent {
                if let Some(layer) = map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL) {
                    layer.layer_surface().send_configure();
                }
            }
        }

        let active = self.workspaces.active;
        if self.wm_workspaces[active].mode == WorkspaceMode::Tiling {
            self.tile_workspace(active);
        }
    }
}

impl WlrLayerShellHandler for MeridianState {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        output: Option<WlOutput>,
        _layer: WlrLayer,
        namespace: String,
    ) {
        let output = output
            .as_ref()
            .and_then(Output::from_resource)
            .or_else(|| self.outputs.first().cloned());

        let Some(output) = output else {
            surface.send_close();
            return;
        };

        let layer = DesktopLayerSurface::new(surface, namespace);
        let map_result = {
            let mut map = layer_map_for_output(&output);
            map.map_layer(&layer)
        };

        if let Err(err) = map_result {
            tracing::warn!("failed to map layer surface: {}", err);
            layer.layer_surface().send_close();
        }
    }

    fn new_popup(&mut self, _parent: WlrLayerSurface, popup: PopupSurface) {
        let _ = self.popups.track_popup(PopupKind::Xdg(popup));
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        for output in &self.outputs {
            let mut map = layer_map_for_output(output);
            let layer = map
                .layers()
                .find(|layer| layer.layer_surface() == &surface)
                .cloned();

            if let Some(layer) = layer {
                map.unmap_layer(&layer);
                break;
            }
        }
    }
}

fn check_grab(
    seat: &Seat<MeridianState>,
    surface: &WlSurface,
    serial: Serial,
) -> Option<PointerGrabStartData<MeridianState>> {
    let pointer = seat.get_pointer()?;
    if !pointer.has_grab(serial) {
        return None;
    }
    let start_data = pointer.grab_start_data()?;
    let (focus, _) = start_data.focus.as_ref()?;
    if !focus.id().same_client_as(&surface.id()) {
        return None;
    }
    Some(start_data)
}

// ── SHM ───────────────────────────────────────────────────────────────────────

impl ShmHandler for MeridianState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

// ── XDG Shell ─────────────────────────────────────────────────────────────────

impl XdgShellHandler for MeridianState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        tracing::info!("new xdg toplevel: {}", toplevel_title(&surface));
        self.broadcast_toplevel_opened(&surface);
        let wl_surface = surface.wl_surface().clone();
        let window = Window::new_wayland_window(surface);
        let active = self.workspaces.active;
        self.workspaces
            .active_space_mut()
            .map_element(window.clone(), (0, 0), true);
        let serial = SERIAL_COUNTER.next_serial();
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(self, Some(wl_surface.clone()), serial);
            self.broadcast_toplevel_focused(&wl_surface);
        }
        if self.wm_workspaces[active].mode == WorkspaceMode::Tiling {
            let focused = self.focused_window();
            self.wm_workspaces[active].add_tiled(window, focused.as_ref());
            self.tile_workspace(active);
        }
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        self.broadcast_toplevel_closed(&surface);
    }

    fn app_id_changed(&mut self, surface: ToplevelSurface) {
        self.broadcast_toplevel_opened(&surface);
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        self.broadcast_toplevel_opened(&surface);
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {}

    fn reposition_request(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: WlSeat, serial: Serial) {
        let seat = Seat::from_resource(&seat).unwrap();
        let wl_surface = surface.wl_surface();
        if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
            let window = match self
                .workspaces
                .active_space()
                .elements()
                .find(|w| w.toplevel().map_or(false, |t| t.wl_surface() == wl_surface))
                .cloned()
            {
                Some(w) => w,
                None => return,
            };
            let initial_window_location = self
                .workspaces
                .active_space()
                .element_location(&window)
                .unwrap();
            let grab = MoveSurfaceGrab {
                start_data,
                window,
                initial_window_location,
            };
            seat.get_pointer()
                .unwrap()
                .set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        let seat = Seat::from_resource(&seat).unwrap();
        let wl_surface = surface.wl_surface();
        if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
            let window = match self
                .workspaces
                .active_space()
                .elements()
                .find(|w| w.toplevel().map_or(false, |t| t.wl_surface() == wl_surface))
                .cloned()
            {
                Some(w) => w,
                None => return,
            };
            let initial_window_location = self
                .workspaces
                .active_space()
                .element_location(&window)
                .unwrap();
            let initial_window_size = window.geometry().size;
            surface.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
            });
            surface.send_pending_configure();
            let grab = ResizeSurfaceGrab::start(
                start_data,
                window,
                ResizeEdge::from(edges),
                Rectangle::new(initial_window_location, initial_window_size),
            );
            seat.get_pointer()
                .unwrap()
                .set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        let output_geo = self
            .outputs
            .first()
            .and_then(|o| self.workspaces.active_space().output_geometry(o));
        if let Some(geo) = output_geo {
            surface.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Maximized);
                state.size = Some(geo.size);
            });
            let window = self
                .workspaces
                .active_space()
                .elements()
                .find(|w| {
                    w.toplevel()
                        .map_or(false, |t| t.wl_surface() == surface.wl_surface())
                })
                .cloned();
            if let Some(window) = window {
                self.workspaces
                    .active_space_mut()
                    .map_element(window, geo.loc, true);
            }
        }
        surface.send_pending_configure();
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        surface.with_pending_state(|state| {
            state.states.unset(xdg_toplevel::State::Maximized);
            state.size = None;
        });
        surface.send_pending_configure();
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, _output: Option<WlOutput>) {
        let output_geo = self
            .outputs
            .first()
            .and_then(|o| self.workspaces.active_space().output_geometry(o));
        if let Some(geo) = output_geo {
            surface.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Fullscreen);
                state.size = Some(geo.size);
            });
            let window = self
                .workspaces
                .active_space()
                .elements()
                .find(|w| {
                    w.toplevel()
                        .map_or(false, |t| t.wl_surface() == surface.wl_surface())
                })
                .cloned();
            if let Some(window) = window {
                self.workspaces
                    .active_space_mut()
                    .map_element(window, geo.loc, true);
            }
        }
        surface.send_pending_configure();
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        surface.with_pending_state(|state| {
            state.states.unset(xdg_toplevel::State::Fullscreen);
            state.size = None;
        });
        surface.send_pending_configure();
    }
}

// ── Seat ──────────────────────────────────────────────────────────────────────

impl SeatHandler for MeridianState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, _image: CursorImageStatus) {}
}

// ── Output ────────────────────────────────────────────────────────────────────

impl OutputHandler for MeridianState {}

// ── Client state ──────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl smithay::reexports::wayland_server::backend::ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

// ── Selection / DataDevice ────────────────────────────────────────────────────

impl SelectionHandler for MeridianState {
    type SelectionUserData = ();
}

impl WaylandDndGrabHandler for MeridianState {}

impl DataDeviceHandler for MeridianState {
    fn data_device_state(&mut self) -> &mut DataDeviceState {
        &mut self.data_device_state
    }
}

impl DndGrabHandler for MeridianState {}

// ── Delegation ────────────────────────────────────────────────────────────────

delegate_dispatch2!(MeridianState);
