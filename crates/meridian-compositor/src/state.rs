use std::{ffi::OsString, sync::Arc, time::Instant};

use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_dispatch2,
    desktop::{PopupKind, PopupManager, Window, WindowSurfaceType},
    input::{Seat, SeatHandler, SeatState, pointer::{CursorImageStatus, Focus, GrabStartData as PointerGrabStartData}},
    output::Output,
    reexports::{
        calloop::{EventLoop, Interest, LoopSignal, Mode, PostAction, generic::Generic},
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            Client, Display, DisplayHandle, Resource,
            backend::{ClientId, DisconnectReason},
            protocol::{wl_buffer::WlBuffer, wl_output::WlOutput, wl_seat::WlSeat, wl_surface::WlSurface},
        },
    },
    utils::{Logical, Point, Rectangle, Serial, SERIAL_COUNTER},
    wayland::{
        buffer::BufferHandler,
        compositor::{
            CompositorClientState, CompositorHandler, CompositorState, get_parent,
            is_sync_subsurface,
        },
        output::{OutputHandler, OutputManagerState},
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
        },
        shm::{ShmHandler, ShmState},
        socket::ListeningSocketSource,
    },
};

use meridian_config::ThemeManager;
use meridian_wm::{WmWorkspace, WorkspaceMode};

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
        let output_rect = self.outputs.first()
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
            let windows: Vec<Window> =
                self.workspaces.active_space().elements().cloned().collect();
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
        self.workspaces.active_space().element_under(pos).and_then(|(window, location)| {
            window
                .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                .map(|(s, p)| (s, (p + location).to_f64()))
        })
    }

    pub fn switch_workspace(&mut self, idx: usize) {
        if let Some((old, new)) = self.workspaces.try_switch(idx) {
            let outputs = self.outputs.clone();
            self.workspaces.remap_outputs(&outputs, old, new);
            let serial = SERIAL_COUNTER.next_serial();
            if let Some(kbd) = self.seat.get_keyboard() {
                kbd.set_focus(self, Option::<WlSurface>::None, serial);
            }
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
        let window = self.workspaces.active_space()
            .elements()
            .find(|w| w.toplevel().unwrap().wl_surface() == &surface)
            .cloned();
        if let Some(window) = window {
            let serial = SERIAL_COUNTER.next_serial();
            kbd.set_focus(self, Option::<WlSurface>::None, serial);
            self.workspaces.move_window_to(window, target);
        }
    }
}

pub struct MeridianState {
    pub start_time: Instant,
    pub display_handle: DisplayHandle,
    pub loop_signal: LoopSignal,
    pub socket_name: OsString,
    pub seat: Seat<Self>,
    pub workspaces: WorkspaceManager,
    pub outputs: Vec<Output>,
    pub popups: PopupManager,
    pub theme_manager: ThemeManager,
    pub wm_workspaces: Vec<WmWorkspace>,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub seat_state: SeatState<Self>,
    pub output_manager_state: OutputManagerState,
    pub drm_backend: Option<DrmBackend>,
}

impl MeridianState {
    pub fn new(event_loop: &mut EventLoop<Self>, display: Display<Self>) -> Self {
        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);

        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(&dh, "seat-0");
        seat.add_keyboard(Default::default(), 200, 25).unwrap();
        seat.add_pointer();

        let socket_name = Self::init_wayland_listener(display, event_loop);
        let loop_signal = event_loop.get_signal();

        Self {
            start_time: Instant::now(),
            display_handle: dh,
            loop_signal,
            socket_name,
            seat,
            workspaces: WorkspaceManager::new(),
            outputs: Vec::new(),
            popups: PopupManager::default(),
            theme_manager: ThemeManager::new(),
            wm_workspaces: (0..9).map(|_| WmWorkspace::new()).collect(),
            compositor_state,
            xdg_shell_state,
            shm_state,
            seat_state,
            output_manager_state,
            drm_backend: None,
        }
    }

    fn init_wayland_listener(
        display: Display<Self>,
        event_loop: &mut EventLoop<Self>,
    ) -> OsString {
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
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);

        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(window) = self
                .workspaces
                .active_space()
                .elements()
                .find(|w| w.toplevel().unwrap().wl_surface() == &root)
            {
                window.on_commit();
            }
        }

        handle_commit(&mut self.popups, self.workspaces.active_space(), surface);
        crate::grabs::resize_grab::handle_commit(self.workspaces.active_space_mut(), surface);

        let active = self.workspaces.active;
        if self.wm_workspaces[active].mode == WorkspaceMode::Tiling {
            self.tile_workspace(active);
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
        let window = Window::new_wayland_window(surface);
        let active = self.workspaces.active;
        self.workspaces.active_space_mut().map_element(window.clone(), (0, 0), true);
        if self.wm_workspaces[active].mode == WorkspaceMode::Tiling {
            let focused = self.focused_window();
            self.wm_workspaces[active].add_tiled(window, focused.as_ref());
            self.tile_workspace(active);
        }
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
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
            let window = self
                .workspaces
                .active_space()
                .elements()
                .find(|w| w.toplevel().unwrap().wl_surface() == wl_surface)
                .unwrap()
                .clone();
            let initial_window_location = self.workspaces.active_space().element_location(&window).unwrap();
            let grab = MoveSurfaceGrab { start_data, window, initial_window_location };
            seat.get_pointer().unwrap().set_grab(self, grab, serial, Focus::Clear);
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
            let window = self
                .workspaces
                .active_space()
                .elements()
                .find(|w| w.toplevel().unwrap().wl_surface() == wl_surface)
                .unwrap()
                .clone();
            let initial_window_location = self.workspaces.active_space().element_location(&window).unwrap();
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
            seat.get_pointer().unwrap().set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        let output_geo = self.outputs.first()
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
                .find(|w| w.toplevel().unwrap().wl_surface() == surface.wl_surface())
                .cloned();
            if let Some(window) = window {
                self.workspaces.active_space_mut().map_element(window, geo.loc, true);
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
        let output_geo = self.outputs.first()
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
                .find(|w| w.toplevel().unwrap().wl_surface() == surface.wl_surface())
                .cloned();
            if let Some(window) = window {
                self.workspaces.active_space_mut().map_element(window, geo.loc, true);
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

// ── Delegation ────────────────────────────────────────────────────────────────

delegate_dispatch2!(MeridianState);
