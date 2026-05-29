// Wayland side of the polkit agent. Raw wayland-client + wlr-layer-shell,
// modelled on meridian-lock (single layer surface, tiny-skia render, xkb
// keyboard) but with the surface created on demand instead of always-on,
// and driven by polkit BeginAuthentication requests instead of a session
// lock.
//
// We deliberately do NOT use smithay-client-toolkit here: meridian-lock's
// raw dispatch idiom is a smaller surface area and matches the rest of
// the codebase.

use std::os::fd::AsFd;
use std::os::raw::c_void;
use std::os::unix::io::AsRawFd;

use ab_glyph::FontRef;
use meridian_config::{MeridianConfig, ThemeConfig, ThemeManager};
use smithay_client_toolkit::reexports::calloop::channel as cchannel;
use tracing::{debug, info, warn};
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_buffer, wl_compositor, wl_keyboard, wl_output, wl_registry, wl_seat, wl_shm,
        wl_shm_pool, wl_surface,
    },
    Connection, Dispatch, EventQueue, QueueHandle, WEnum,
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};
use xkbcommon::xkb;
use zeroize::Zeroizing;

use crate::dbus::{AuthRequest, Identity, Outcome};
use crate::ui;

pub struct PamResult {
    pub cookie: String,
    pub ok: bool,
}

pub struct ActiveAuth {
    // Nur fuers Logging/zukuenftige Anzeige gehalten.
    #[allow(dead_code)]
    pub action_id: String,
    pub message: String,
    pub cookie: String,
    pub identity: Identity,
    pub password: Zeroizing<String>,
    pub status: ui::Status,
    pub retries: u32,
    pub reply: Option<tokio::sync::oneshot::Sender<Outcome>>,
}

pub struct AppState {
    pub running: bool,
    pub font: FontRef<'static>,
    pub theme: ThemeConfig,

    // Wayland globals (set in registry dispatch)
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    seat: Option<wl_seat::WlSeat>,
    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,

    // Keyboard
    xkb_ctx: xkb::Context,
    xkb_state: Option<xkb::State>,

    // Active popup
    active: Option<ActiveAuth>,
    popup: Option<PopupSurface>,

    // PAM result channel
    pub pam_tx: cchannel::Sender<PamResult>,
}

struct PopupSurface {
    surface: wl_surface::WlSurface,
    layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    width: u32,
    height: u32,
    configured: bool,
    shm_ptr: *mut u8,
    shm_size: usize,
    buffer: Option<wl_buffer::WlBuffer>,
}

// PopupSurface is only ever touched on the main thread (calloop driver).
unsafe impl Send for AppState {}

const POPUP_W: u32 = 520;
const POPUP_H: u32 = 340;

impl AppState {
    pub fn new(
        font: FontRef<'static>,
        theme: ThemeConfig,
        pam_tx: cchannel::Sender<PamResult>,
    ) -> Self {
        Self {
            running: true,
            font,
            theme,
            compositor: None,
            shm: None,
            seat: None,
            layer_shell: None,
            xkb_ctx: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            xkb_state: None,
            active: None,
            popup: None,
            pam_tx,
        }
    }

    pub fn on_auth_request(&mut self, req: AuthRequest, qh: &QueueHandle<Self>) {
        // First-come-first-served. If a popup is already up, decline the
        // new one — polkit will retry. Most desktops queue; we'll add a
        // queue if it becomes a real annoyance.
        if self.active.is_some() {
            warn!(cookie = %req.cookie, "polkit: popup already active, declining new request");
            let _ = req.reply.send(Outcome::Cancelled);
            return;
        }
        // Refresh the theme from disk on every auth request so live
        // theme switches in the shell (config rewrite) take effect on
        // the next popup, not on next agent restart.
        self.reload_theme();
        let identity = req.identities.first().cloned().unwrap_or(Identity {
            uid: 0,
            username: "root".to_string(),
        });
        info!(
            action_id = %req.action_id,
            cookie = %req.cookie,
            identity = %identity.username,
            "polkit: opening auth popup"
        );
        self.active = Some(ActiveAuth {
            action_id: req.action_id,
            message: req.message,
            cookie: req.cookie,
            identity,
            password: Zeroizing::new(String::new()),
            status: ui::Status::Idle,
            retries: 0,
            reply: Some(req.reply),
        });
        self.ensure_popup(qh);
        // First draw arrives via the layer_surface Configure event.
    }

    fn reload_theme(&mut self) {
        let config = MeridianConfig::load();
        let mut manager = ThemeManager::new();
        if !config.general.theme.is_empty() && config.general.theme != manager.current().name {
            if let Err(err) = manager.set_theme(&config.general.theme) {
                warn!(
                    "polkit: failed to reload theme {:?}: {} (keeping previous)",
                    config.general.theme, err
                );
                return;
            }
        }
        let new_name = manager.current().name.clone();
        self.theme = manager.current().config.clone();
        debug!(theme = %new_name, "polkit: theme reloaded");
    }

    pub fn on_cancel_from_polkit(&mut self, cookie: String) {
        if let Some(active) = &self.active {
            if active.cookie == cookie {
                debug!(cookie = %cookie, "polkit: external cancel; closing popup");
                self.finish(Outcome::Cancelled);
            }
        }
    }

    pub fn on_pam_result(&mut self, result: PamResult) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        if active.cookie != result.cookie {
            return;
        }
        if result.ok {
            let uid = active.identity.uid;
            let username = active.identity.username.clone();
            info!(uid, username = %username, "polkit: PAM success");
            self.finish(Outcome::Authenticated { uid, username });
        } else {
            active.retries += 1;
            active.status = ui::Status::Failed;
            active.password = Zeroizing::new(String::new());
            warn!(retries = active.retries, "polkit: PAM failure");
            if active.retries >= 3 {
                info!("polkit: too many failures; declining");
                self.finish(Outcome::Cancelled);
            }
            // caller redraws via draw(&qh) after on_pam_result returns
        }
    }

    fn finish(&mut self, outcome: Outcome) {
        if let Some(mut active) = self.active.take() {
            if let Some(reply) = active.reply.take() {
                let _ = reply.send(outcome);
            }
        }
        self.destroy_popup();
    }

    fn ensure_popup(&mut self, qh: &QueueHandle<Self>) {
        if self.popup.is_some() {
            return;
        }
        let Some(compositor) = self.compositor.clone() else {
            warn!("polkit: no compositor global yet");
            return;
        };
        let Some(layer_shell) = self.layer_shell.clone() else {
            warn!("polkit: no layer_shell global yet");
            return;
        };
        let surface = compositor.create_surface(qh, ());
        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            None, // any output (the compositor picks the focused one)
            zwlr_layer_shell_v1::Layer::Overlay,
            "meridian-polkit".to_string(),
            qh,
            (),
        );
        layer_surface.set_size(POPUP_W, POPUP_H);
        layer_surface
            .set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive);
        // No anchor → centered by the compositor.
        surface.commit();
        self.popup = Some(PopupSurface {
            surface,
            layer_surface,
            width: POPUP_W,
            height: POPUP_H,
            configured: false,
            shm_ptr: std::ptr::null_mut(),
            shm_size: 0,
            buffer: None,
        });
    }

    fn destroy_popup(&mut self) {
        if let Some(mut p) = self.popup.take() {
            if let Some(buf) = p.buffer.take() {
                buf.destroy();
            }
            if !p.shm_ptr.is_null() {
                unsafe { libc::munmap(p.shm_ptr as *mut c_void, p.shm_size) };
            }
            p.layer_surface.destroy();
            p.surface.destroy();
        }
    }

    /// Render path that has access to a real QueueHandle.
    pub fn draw(&mut self, qh: &QueueHandle<Self>) {
        let (w, h) = match self.popup.as_ref() {
            Some(p) if p.configured => (p.width, p.height),
            _ => return,
        };
        let stride = (w * 4) as i32;
        let size = (stride as u32 * h) as usize;

        // (Re)allocate shm if needed
        if self
            .popup
            .as_ref()
            .map(|p| p.shm_size != size || p.buffer.is_none())
            .unwrap_or(false)
        {
            // Tear down old buffer
            {
                let p = self.popup.as_mut().unwrap();
                if let Some(buf) = p.buffer.take() {
                    buf.destroy();
                }
                if !p.shm_ptr.is_null() {
                    unsafe { libc::munmap(p.shm_ptr as *mut c_void, p.shm_size) };
                    p.shm_ptr = std::ptr::null_mut();
                    p.shm_size = 0;
                }
            }
            let fd = unsafe { libc::memfd_create(c"meridian-polkit".as_ptr(), 0) };
            if fd < 0 {
                warn!("memfd_create failed");
                return;
            }
            use std::os::fd::FromRawFd;
            let owned_fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) };
            unsafe {
                if libc::ftruncate(owned_fd.as_raw_fd(), size as i64) < 0 {
                    warn!("ftruncate failed");
                    return;
                }
            }
            let ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    size,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    owned_fd.as_raw_fd(),
                    0,
                )
            };
            if ptr == libc::MAP_FAILED {
                warn!("mmap failed");
                return;
            }
            let shm = self.shm.as_ref().unwrap();
            let pool = shm.create_pool(owned_fd.as_fd(), size as i32, qh, ());
            let buf = pool.create_buffer(
                0,
                w as i32,
                h as i32,
                stride,
                wl_shm::Format::Argb8888,
                qh,
                (),
            );
            pool.destroy();
            let p = self.popup.as_mut().unwrap();
            p.shm_ptr = ptr as *mut u8;
            p.shm_size = size;
            p.buffer = Some(buf);
        }

        let popup = self.popup.as_ref().unwrap();
        let active = match &self.active {
            Some(a) => a,
            None => return,
        };

        let pixels: &mut [u8] = unsafe { std::slice::from_raw_parts_mut(popup.shm_ptr, size) };
        let view = ui::View {
            title: "Authentifizierung erforderlich",
            message: if active.message.is_empty() {
                "Eine Anwendung benötigt Administratorrechte."
            } else {
                active.message.as_str()
            },
            username: &active.identity.username,
            password_len: active.password.chars().count(),
            status: active.status,
            hint: "Enter zum Bestätigen · Esc zum Abbrechen",
        };
        ui::render(pixels, w, h, &self.font, &self.theme, &view);

        if let Some(buf) = &popup.buffer {
            popup.surface.attach(Some(buf), 0, 0);
            popup.surface.damage_buffer(0, 0, w as i32, h as i32);
            popup.surface.commit();
        }
    }

    fn handle_key(&mut self, linux_key: u32) -> bool {
        let Some(active) = self.active.as_mut() else {
            return false;
        };
        let keycode = linux_key + 8;
        let Some(xkb_state) = self.xkb_state.as_ref() else {
            return false;
        };
        let keysym = xkb_state.key_get_one_sym(keycode.into());
        match keysym {
            xkb::Keysym::Return | xkb::Keysym::KP_Enter => {
                if active.password.is_empty() {
                    return false;
                }
                active.status = ui::Status::Checking;
                let username = active.identity.username.clone();
                let cookie = active.cookie.clone();
                let password =
                    std::mem::replace(&mut active.password, Zeroizing::new(String::new()));
                // Stash for redraw: show "Checking" but keep password
                // count at 0. We took it out so the user can keep
                // typing while PAM is running; on failure we wipe.
                let cookie_for_helper = cookie.clone();
                let tx = self.pam_tx.clone();
                std::thread::spawn(move || {
                    let ok = crate::auth::authenticate_via_helper(
                        &username,
                        &cookie_for_helper,
                        &password,
                    );
                    let _ = tx.send(PamResult { cookie, ok });
                });
                true
            }
            xkb::Keysym::Escape => {
                self.finish(Outcome::Cancelled);
                true
            }
            xkb::Keysym::BackSpace => {
                active.password.pop();
                if active.status == ui::Status::Failed {
                    active.status = ui::Status::Idle;
                }
                true
            }
            _ => {
                let utf8 = xkb_state.key_get_utf8(keycode.into());
                if !utf8.is_empty() && !utf8.chars().any(|c| c.is_control()) {
                    active.password.push_str(&utf8);
                    if active.status == ui::Status::Failed {
                        active.status = ui::Status::Idle;
                    }
                    true
                } else {
                    false
                }
            }
        }
    }
}

// ── Registry ────────────────────────────────────────────────────────────────

impl Dispatch<wl_registry::WlRegistry, ()> for AppState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => {
                    state.compositor = Some(registry.bind(name, version.min(5), qh, ()));
                }
                "wl_shm" => {
                    state.shm = Some(registry.bind(name, 1, qh, ()));
                }
                "wl_seat" => {
                    state.seat = Some(registry.bind(name, version.min(7), qh, ()));
                }
                "zwlr_layer_shell_v1" => {
                    state.layer_shell = Some(registry.bind(name, version.min(4), qh, ()));
                }
                _ => {}
            }
        }
    }
}

// ── Seat / Keyboard ─────────────────────────────────────────────────────────

impl Dispatch<wl_seat::WlSeat, ()> for AppState {
    fn event(
        _state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(caps),
        } = event
        {
            if caps.contains(wl_seat::Capability::Keyboard) {
                seat.get_keyboard(qh, ());
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for AppState {
    fn event(
        state: &mut Self,
        _kbd: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap {
                format: WEnum::Value(wl_keyboard::KeymapFormat::XkbV1),
                fd,
                size,
            } => {
                let size_usize = size as usize;
                let raw = fd.as_raw_fd();
                let ptr = unsafe {
                    libc::mmap(
                        std::ptr::null_mut(),
                        size_usize,
                        libc::PROT_READ,
                        libc::MAP_PRIVATE,
                        raw,
                        0,
                    )
                };
                if ptr == libc::MAP_FAILED {
                    return;
                }
                let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, size_usize) };
                let len = size_usize.saturating_sub(1);
                if let Ok(s) = std::str::from_utf8(&bytes[..len]) {
                    if let Some(keymap) = xkb::Keymap::new_from_string(
                        &state.xkb_ctx,
                        s.to_string(),
                        xkb::KEYMAP_FORMAT_TEXT_V1,
                        0,
                    ) {
                        state.xkb_state = Some(xkb::State::new(&keymap));
                    }
                }
                unsafe { libc::munmap(ptr, size_usize) };
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                if let Some(ref mut s) = state.xkb_state {
                    s.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);
                }
            }
            wl_keyboard::Event::Key {
                key,
                state: WEnum::Value(wl_keyboard::KeyState::Pressed),
                ..
            } if state.handle_key(key) => {
                state.draw(qh);
            }
            _ => {}
        }
    }
}

// ── Layer surface ───────────────────────────────────────────────────────────

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for AppState {
    fn event(
        state: &mut Self,
        ls: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                ls.ack_configure(serial);
                if let Some(p) = state.popup.as_mut() {
                    if width > 0 {
                        p.width = width;
                    }
                    if height > 0 {
                        p.height = height;
                    }
                    p.configured = true;
                }
                state.draw(qh);
            }
            zwlr_layer_surface_v1::Event::Closed => {
                debug!("polkit: layer surface closed by compositor");
                state.finish(Outcome::Cancelled);
            }
            _ => {}
        }
    }
}

// delegate_noop! panics if an event arrives — only safe for objects
// that never emit events. Use it for the genuinely event-less ones,
// and write tiny no-op Dispatch impls for the rest. wl_shm sends
// Format events, wl_buffer sends Release, wl_output sends a stream of
// geometry events, wl_surface emits Enter/Leave when it crosses
// outputs — none of which we care about.
delegate_noop!(AppState: wl_compositor::WlCompositor);
delegate_noop!(AppState: wl_shm_pool::WlShmPool);
delegate_noop!(AppState: zwlr_layer_shell_v1::ZwlrLayerShellV1);

macro_rules! ignore_events {
    ($iface:ty) => {
        impl Dispatch<$iface, ()> for AppState {
            fn event(
                _state: &mut Self,
                _proxy: &$iface,
                _event: <$iface as wayland_client::Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        }
    };
}

ignore_events!(wl_shm::WlShm);
ignore_events!(wl_buffer::WlBuffer);
ignore_events!(wl_surface::WlSurface);
ignore_events!(wl_output::WlOutput);

// ── Bootstrap ───────────────────────────────────────────────────────────────

pub fn connect() -> Result<(Connection, EventQueue<AppState>), Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let event_queue = conn.new_event_queue::<AppState>();
    let qh = event_queue.handle();
    let display = conn.display();
    display.get_registry(&qh, ());
    Ok((conn, event_queue))
}
