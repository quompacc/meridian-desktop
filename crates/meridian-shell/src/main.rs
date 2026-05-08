use std::{
    cell::RefCell,
    ffi::{CStr, CString},
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    path::PathBuf,
    ptr,
    time::{Duration, Instant},
};

use meridian_config::{Color, ThemeConfig, ThemeManager};
use meridian_ipc::{ShellCommand, ShellEvent};
use smithay_client_toolkit::reexports::{
    calloop::{
        timer::{TimeoutAction, Timer},
        EventLoop,
    },
    calloop_wayland_source::WaylandSource,
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{
        slot::{Buffer, SlotPool},
        Shm, ShmHandler,
    },
};
use tracing::{debug, warn};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};

mod launcher;
mod panel;

pub const PANEL_HEIGHT: u32 = 36;
pub const LAUNCHER_WIDTH: u32 = 520;
pub const LAUNCHER_HEIGHT: u32 = 420;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let conn = Connection::connect_to_env()?;
    let (globals, event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();
    let mut event_loop: EventLoop<MeridianShell> = EventLoop::try_new()?;
    WaylandSource::new(conn.clone(), event_queue).insert(event_loop.handle())?;

    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor is not available");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("wlr layer shell is not available");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm is not available");

    let panel_surface = compositor.create_surface(&qh);
    let panel = layer_shell.create_layer_surface(
        &qh,
        panel_surface,
        Layer::Top,
        Some("meridian-panel"),
        None,
    );
    panel.set_anchor(Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    panel.set_size(0, PANEL_HEIGHT);
    panel.set_exclusive_zone(PANEL_HEIGHT as i32);
    panel.set_keyboard_interactivity(KeyboardInteractivity::None);
    panel.commit();

    let launcher_surface = compositor.create_surface(&qh);
    let launcher_layer = layer_shell.create_layer_surface(
        &qh,
        launcher_surface,
        Layer::Overlay,
        Some("meridian-launcher"),
        None,
    );
    launcher_layer.set_anchor(Anchor::BOTTOM | Anchor::LEFT);
    launcher_layer.set_margin(0, 0, PANEL_HEIGHT as i32, 8);
    launcher_layer.set_size(LAUNCHER_WIDTH, LAUNCHER_HEIGHT);
    launcher_layer.set_exclusive_zone(0);
    launcher_layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    launcher_layer.commit();

    let theme = ThemeManager::new().current().config.clone();
    let font = TextRenderer::new(&theme.fonts.ui, 13);
    let pool = SlotPool::new(1024 * 1024 * 4, &shm)?;

    let mut shell = MeridianShell {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        panel,
        launcher_layer,
        panel_configured: false,
        launcher_configured: false,
        panel_buffer: None,
        launcher_buffer: None,
        pool,
        width: 1024,
        launcher_width: LAUNCHER_WIDTH,
        launcher_height: LAUNCHER_HEIGHT,
        keyboard: None,
        keyboard_focus: SurfaceKind::None,
        pointer: None,
        pointer_position: (0.0, 0.0),
        pointer_surface: SurfaceKind::None,
        theme,
        font: RefCell::new(font),
        ipc: IpcClient::connect(),
        panel_state: panel::PanelState::new(),
        launcher_state: launcher::LauncherState::new(),
        focused_window_id: None,
        focused_title: None,
        windows: Vec::new(),
        active_workspace: 1,
        last_clock: String::new(),
        last_tick: Instant::now(),
        exit: false,
    };

    let timer_qh = qh.clone();
    event_loop
        .handle()
        .insert_source(Timer::immediate(), move |_, _, shell| {
            let ipc_changed = shell.poll_ipc();
            if ipc_changed {
                shell.draw_panel(&timer_qh);
            }
            shell.tick(&timer_qh);
            TimeoutAction::ToDuration(Duration::from_millis(250))
        })?;

    while !shell.exit {
        event_loop.dispatch(Duration::from_millis(500), &mut shell)?;
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SurfaceKind {
    None,
    Panel,
    Launcher,
}

#[derive(Debug, Clone)]
struct WindowInfo {
    id: String,
    title: String,
}

#[derive(Debug, Clone)]
pub enum ClickAction {
    SwitchWorkspace(u8),
    LaunchApp(usize),
}

#[derive(Debug, Clone)]
pub struct ClickZone {
    pub rect: Rect,
    pub action: ClickAction,
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x as f64
            && y >= self.y as f64
            && x < (self.x + self.w) as f64
            && y < (self.y + self.h) as f64
    }
}

struct MeridianShell {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    panel: LayerSurface,
    launcher_layer: LayerSurface,
    panel_configured: bool,
    launcher_configured: bool,
    panel_buffer: Option<Buffer>,
    launcher_buffer: Option<Buffer>,
    pool: SlotPool,
    width: u32,
    launcher_width: u32,
    launcher_height: u32,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    keyboard_focus: SurfaceKind,
    pointer: Option<wl_pointer::WlPointer>,
    pointer_position: (f64, f64),
    pointer_surface: SurfaceKind,
    theme: ThemeConfig,
    font: RefCell<Option<TextRenderer>>,
    ipc: IpcClient,
    panel_state: panel::PanelState,
    launcher_state: launcher::LauncherState,
    focused_window_id: Option<String>,
    focused_title: Option<String>,
    windows: Vec<WindowInfo>,
    active_workspace: u8,
    last_clock: String,
    last_tick: Instant,
    exit: bool,
}

impl MeridianShell {
    fn tick(&mut self, qh: &QueueHandle<Self>) {
        let now = Instant::now();
        if now.duration_since(self.last_tick) >= Duration::from_secs(1) {
            self.last_tick = now;
            let clock = formatted_time();
            if clock != self.last_clock {
                self.last_clock = clock;
                self.draw_panel(qh);
            }
        }

        if self.ipc.should_reconnect() {
            self.ipc.reconnect();
        }
    }

    fn poll_ipc(&mut self) -> bool {
        let mut changed = false;
        for event in self.ipc.poll() {
            self.apply_ipc_event(event);
            changed = true;
        }
        changed
    }

    fn apply_ipc_event(&mut self, event: ShellEvent) {
        match event {
            ShellEvent::WorkspaceChanged { workspace } => {
                self.active_workspace = workspace.clamp(1, 9);
                self.windows.clear();
                self.focused_window_id = None;
                self.focused_title = None;
            }
            ShellEvent::WindowOpened { id, title } => {
                if let Some(window) = self.windows.iter_mut().find(|w| w.id == id) {
                    window.title = title;
                } else {
                    self.windows.push(WindowInfo { id, title });
                }
                self.update_focused_title();
            }
            ShellEvent::WindowClosed { id } => {
                self.windows.retain(|w| w.id != id);
                if self.focused_window_id.as_deref() == Some(id.as_str()) {
                    self.focused_window_id = None;
                    self.focused_title = None;
                }
            }
            ShellEvent::WindowFocused { id } => {
                self.focused_window_id = Some(id);
                self.update_focused_title();
            }
            ShellEvent::ToggleLauncher => {
                self.toggle_launcher();
            }
        }
    }

    fn update_focused_title(&mut self) {
        self.focused_title = self
            .focused_window_id
            .as_deref()
            .and_then(|id| self.windows.iter().find(|w| w.id == id))
            .map(|w| w.title.clone());
    }

    fn toggle_launcher(&mut self) {
        let was_open = self.launcher_state.open;
        self.launcher_state.toggle();
        if !was_open && self.launcher_state.open {
            self.launcher_state.apps = launcher::DesktopApp::load_system();
        }
    }

    fn draw_panel(&mut self, qh: &QueueHandle<Self>) {
        if !self.panel_configured || self.width == 0 {
            return;
        }

        let width = self.width;
        let height = PANEL_HEIGHT;
        let stride = (width * 4) as i32;
        let buffer = Self::buffer_for(
            &mut self.pool,
            &mut self.panel_buffer,
            width,
            height,
            stride,
        );
        let Some(canvas) = buffer.canvas(&mut self.pool) else {
            self.panel_buffer = None;
            return self.draw_panel(qh);
        };

        let mut painter = Painter::new(canvas, width as i32, height as i32);
        let clock = if self.last_clock.is_empty() {
            formatted_time()
        } else {
            self.last_clock.clone()
        };

        panel::draw_panel(
            &mut self.panel_state,
            &mut painter,
            &self.font,
            &self.theme,
            self.active_workspace,
            self.focused_title.as_deref(),
            &clock,
            width,
        );

        self.panel
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);
        self.panel
            .wl_surface()
            .frame(qh, self.panel.wl_surface().clone());
        buffer
            .attach_to(self.panel.wl_surface())
            .expect("panel buffer attach");
        self.panel.commit();
    }

    fn draw_launcher(&mut self, qh: &QueueHandle<Self>) {
        if !self.launcher_configured || !self.launcher_state.open {
            return;
        }

        let width = self.launcher_width.max(LAUNCHER_WIDTH);
        let height = self.launcher_height.max(LAUNCHER_HEIGHT);
        let stride = (width * 4) as i32;
        let buffer = Self::buffer_for(
            &mut self.pool,
            &mut self.launcher_buffer,
            width,
            height,
            stride,
        );
        let Some(canvas) = buffer.canvas(&mut self.pool) else {
            self.launcher_buffer = None;
            return self.draw_launcher(qh);
        };

        let mut painter = Painter::new(canvas, width as i32, height as i32);
        launcher::draw_launcher(
            &mut self.launcher_state,
            &mut painter,
            &self.font,
            &self.theme,
            width,
            height,
        );

        self.launcher_layer
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);
        self.launcher_layer
            .wl_surface()
            .frame(qh, self.launcher_layer.wl_surface().clone());
        buffer
            .attach_to(self.launcher_layer.wl_surface())
            .expect("launcher buffer attach");
        self.launcher_layer.commit();
    }

    fn unmap_launcher(&mut self) {
        self.launcher_layer.wl_surface().attach(None, 0, 0);
        self.launcher_layer.commit();
    }

    fn buffer_for<'a>(
        pool: &mut SlotPool,
        current: &'a mut Option<Buffer>,
        width: u32,
        height: u32,
        stride: i32,
    ) -> &'a mut Buffer {
        let recreate = current
            .as_ref()
            .map(|buffer| buffer.height() != height as i32 || buffer.stride() != stride)
            .unwrap_or(true);

        if recreate {
            let (buffer, _) = pool
                .create_buffer(
                    width as i32,
                    height as i32,
                    stride,
                    wl_shm::Format::Argb8888,
                )
                .expect("create shm buffer");
            *current = Some(buffer);
        }

        current.as_mut().expect("buffer exists")
    }

    fn handle_panel_click(&mut self, _qh: &QueueHandle<Self>, action: ClickAction) {
        match action {
            ClickAction::SwitchWorkspace(workspace) => {
                self.active_workspace = workspace;
                self.ipc.send(&ShellCommand::SwitchWorkspace { workspace });
            }
            ClickAction::LaunchApp(index) => {
                self.launcher_state.launch_app(index, &mut self.ipc);
            }
        }
    }

    fn handle_launcher_click(&mut self, _qh: &QueueHandle<Self>, action: ClickAction) {
        match action {
            ClickAction::LaunchApp(index) => {
                self.launcher_state.launch_app(index, &mut self.ipc);
            }
            ClickAction::SwitchWorkspace(_) => {}
        }
    }
}

impl CompositorHandler for MeridianShell {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        if self.panel.wl_surface() == surface {
            self.draw_panel(qh);
        } else if self.launcher_state.open && self.launcher_layer.wl_surface() == surface {
            self.draw_launcher(qh);
        }
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for MeridianShell {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if self.panel == *layer {
            self.panel_configured = true;
            if configure.new_size.0 > 0 {
                self.width = configure.new_size.0;
            }
            self.draw_panel(qh);
        } else if self.launcher_layer == *layer {
            self.launcher_configured = true;
            if configure.new_size.0 > 0 {
                self.launcher_width = configure.new_size.0;
            }
            if configure.new_size.1 > 0 {
                self.launcher_height = configure.new_size.1;
            }
            if self.launcher_state.open {
                self.draw_launcher(qh);
            }
        }
    }
}

impl SeatHandler for MeridianShell {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard = self.seat_state.get_keyboard(qh, &seat, None).ok();
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            self.pointer = self.seat_state.get_pointer(qh, &seat).ok();
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            if let Some(keyboard) = self.keyboard.take() {
                keyboard.release();
            }
        }
        if capability == Capability::Pointer {
            if let Some(pointer) = self.pointer.take() {
                pointer.release();
            }
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for MeridianShell {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        self.keyboard_focus = if self.launcher_layer.wl_surface() == surface {
            SurfaceKind::Launcher
        } else if self.panel.wl_surface() == surface {
            SurfaceKind::Panel
        } else {
            SurfaceKind::None
        };
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        self.keyboard_focus = SurfaceKind::None;
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        if !self.launcher_state.open {
            return;
        }

        let is_escape = event.keysym == Keysym::Escape;
        let is_enter = event.keysym == Keysym::Return || event.keysym == Keysym::KP_Enter;
        let is_backspace = event.keysym == Keysym::BackSpace;
        let ch = event.keysym.key_char().filter(|ch| !ch.is_control());

        use launcher::LauncherInputResult;
        match self.launcher_state.handle_key(ch, is_backspace, is_enter, is_escape) {
            LauncherInputResult::Close => {
                self.unmap_launcher();
                self.draw_panel(qh);
            }
            LauncherInputResult::Launch(idx) => {
                self.launcher_state.launch_app(idx, &mut self.ipc);
            }
            LauncherInputResult::Redraw => {
                self.draw_launcher(qh);
            }
            LauncherInputResult::None => {}
        }
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _modifiers: Modifiers,
        _layout: u32,
    ) {
    }
}

impl PointerHandler for MeridianShell {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            self.pointer_surface = if &event.surface == self.panel.wl_surface() {
                SurfaceKind::Panel
            } else if &event.surface == self.launcher_layer.wl_surface() {
                SurfaceKind::Launcher
            } else {
                SurfaceKind::None
            };
            self.pointer_position = event.position;

            if let PointerEventKind::Press { button: 0x110, .. } = event.kind {
                let action = match self.pointer_surface {
                    SurfaceKind::Panel => self
                        .panel_state
                        .clicks
                        .iter()
                        .find(|zone| zone.rect.contains(event.position.0, event.position.1))
                        .map(|zone| zone.action.clone()),
                    SurfaceKind::Launcher => self
                        .launcher_state
                        .clicks
                        .iter()
                        .find(|zone| zone.rect.contains(event.position.0, event.position.1))
                        .map(|zone| zone.action.clone()),
                    SurfaceKind::None => None,
                };
                if let Some(action) = action {
                    match self.pointer_surface {
                        SurfaceKind::Panel => self.handle_panel_click(qh, action),
                        SurfaceKind::Launcher => self.handle_launcher_click(qh, action),
                        SurfaceKind::None => {}
                    }
                }
            }
        }
    }
}

impl OutputHandler for MeridianShell {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl ShmHandler for MeridianShell {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_compositor!(MeridianShell);
delegate_output!(MeridianShell);
delegate_shm!(MeridianShell);
delegate_seat!(MeridianShell);
delegate_keyboard!(MeridianShell);
delegate_pointer!(MeridianShell);
delegate_layer!(MeridianShell);
delegate_registry!(MeridianShell);

impl ProvidesRegistryState for MeridianShell {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState, SeatState];
}

struct IpcClient {
    stream: Option<UnixStream>,
    buffer: Vec<u8>,
    last_attempt: Instant,
}

impl IpcClient {
    fn connect() -> Self {
        let mut client = Self {
            stream: None,
            buffer: Vec::new(),
            last_attempt: Instant::now() - Duration::from_secs(5),
        };
        client.reconnect();
        client
    }

    fn should_reconnect(&self) -> bool {
        self.stream.is_none() && self.last_attempt.elapsed() >= Duration::from_secs(2)
    }

    fn reconnect(&mut self) {
        self.last_attempt = Instant::now();
        match UnixStream::connect(meridian_ipc::socket_path()) {
            Ok(stream) => {
                if let Err(err) = stream.set_nonblocking(true) {
                    warn!("failed to set meridian IPC nonblocking: {}", err);
                }
                self.stream = Some(stream);
            }
            Err(err) => {
                debug!("meridian IPC unavailable: {}", err);
            }
        }
    }

    fn poll(&mut self) -> Vec<ShellEvent> {
        let mut out = Vec::new();
        let Some(stream) = self.stream.as_mut() else {
            return out;
        };

        let mut tmp = [0_u8; 4096];
        loop {
            match stream.read(&mut tmp) {
                Ok(0) => {
                    self.disconnect();
                    break;
                }
                Ok(n) => self.buffer.extend_from_slice(&tmp[..n]),
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                Err(err) => {
                    warn!("meridian IPC read failed: {}", err);
                    self.disconnect();
                    break;
                }
            }
        }

        while let Some(pos) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let line = self.buffer.drain(..=pos).collect::<Vec<_>>();
            let line = String::from_utf8_lossy(&line);
            if let Some(event) = parse_event_line(line.trim()) {
                out.push(event);
            }
        }

        out
    }

    fn send(&mut self, command: &ShellCommand) -> bool {
        if self.stream.is_none() {
            self.reconnect();
        }

        let Some(stream) = self.stream.as_mut() else {
            return false;
        };

        let Ok(bytes) = meridian_ipc::encode_command(command) else {
            return false;
        };

        match stream.write_all(&bytes) {
            Ok(()) => true,
            Err(err) => {
                warn!("meridian IPC write failed: {}", err);
                self.disconnect();
                false
            }
        }
    }

    fn disconnect(&mut self) {
        if let Some(stream) = self.stream.take() {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

fn parse_event_line(line: &str) -> Option<ShellEvent> {
    if line.is_empty() {
        return None;
    }
    if let Ok(event) = meridian_ipc::decode_event(line) {
        return Some(event);
    }

    let mut parts = line.splitn(3, ' ');
    match parts.next()? {
        "workspace-changed" => parts
            .next()
            .and_then(|workspace| workspace.parse().ok())
            .map(|workspace| ShellEvent::WorkspaceChanged { workspace }),
        "window-opened" => {
            let id = parts.next()?.to_string();
            let title = parts.next().unwrap_or("").to_string();
            Some(ShellEvent::WindowOpened { id, title })
        }
        "window-closed" => Some(ShellEvent::WindowClosed {
            id: parts.next()?.to_string(),
        }),
        "window-focused" => Some(ShellEvent::WindowFocused {
            id: parts.next()?.to_string(),
        }),
        _ => None,
    }
}

struct Painter<'a> {
    data: &'a mut [u8],
    width: i32,
    height: i32,
}

impl<'a> Painter<'a> {
    fn new(data: &'a mut [u8], width: i32, height: i32) -> Self {
        Self { data, width, height }
    }

    pub fn clear(&mut self, color: Color) {
        let pixel = argb(color).to_le_bytes();
        for chunk in self.data.chunks_exact_mut(4) {
            chunk.copy_from_slice(&pixel);
        }
    }

    pub fn roundish_rect(&mut self, rect: Rect, color: Color) {
        self.rect(rect, color);
    }

    pub fn rect(&mut self, rect: Rect, color: Color) {
        let x0 = rect.x.clamp(0, self.width);
        let y0 = rect.y.clamp(0, self.height);
        let x1 = (rect.x + rect.w).clamp(0, self.width);
        let y1 = (rect.y + rect.h).clamp(0, self.height);
        let pixel = argb(color).to_le_bytes();

        for y in y0..y1 {
            let row = (y * self.width * 4) as usize;
            for x in x0..x1 {
                let offset = row + (x * 4) as usize;
                self.data[offset..offset + 4].copy_from_slice(&pixel);
            }
        }
    }

    pub fn stroke_rect(&mut self, rect: Rect, color: Color) {
        self.rect(Rect { x: rect.x, y: rect.y, w: rect.w, h: 1 }, color);
        self.rect(Rect { x: rect.x, y: rect.y + rect.h - 1, w: rect.w, h: 1 }, color);
        self.rect(Rect { x: rect.x, y: rect.y, w: 1, h: rect.h }, color);
        self.rect(Rect { x: rect.x + rect.w - 1, y: rect.y, w: 1, h: rect.h }, color);
    }

    pub fn text_centered(
        &mut self,
        font: &RefCell<Option<TextRenderer>>,
        text: &str,
        rect: Rect,
        color: Color,
    ) {
        let approx_w = text.chars().count() as i32 * 8;
        let x = rect.x + (rect.w - approx_w).max(0) / 2;
        let baseline = rect.y + (rect.h / 2) + 5;
        self.text_clipped(font, text, x, baseline, rect.w, color);
    }

    pub fn text_clipped(
        &mut self,
        font: &RefCell<Option<TextRenderer>>,
        text: &str,
        x: i32,
        baseline: i32,
        max_w: i32,
        color: Color,
    ) {
        if max_w <= 0 {
            return;
        }
        if let Some(renderer) = font.borrow_mut().as_mut() {
            if renderer.draw_text(self, text, x, baseline, max_w, color) {
                return;
            }
        }
        self.draw_bitmap_text(text, x, baseline - 10, max_w, color);
    }

    pub fn blend_pixel(&mut self, x: i32, y: i32, color: Color, alpha: u8) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height || alpha == 0 {
            return;
        }
        let offset = ((y * self.width + x) * 4) as usize;
        let src_a = (u16::from(color.a) * u16::from(alpha)) / 255;
        let inv_a = 255 - src_a;

        let dst_b = u16::from(self.data[offset]);
        let dst_g = u16::from(self.data[offset + 1]);
        let dst_r = u16::from(self.data[offset + 2]);

        self.data[offset] = ((u16::from(color.b) * src_a + dst_b * inv_a) / 255) as u8;
        self.data[offset + 1] = ((u16::from(color.g) * src_a + dst_g * inv_a) / 255) as u8;
        self.data[offset + 2] = ((u16::from(color.r) * src_a + dst_r * inv_a) / 255) as u8;
        self.data[offset + 3] = 255;
    }

    fn draw_bitmap_text(&mut self, text: &str, x: i32, y: i32, max_w: i32, color: Color) {
        let mut cursor = x;
        for ch in text.chars() {
            if cursor + 6 > x + max_w {
                break;
            }
            let glyph = bitmap_glyph(ch);
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..5 {
                    if bits & (1 << (4 - col)) != 0 {
                        self.blend_pixel(cursor + col, y + row as i32, color, 255);
                    }
                }
            }
            cursor += 6;
        }
    }
}

fn bitmap_glyph(ch: char) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        '0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        '1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        '2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        '3' => [0x1e, 0x01, 0x01, 0x0e, 0x01, 0x01, 0x1e],
        '4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        '5' => [0x1f, 0x10, 0x10, 0x1e, 0x01, 0x01, 0x1e],
        '6' => [0x0e, 0x10, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        '7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        '9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x01, 0x0e],
        'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        'C' => [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        'D' => [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
        'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        'G' => [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0e],
        'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x12, 0x12, 0x0c],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        'Q' => [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x0a, 0x0a, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1b, 0x11],
        'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        ':' => [0x00, 0x04, 0x04, 0x00, 0x04, 0x04, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c],
        '-' => [0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1f],
        '/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        ' ' => [0x00; 7],
        _ => [0x1f, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
    }
}

fn argb(color: Color) -> u32 {
    (u32::from(color.a) << 24)
        | (u32::from(color.r) << 16)
        | (u32::from(color.g) << 8)
        | u32::from(color.b)
}

struct TextRenderer {
    face: ft::Face,
    library: ft::Library,
}

impl TextRenderer {
    fn new(pattern: &str, pixels: u32) -> Option<Self> {
        let font_path = fontconfig_match(pattern).or_else(|| fontconfig_match("sans"))?;
        let library = ft::Library::new().ok()?;
        let face = ft::Face::new(&library, &font_path, pixels).ok()?;
        Some(Self { face, library })
    }

    fn draw_text(
        &mut self,
        painter: &mut Painter<'_>,
        text: &str,
        x: i32,
        baseline: i32,
        max_w: i32,
        color: Color,
    ) -> bool {
        let mut pen_x = x;
        let end_x = x + max_w;
        let mut drew = false;

        for ch in text.chars() {
            if pen_x >= end_x {
                break;
            }
            let Some(glyph) = self.face.load_char(ch) else {
                continue;
            };

            let draw_x = pen_x + glyph.left;
            let draw_y = baseline - glyph.top;
            for row in 0..glyph.rows {
                for col in 0..glyph.width {
                    let idx = if glyph.pitch >= 0 {
                        (row * glyph.pitch as u32 + col) as usize
                    } else {
                        ((glyph.rows - 1 - row) * (-glyph.pitch) as u32 + col) as usize
                    };
                    let alpha = glyph.buffer.get(idx).copied().unwrap_or(0);
                    painter.blend_pixel(draw_x + col as i32, draw_y + row as i32, color, alpha);
                    drew = drew || alpha != 0;
                }
            }
            pen_x += glyph.advance;
        }

        drew
    }
}

impl Drop for TextRenderer {
    fn drop(&mut self) {
        let _ = &self.library;
    }
}

fn fontconfig_match(pattern: &str) -> Option<PathBuf> {
    unsafe {
        if fc::FcInit() == 0 {
            return None;
        }
        let pattern = CString::new(pattern).ok()?;
        let fc_pattern = fc::FcNameParse(pattern.as_ptr() as *const fc::FcChar8);
        if fc_pattern.is_null() {
            return None;
        }

        fc::FcConfigSubstitute(ptr::null_mut(), fc_pattern, fc::FcMatchPattern);
        fc::FcDefaultSubstitute(fc_pattern);

        let mut result = fc::FcResultNoMatch;
        let match_pattern = fc::FcFontMatch(ptr::null_mut(), fc_pattern, &mut result);
        fc::FcPatternDestroy(fc_pattern);

        if match_pattern.is_null() || result != fc::FcResultMatch {
            if !match_pattern.is_null() {
                fc::FcPatternDestroy(match_pattern);
            }
            return None;
        }

        let mut file: *mut fc::FcChar8 = ptr::null_mut();
        let key = CString::new("file").ok()?;
        let get_result = fc::FcPatternGetString(match_pattern, key.as_ptr(), 0, &mut file);
        let path = if get_result == fc::FcResultMatch && !file.is_null() {
            CStr::from_ptr(file as *const libc::c_char)
                .to_str()
                .ok()
                .map(PathBuf::from)
        } else {
            None
        };
        fc::FcPatternDestroy(match_pattern);
        path
    }
}

fn formatted_time() -> String {
    unsafe {
        let mut now = libc::time(ptr::null_mut());
        let mut tm = std::mem::zeroed::<libc::tm>();
        if libc::localtime_r(&mut now, &mut tm).is_null() {
            return String::new();
        }
        let mut out = [0_i8; 64];
        let fmt = CString::new("%H:%M  %d.%m.%Y").expect("valid strftime format");
        let len = libc::strftime(out.as_mut_ptr(), out.len(), fmt.as_ptr(), &tm);
        if len == 0 {
            String::new()
        } else {
            CStr::from_ptr(out.as_ptr()).to_string_lossy().into_owned()
        }
    }
}

mod fc {
    #![allow(non_camel_case_types, non_upper_case_globals)]

    use libc::{c_char, c_int, c_uchar, c_void};

    pub type FcChar8 = c_uchar;
    pub type FcBool = c_int;
    pub enum FcConfig {}
    pub enum FcPattern {}
    pub type FcResult = c_int;

    pub const FcMatchPattern: c_int = 0;
    pub const FcResultMatch: FcResult = 0;
    pub const FcResultNoMatch: FcResult = 1;

    #[link(name = "fontconfig")]
    extern "C" {
        pub fn FcInit() -> FcBool;
        pub fn FcNameParse(name: *const FcChar8) -> *mut FcPattern;
        pub fn FcConfigSubstitute(
            config: *mut FcConfig,
            pattern: *mut FcPattern,
            kind: c_int,
        ) -> FcBool;
        pub fn FcDefaultSubstitute(pattern: *mut FcPattern);
        pub fn FcFontMatch(
            config: *mut FcConfig,
            pattern: *mut FcPattern,
            result: *mut FcResult,
        ) -> *mut FcPattern;
        pub fn FcPatternGetString(
            pattern: *const FcPattern,
            object: *const c_char,
            n: c_int,
            s: *mut *mut FcChar8,
        ) -> FcResult;
        pub fn FcPatternDestroy(pattern: *mut FcPattern);
    }

    #[allow(dead_code)]
    type _KeepVoid = c_void;
}

mod ft {
    #![allow(non_camel_case_types, non_snake_case)]

    use std::{
        ffi::CString,
        os::raw::{c_char, c_int, c_long, c_uint, c_ulong, c_void},
        path::Path,
        ptr, slice,
    };

    pub struct Library(FT_Library);
    pub struct Face(FT_Face);

    pub struct GlyphBitmap {
        pub buffer: Vec<u8>,
        pub width: u32,
        pub rows: u32,
        pub pitch: i32,
        pub left: i32,
        pub top: i32,
        pub advance: i32,
    }

    impl Library {
        pub fn new() -> Result<Self, c_int> {
            let mut library = ptr::null_mut();
            let err = unsafe { FT_Init_FreeType(&mut library) };
            if err == 0 {
                Ok(Self(library))
            } else {
                Err(err)
            }
        }
    }

    impl Drop for Library {
        fn drop(&mut self) {
            unsafe {
                FT_Done_FreeType(self.0);
            }
        }
    }

    impl Face {
        pub fn new(library: &Library, path: &Path, pixels: u32) -> Result<Self, c_int> {
            let path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| -1)?;
            let mut face = ptr::null_mut();
            let err = unsafe { FT_New_Face(library.0, path.as_ptr(), 0, &mut face) };
            if err != 0 {
                return Err(err);
            }

            let err = unsafe { FT_Set_Pixel_Sizes(face, 0, pixels) };
            if err != 0 {
                unsafe {
                    FT_Done_Face(face);
                }
                return Err(err);
            }

            Ok(Self(face))
        }

        pub fn load_char(&mut self, ch: char) -> Option<GlyphBitmap> {
            let err = unsafe { FT_Load_Char(self.0, ch as c_ulong, FT_LOAD_RENDER) };
            if err != 0 {
                return None;
            }

            unsafe {
                let slot = (*self.0).glyph;
                if slot.is_null() {
                    return None;
                }
                let bitmap = &(*slot).bitmap;
                let len = bitmap.rows as usize * bitmap.pitch.unsigned_abs() as usize;
                let buffer = if bitmap.buffer.is_null() || len == 0 {
                    Vec::new()
                } else {
                    slice::from_raw_parts(bitmap.buffer, len).to_vec()
                };

                Some(GlyphBitmap {
                    buffer,
                    width: bitmap.width,
                    rows: bitmap.rows,
                    pitch: bitmap.pitch,
                    left: (*slot).bitmap_left,
                    top: (*slot).bitmap_top,
                    advance: ((*slot).advance.x >> 6) as i32,
                })
            }
        }
    }

    impl Drop for Face {
        fn drop(&mut self) {
            unsafe {
                FT_Done_Face(self.0);
            }
        }
    }

    const FT_LOAD_RENDER: c_int = 4;

    type FT_Library = *mut c_void;
    type FT_Face = *mut FT_FaceRec;
    type FT_GlyphSlot = *mut FT_GlyphSlotRec;
    type FT_Size = *mut c_void;
    type FT_CharMap = *mut c_void;
    type FT_Driver = *mut c_void;
    type FT_Memory = *mut c_void;
    type FT_Stream = *mut c_void;
    type FT_Face_Internal = *mut c_void;
    type FT_SubGlyph = *mut c_void;
    type FT_Slot_Internal = *mut c_void;
    type FT_Pos = c_long;
    type FT_Fixed = c_long;

    #[repr(C)]
    struct FT_Generic {
        data: *mut c_void,
        finalizer: *mut c_void,
    }

    #[repr(C)]
    struct FT_BBox {
        x_min: FT_Pos,
        y_min: FT_Pos,
        x_max: FT_Pos,
        y_max: FT_Pos,
    }

    #[repr(C)]
    struct FT_Vector {
        x: FT_Pos,
        y: FT_Pos,
    }

    #[repr(C)]
    struct FT_Bitmap {
        rows: c_uint,
        width: c_uint,
        pitch: c_int,
        buffer: *mut u8,
        num_grays: u16,
        pixel_mode: u8,
        palette_mode: u8,
        palette: *mut c_void,
    }

    #[repr(C)]
    struct FT_Glyph_Metrics {
        width: FT_Pos,
        height: FT_Pos,
        hori_bearing_x: FT_Pos,
        hori_bearing_y: FT_Pos,
        hori_advance: FT_Pos,
        vert_bearing_x: FT_Pos,
        vert_bearing_y: FT_Pos,
        vert_advance: FT_Pos,
    }

    #[repr(C)]
    struct FT_Bitmap_Size {
        height: i16,
        width: i16,
        size: FT_Pos,
        x_ppem: FT_Pos,
        y_ppem: FT_Pos,
    }

    #[repr(C)]
    struct FT_ListRec {
        head: *mut c_void,
        tail: *mut c_void,
    }

    #[repr(C)]
    struct FT_Outline {
        n_contours: i16,
        n_points: i16,
        points: *mut FT_Vector,
        tags: *mut c_char,
        contours: *mut i16,
        flags: c_int,
    }

    #[repr(C)]
    struct FT_GlyphSlotRec {
        library: FT_Library,
        face: FT_Face,
        next: FT_GlyphSlot,
        glyph_index: c_uint,
        generic: FT_Generic,
        metrics: FT_Glyph_Metrics,
        linear_hori_advance: FT_Fixed,
        linear_vert_advance: FT_Fixed,
        advance: FT_Vector,
        format: c_uint,
        bitmap: FT_Bitmap,
        bitmap_left: c_int,
        bitmap_top: c_int,
        outline: FT_Outline,
        num_subglyphs: c_uint,
        subglyphs: FT_SubGlyph,
        control_data: *mut c_void,
        control_len: c_long,
        lsb_delta: FT_Pos,
        rsb_delta: FT_Pos,
        other: *mut c_void,
        internal: FT_Slot_Internal,
    }

    #[repr(C)]
    struct FT_FaceRec {
        num_faces: c_long,
        face_index: c_long,
        face_flags: c_long,
        style_flags: c_long,
        num_glyphs: c_long,
        family_name: *mut c_char,
        style_name: *mut c_char,
        num_fixed_sizes: c_int,
        available_sizes: *mut FT_Bitmap_Size,
        num_charmaps: c_int,
        charmaps: *mut FT_CharMap,
        generic: FT_Generic,
        bbox: FT_BBox,
        units_per_em: u16,
        ascender: i16,
        descender: i16,
        height: i16,
        max_advance_width: i16,
        max_advance_height: i16,
        underline_position: i16,
        underline_thickness: i16,
        glyph: FT_GlyphSlot,
        size: FT_Size,
        charmap: FT_CharMap,
        driver: FT_Driver,
        memory: FT_Memory,
        stream: FT_Stream,
        sizes_list: FT_ListRec,
        autohint: FT_Generic,
        extensions: *mut c_void,
        internal: FT_Face_Internal,
    }

    #[link(name = "freetype")]
    extern "C" {
        fn FT_Init_FreeType(alibrary: *mut FT_Library) -> c_int;
        fn FT_Done_FreeType(library: FT_Library) -> c_int;
        fn FT_New_Face(
            library: FT_Library,
            filepathname: *const c_char,
            face_index: c_long,
            aface: *mut FT_Face,
        ) -> c_int;
        fn FT_Done_Face(face: FT_Face) -> c_int;
        fn FT_Set_Pixel_Sizes(face: FT_Face, pixel_width: c_uint, pixel_height: c_uint) -> c_int;
        fn FT_Load_Char(face: FT_Face, char_code: c_ulong, load_flags: c_int) -> c_int;
    }
}
