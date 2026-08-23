mod auth;

use std::os::unix::io::{AsRawFd, BorrowedFd};
use std::sync::mpsc;

use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, PixmapMut, Transform};
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_buffer, wl_compositor, wl_keyboard, wl_output, wl_registry, wl_seat, wl_shm,
        wl_shm_pool, wl_surface,
    },
    Connection, Dispatch, QueueHandle, WEnum,
};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1, ext_session_lock_surface_v1, ext_session_lock_v1,
};
use xkbcommon::xkb;
use zeroize::Zeroizing;

/// Lock-screen colours + corner radii, derived once from the active theme so the
/// lock matches the desktop instead of a hardcoded palette. `0xAARRGGBB` to feed
/// the existing `color()` helper. Falls back to the built-in default theme when
/// no config/theme can be read (see `LockStyle::load`).
struct LockStyle {
    bg: u32,
    card: u32,
    field_bg: u32,
    field_border: u32,
    accent: u32,
    text: u32,
    dim: u32,
    dot: u32,
    err: u32,
    card_radius: f32,
    control_radius: f32,
}

impl LockStyle {
    fn from_theme(theme: &meridian_config::ThemeConfig) -> Self {
        let c = &theme.colors;
        let argb = |col: meridian_config::Color| -> u32 {
            ((col.a as u32) << 24) | ((col.r as u32) << 16) | ((col.g as u32) << 8) | (col.b as u32)
        };
        Self {
            bg: argb(c.background),
            card: argb(c.surface_alt),
            field_bg: argb(c.surface),
            field_border: argb(c.border),
            accent: argb(c.accent),
            text: argb(c.text),
            dim: argb(c.text_dim),
            dot: argb(c.accent),
            err: argb(c.error),
            card_radius: theme
                .decorations
                .surface_radius(meridian_config::ThemeSurface::Modal),
            control_radius: theme
                .decorations
                .surface_radius(meridian_config::ThemeSurface::Control),
        }
    }

    /// Resolve the user's configured theme (same source as the shell/greeter),
    /// falling back to the built-in default if config or theme are unreadable.
    fn load() -> Self {
        let config = meridian_config::MeridianConfig::load();
        let mut manager = meridian_config::ThemeManager::new();
        let name = config.general.theme.trim();
        let name = if name.is_empty() { "dark" } else { name };
        if let Err(err) = manager.set_theme(name) {
            tracing::warn!(theme = name, error = %err, "lock theme load failed; using default");
        }
        Self::from_theme(&manager.current().config)
    }
}

const CARD_W: f32 = 460.0;
const CARD_H: f32 = 310.0;
const FIELD_W: f32 = 380.0;
const FIELD_H: f32 = 42.0;

// ── Wayland state ─────────────────────────────────────────────────────────────

struct LockSurface {
    surface: wl_surface::WlSurface,
    lock_surface: ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
    width: u32,
    height: u32,
    needs_render: bool,
    background_initialized: bool,
    shm_ptr: *mut u8,
    shm_size: usize,
    buffer: Option<wl_buffer::WlBuffer>,
}

// Safety: AppState stays on a single thread
unsafe impl Send for AppState {}

struct AppState {
    running: bool,
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    seat: Option<wl_seat::WlSeat>,
    lock_manager: Option<ext_session_lock_manager_v1::ExtSessionLockManagerV1>,
    lock: Option<ext_session_lock_v1::ExtSessionLockV1>,
    session_locked: bool,
    finished: bool,
    pending_outputs: Vec<wl_output::WlOutput>,
    lock_surfaces: Vec<LockSurface>,
    xkb_ctx: xkb::Context,
    xkb_state: Option<xkb::State>,
    password: Zeroizing<String>,
    username: String,
    status: LockStatus,
    auth_rx: Option<mpsc::Receiver<bool>>,
    style: LockStyle,
}

#[derive(Clone, PartialEq)]
enum LockStatus {
    Idle,
    Pending,
    Failed,
}

impl AppState {
    fn mark_all_dirty(&mut self) {
        for ls in &mut self.lock_surfaces {
            ls.needs_render = true;
        }
    }
}

// ── Registry dispatch: bind globals ──────────────────────────────────────────

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
                    state.compositor = Some(registry.bind::<wl_compositor::WlCompositor, _, _>(
                        name,
                        version.min(5),
                        qh,
                        (),
                    ));
                }
                "wl_shm" => {
                    state.shm = Some(registry.bind::<wl_shm::WlShm, _, _>(name, 1, qh, ()));
                }
                "wl_seat" => {
                    state.seat =
                        Some(registry.bind::<wl_seat::WlSeat, _, _>(name, version.min(7), qh, ()));
                }
                "wl_output" => {
                    let output =
                        registry.bind::<wl_output::WlOutput, _, _>(name, version.min(3), qh, ());
                    state.pending_outputs.push(output);
                }
                "ext_session_lock_manager_v1" => {
                    state.lock_manager = Some(
                        registry
                            .bind::<ext_session_lock_manager_v1::ExtSessionLockManagerV1, _, _>(
                                name,
                                1,
                                qh,
                                (),
                            ),
                    );
                }
                _ => {}
            }
        }
    }
}

// ── Output events (mostly ignore; size comes from lock surface configure) ────

impl Dispatch<wl_output::WlOutput, ()> for AppState {
    fn event(
        _state: &mut Self,
        _output: &wl_output::WlOutput,
        _event: wl_output::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

// ── Seat / Keyboard ───────────────────────────────────────────────────────────

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
        _qh: &QueueHandle<Self>,
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
                let len = size_usize.saturating_sub(1); // strip trailing NUL
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
            } => {
                handle_key(state, key);
            }
            _ => {}
        }
    }
}

fn handle_key(state: &mut AppState, linux_key: u32) {
    // Don't accept input while auth is in progress
    if state.auth_rx.is_some() {
        return;
    }

    let xkb_key = xkb::Keycode::new(linux_key + 8);
    let sym = state
        .xkb_state
        .as_ref()
        .map(|s| s.key_get_one_sym(xkb_key))
        .unwrap_or(xkb::Keysym::new(0));

    match sym.raw() {
        xkbcommon::xkb::keysyms::KEY_Return | xkbcommon::xkb::keysyms::KEY_KP_Enter => {
            if state.password.is_empty() {
                return;
            }
            let username = state.username.clone();
            let password = state.password.clone();
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let ok = auth::authenticate(&username, &password);
                let _ = tx.send(ok);
            });
            state.auth_rx = Some(rx);
            state.status = LockStatus::Pending;
            state.mark_all_dirty();
        }
        xkbcommon::xkb::keysyms::KEY_BackSpace => {
            // Remove last UTF-8 character
            let mut s = std::mem::take(&mut *state.password);
            let new_len = s.char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
            s.truncate(new_len);
            *state.password = s;
            if state.status == LockStatus::Failed {
                state.status = LockStatus::Idle;
            }
            state.mark_all_dirty();
        }
        xkbcommon::xkb::keysyms::KEY_Escape => {
            // Clear password field on Escape (but don't exit)
            if !state.password.is_empty() {
                *state.password = String::new();
                state.status = LockStatus::Idle;
                state.mark_all_dirty();
            }
        }
        _ => {
            let utf8 = state
                .xkb_state
                .as_ref()
                .map(|s| s.key_get_utf8(xkb_key))
                .unwrap_or_default();
            for ch in utf8.chars() {
                if !ch.is_control() {
                    state.password.push(ch);
                    if state.status == LockStatus::Failed {
                        state.status = LockStatus::Idle;
                    }
                    state.mark_all_dirty();
                }
            }
        }
    }
}

// ── Session lock protocol ─────────────────────────────────────────────────────

impl Dispatch<ext_session_lock_manager_v1::ExtSessionLockManagerV1, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &ext_session_lock_manager_v1::ExtSessionLockManagerV1,
        _: ext_session_lock_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ext_session_lock_v1::ExtSessionLockV1, ()> for AppState {
    fn event(
        state: &mut Self,
        _: &ext_session_lock_v1::ExtSessionLockV1,
        event: ext_session_lock_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_session_lock_v1::Event::Locked => {
                state.session_locked = true;
                tracing::info!("session locked");
            }
            ext_session_lock_v1::Event::Finished => {
                // Compositor refused the lock (another client already locked, or policy)
                tracing::error!("session lock refused by compositor (finished event)");
                state.finished = true;
                state.running = false;
            }
            _ => {}
        }
    }
}

impl Dispatch<ext_session_lock_surface_v1::ExtSessionLockSurfaceV1, ()> for AppState {
    fn event(
        state: &mut Self,
        lock_surface_proxy: &ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
        event: ext_session_lock_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_session_lock_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            lock_surface_proxy.ack_configure(serial);
            let w = width.max(1);
            let h = height.max(1);
            if let Some(ls) = state
                .lock_surfaces
                .iter_mut()
                .find(|ls| &ls.lock_surface == lock_surface_proxy)
            {
                if ls.width != w || ls.height != h {
                    if !ls.shm_ptr.is_null() {
                        unsafe { libc::munmap(ls.shm_ptr as *mut _, ls.shm_size) };
                        ls.shm_ptr = std::ptr::null_mut();
                        ls.shm_size = 0;
                    }
                    ls.buffer = None;
                    ls.background_initialized = false;
                    ls.width = w;
                    ls.height = h;
                }
                ls.needs_render = true;
            }
        }
    }
}

// ── No-op delegates ───────────────────────────────────────────────────────────

delegate_noop!(AppState: ignore wl_compositor::WlCompositor);
delegate_noop!(AppState: ignore wl_surface::WlSurface);
delegate_noop!(AppState: ignore wl_shm::WlShm);
delegate_noop!(AppState: ignore wl_shm_pool::WlShmPool);
delegate_noop!(AppState: ignore wl_buffer::WlBuffer);

// ── SHM buffer helpers ────────────────────────────────────────────────────────

include!("main/render_helpers.rs");
include!("main/render.rs");
include!("main/runtime.rs");

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
