mod auth;

use std::os::fd::AsFd;
use std::os::unix::io::{AsRawFd, BorrowedFd};
use std::sync::mpsc;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
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

static FONT_DATA: &[u8] = include_bytes!("../../meridian-ui/assets/fonts/AdwaitaSans-Regular.ttf");

// ── colours (RGBA, tiny-skia order) ──────────────────────────────────────────
const BG: u32 = 0xFF1A1B26;
const CARD: u32 = 0xFF1F2335;
const FIELD_BG: u32 = 0xFF24283B;
const FIELD_BORDER: u32 = 0xFF414868;
const ACCENT: u32 = 0xFF7AA2F7;
const TEXT: u32 = 0xFFC0CAF5;
const DIM: u32 = 0xFF565F89;
const DOT: u32 = 0xFF7AA2F7;
const ERR: u32 = 0xFFF7768E;

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

fn create_shm_buffer(
    shm: &wl_shm::WlShm,
    width: u32,
    height: u32,
    qh: &QueueHandle<AppState>,
) -> Option<(*mut u8, usize, wl_buffer::WlBuffer)> {
    let stride = width * 4;
    let size = (stride * height) as usize;

    let raw_fd = unsafe {
        let name = b"meridian-lock-shm\0";
        libc::memfd_create(name.as_ptr() as *const libc::c_char, libc::MFD_CLOEXEC)
    };
    if raw_fd < 0 {
        return None;
    }
    if unsafe { libc::ftruncate(raw_fd, size as libc::off_t) } != 0 {
        unsafe { libc::close(raw_fd) };
        return None;
    }
    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            raw_fd,
            0,
        )
    };
    if ptr == libc::MAP_FAILED {
        unsafe { libc::close(raw_fd) };
        return None;
    }

    let borrowed = unsafe { BorrowedFd::borrow_raw(raw_fd) };
    let pool = shm.create_pool(borrowed, size as i32, qh, ());
    let buffer = pool.create_buffer(
        0,
        width as i32,
        height as i32,
        stride as i32,
        wl_shm::Format::Argb8888,
        qh,
        (),
    );
    pool.destroy();
    unsafe { libc::close(raw_fd) };

    Some((ptr as *mut u8, size, buffer))
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn color(rgba_hex: u32) -> Color {
    let a = ((rgba_hex >> 24) & 0xff) as f32 / 255.0;
    let r = ((rgba_hex >> 16) & 0xff) as f32 / 255.0;
    let g = ((rgba_hex >> 8) & 0xff) as f32 / 255.0;
    let b = (rgba_hex & 0xff) as f32 / 255.0;
    Color::from_rgba(r, g, b, a).unwrap()
}

fn fill_rect(pm: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, r: f32, col: u32) {
    let mut pb = PathBuilder::new();
    if r <= 0.0 {
        pb.move_to(x, y);
        pb.line_to(x + w, y);
        pb.line_to(x + w, y + h);
        pb.line_to(x, y + h);
    } else {
        pb.move_to(x + r, y);
        pb.line_to(x + w - r, y);
        pb.quad_to(x + w, y, x + w, y + r);
        pb.line_to(x + w, y + h - r);
        pb.quad_to(x + w, y + h, x + w - r, y + h);
        pb.line_to(x + r, y + h);
        pb.quad_to(x, y + h, x, y + h - r);
        pb.line_to(x, y + r);
        pb.quad_to(x, y, x + r, y);
    }
    pb.close();
    let path = pb.finish().unwrap();
    let mut paint = Paint::default();
    paint.set_color(color(col));
    pm.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn fill_circle(pm: &mut PixmapMut, cx: f32, cy: f32, radius: f32, col: u32) {
    let mut pb = PathBuilder::new();
    // Approximate circle with 4 cubic bezier arcs
    let k = 0.552_284_8;
    let r = radius;
    pb.move_to(cx, cy - r);
    pb.cubic_to(cx + k * r, cy - r, cx + r, cy - k * r, cx + r, cy);
    pb.cubic_to(cx + r, cy + k * r, cx + k * r, cy + r, cx, cy + r);
    pb.cubic_to(cx - k * r, cy + r, cx - r, cy + k * r, cx - r, cy);
    pb.cubic_to(cx - r, cy - k * r, cx - k * r, cy - r, cx, cy - r);
    pb.close();
    let path = pb.finish().unwrap();
    let mut paint = Paint::default();
    paint.set_color(color(col));
    pm.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn draw_lock_icon(pm: &mut PixmapMut, cx: f32, cy: f32, col: u32) {
    let paint_col = color(col);
    // Shackle: arc from 180° to 360° at (cx, cy-10) with r=12
    let sr = 12.0;
    let sy = cy - 18.0;
    let mut pb = PathBuilder::new();
    let k = 0.552_284_8;
    // Top half of circle (the shackle)
    pb.move_to(cx - sr, sy);
    pb.cubic_to(cx - sr, sy - k * sr, cx - k * sr, sy - sr, cx, sy - sr);
    pb.cubic_to(cx + k * sr, sy - sr, cx + sr, sy - k * sr, cx + sr, sy);
    // sides down to body
    pb.line_to(cx + sr, sy + 8.0);
    // gap (not closed - we close with a line back)
    pb.move_to(cx - sr, sy + 8.0);
    pb.line_to(cx - sr, sy);
    // Draw as two separate paths with stroke
    let path_shackle = pb.finish().unwrap();
    let stroke_paint = tiny_skia::Stroke {
        width: 3.5,
        line_cap: tiny_skia::LineCap::Round,
        ..Default::default()
    };
    let mut paint = Paint::default();
    paint.set_color(paint_col);
    pm.stroke_path(
        &path_shackle,
        &paint,
        &stroke_paint,
        Transform::identity(),
        None,
    );

    // Body: rounded rect
    let bw = 32.0;
    let bh = 22.0;
    let bx = cx - bw / 2.0;
    let by = cy - 6.0;
    fill_rect(pm, bx, by, bw, bh, 5.0, col);

    // Keyhole: small circle + line
    fill_circle(pm, cx, by + 8.0, 4.0, BG);
    // Keyhole shaft
    let mut pb2 = PathBuilder::new();
    pb2.move_to(cx, by + 12.0);
    pb2.line_to(cx, by + 17.0);
    let path_keyhole = pb2.finish().unwrap();
    let sp2 = tiny_skia::Stroke {
        width: 2.5,
        line_cap: tiny_skia::LineCap::Round,
        ..Default::default()
    };
    let mut kp = Paint::default();
    kp.set_color(color(BG));
    pm.stroke_path(&path_keyhole, &kp, &sp2, Transform::identity(), None);
}

struct TextMetrics {
    total_advance: f32,
    ascent: f32,
    descent: f32,
}

fn measure_text(font: &FontRef<'_>, size: f32, text: &str) -> TextMetrics {
    let scaled = font.as_scaled(PxScale::from(size));
    let mut advance = 0.0_f32;
    let mut prev_glyph_id = None;
    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        if let Some(prev) = prev_glyph_id {
            advance += scaled.kern(prev, id);
        }
        advance += scaled.h_advance(id);
        prev_glyph_id = Some(id);
    }
    TextMetrics {
        total_advance: advance,
        ascent: scaled.ascent(),
        descent: scaled.descent(),
    }
}

fn draw_text(
    pm: &mut PixmapMut,
    font: &FontRef<'_>,
    size: f32,
    pen_x: f32,
    baseline_y: f32,
    text: &str,
    col: u32,
) {
    let scaled = font.as_scaled(PxScale::from(size));
    let pw = pm.width() as i32;
    let ph = pm.height() as i32;
    let a_f = ((col >> 24) & 0xff) as f32 / 255.0;
    let cr = ((col >> 16) & 0xff) as u8;
    let cg = ((col >> 8) & 0xff) as u8;
    let cb = (col & 0xff) as u8;
    let mut x = pen_x;
    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        let glyph = id.with_scale_and_position(PxScale::from(size), ab_glyph::point(x, baseline_y));
        if let Some(outline) = font.outline_glyph(glyph) {
            let b = outline.px_bounds();
            outline.draw(|gx, gy, alpha| {
                let px = b.min.x as i32 + gx as i32;
                let py = b.min.y as i32 + gy as i32;
                if px < 0 || py < 0 || px >= pw || py >= ph {
                    return;
                }
                let idx = (py as usize * pw as usize + px as usize) * 4;
                let data = pm.data_mut();
                let a = (alpha * a_f * 255.0).clamp(0.0, 255.0) as u32;
                for (i, &c) in [cr, cg, cb].iter().enumerate() {
                    let dst = data[idx + i] as u32;
                    data[idx + i] = ((c as u32 * a + dst * (255 - a)) / 255) as u8;
                }
            });
        }
        x += scaled.h_advance(id);
    }
}

fn draw_text_centered(
    pm: &mut PixmapMut,
    font: &FontRef<'_>,
    size: f32,
    cx: f32,
    top_y: f32,
    text: &str,
    col: u32,
) {
    let m = measure_text(font, size, text);
    let pen_x = cx - m.total_advance / 2.0;
    let baseline_y = top_y + m.ascent;
    draw_text(pm, font, size, pen_x, baseline_y, text, col);
}

fn render_frame(
    width: u32,
    height: u32,
    password_len: usize,
    username: &str,
    status: &LockStatus,
    font: &FontRef<'_>,
) -> Vec<u8> {
    let w = width;
    let h = height;
    let mut pm = Pixmap::new(w, h).expect("pixmap");
    let mut pm_mut = pm.as_mut();

    // Background
    fill_rect(&mut pm_mut, 0.0, 0.0, w as f32, h as f32, 0.0, BG);

    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;

    // Card
    let card_x = cx - CARD_W / 2.0;
    let card_y = cy - CARD_H / 2.0;
    fill_rect(&mut pm_mut, card_x, card_y, CARD_W, CARD_H, 16.0, CARD);

    // Lock icon
    draw_lock_icon(&mut pm_mut, cx, card_y + 50.0, ACCENT);

    // "Meridian Desktop" title
    draw_text_centered(
        &mut pm_mut,
        font,
        20.0,
        cx,
        card_y + 82.0,
        "Meridian Desktop",
        TEXT,
    );

    // Username
    draw_text_centered(&mut pm_mut, font, 14.0, cx, card_y + 114.0, username, DIM);

    // Password field
    let field_x = cx - FIELD_W / 2.0;
    let field_y = card_y + 150.0;
    let field_border_col = if status == &LockStatus::Failed {
        ERR
    } else {
        FIELD_BORDER
    };
    fill_rect(
        &mut pm_mut,
        field_x - 1.0,
        field_y - 1.0,
        FIELD_W + 2.0,
        FIELD_H + 2.0,
        9.0,
        field_border_col,
    );
    fill_rect(
        &mut pm_mut,
        field_x,
        field_y,
        FIELD_W,
        FIELD_H,
        8.0,
        FIELD_BG,
    );

    // Password dots
    let dot_r = 5.0;
    let dot_gap = 14.0;
    let total_dots_w = password_len as f32 * (dot_r * 2.0 + dot_gap) - dot_gap;
    let dots_start_x = cx - total_dots_w / 2.0 + dot_r;
    let dots_y = field_y + FIELD_H / 2.0;
    for i in 0..password_len.min(26) {
        let dx = dots_start_x + i as f32 * (dot_r * 2.0 + dot_gap);
        fill_circle(&mut pm_mut, dx, dots_y, dot_r, DOT);
    }
    if password_len == 0 {
        // Placeholder text
        let m = measure_text(font, 14.0, "Passwort eingeben");
        draw_text(
            &mut pm_mut,
            font,
            14.0,
            field_x + (FIELD_W - m.total_advance) / 2.0,
            field_y + (FIELD_H / 2.0) + (m.ascent - (m.ascent - m.descent) / 2.0),
            "Passwort eingeben",
            DIM,
        );
    }

    // Status text
    let status_y = field_y + FIELD_H + 12.0;
    let (status_text, status_col) = match status {
        LockStatus::Idle => ("Drücke Enter zum Entsperren", DIM),
        LockStatus::Pending => ("Authentifizierung …", TEXT),
        LockStatus::Failed => ("Falsches Passwort", ERR),
    };
    draw_text_centered(
        &mut pm_mut,
        font,
        13.0,
        cx,
        status_y,
        status_text,
        status_col,
    );

    // Convert RGBA → BGRA (wl_shm ARGB8888 is BGRA in memory)
    let mut pixels = pm.take();
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }
    pixels
}

// ── Lock surface creation ─────────────────────────────────────────────────────

fn create_lock_surface(
    state: &mut AppState,
    output: &wl_output::WlOutput,
    qh: &QueueHandle<AppState>,
) {
    let compositor = state.compositor.as_ref().unwrap();
    let lock = state.lock.as_ref().unwrap();
    let surface = compositor.create_surface(qh, ());
    let lock_surface = lock.get_lock_surface(&surface, output, qh, ());
    surface.commit();
    state.lock_surfaces.push(LockSurface {
        surface,
        lock_surface,
        width: 1,
        height: 1,
        needs_render: false,
        shm_ptr: std::ptr::null_mut(),
        shm_size: 0,
        buffer: None,
    });
}

fn render_surface(state: &mut AppState, idx: usize, qh: &QueueHandle<AppState>) {
    let ls = &state.lock_surfaces[idx];
    let w = ls.width;
    let h = ls.height;
    if w == 0 || h == 0 {
        return;
    }

    let font = FontRef::try_from_slice(FONT_DATA).unwrap();
    let pixels = render_frame(
        w,
        h,
        state.password.len(),
        &state.username,
        &state.status,
        &font,
    );

    let ls = &mut state.lock_surfaces[idx];

    // Allocate shm if needed
    if ls.shm_ptr.is_null() || ls.shm_size != pixels.len() {
        if !ls.shm_ptr.is_null() {
            unsafe { libc::munmap(ls.shm_ptr as *mut _, ls.shm_size) };
        }
        ls.buffer = None;
        let shm = state.shm.as_ref().unwrap();
        match create_shm_buffer(shm, w, h, qh) {
            Some((ptr, sz, buf)) => {
                ls.shm_ptr = ptr;
                ls.shm_size = sz;
                ls.buffer = Some(buf);
            }
            None => return,
        }
    }

    // Copy pixels into shm
    let dst = unsafe { std::slice::from_raw_parts_mut(ls.shm_ptr, pixels.len().min(ls.shm_size)) };
    dst.copy_from_slice(&pixels[..dst.len()]);

    // Attach + damage + commit
    let buf = ls.buffer.as_ref().unwrap();
    ls.surface.attach(Some(buf), 0, 0);
    ls.surface.damage_buffer(0, 0, w as i32, h as i32);
    ls.surface.commit();
    ls.needs_render = false;
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn get_username() -> String {
    if let Ok(u) = std::env::var("USER") {
        if !u.is_empty() {
            return u;
        }
    }
    // Fallback: getpwuid
    unsafe {
        let uid = libc::getuid();
        let pw = libc::getpwuid(uid);
        if !pw.is_null() {
            let name = (*pw).pw_name;
            if !name.is_null() {
                if let Ok(s) = std::ffi::CStr::from_ptr(name).to_str() {
                    return s.to_string();
                }
            }
        }
    }
    "user".to_string()
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("meridian_lock=info".parse().unwrap()),
        )
        .init();

    let conn = Connection::connect_to_env().expect("failed to connect to Wayland display");
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    let mut state = AppState {
        running: true,
        compositor: None,
        shm: None,
        seat: None,
        lock_manager: None,
        lock: None,
        session_locked: false,
        finished: false,
        pending_outputs: Vec::new(),
        lock_surfaces: Vec::new(),
        xkb_ctx: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
        xkb_state: None,
        password: Zeroizing::new(String::new()),
        username: get_username(),
        status: LockStatus::Idle,
        auth_rx: None,
    };

    let display = conn.display();
    display.get_registry(&qh, ());
    event_queue
        .roundtrip(&mut state)
        .expect("initial roundtrip");

    let lock_manager = state
        .lock_manager
        .take()
        .expect("compositor does not support ext_session_lock_manager_v1");

    let lock = lock_manager.lock(&qh, ());
    state.lock = Some(lock);

    // Create a lock surface for each output discovered so far
    let outputs: Vec<wl_output::WlOutput> = state.pending_outputs.drain(..).collect();
    let n = outputs.len();
    for output in &outputs {
        create_lock_surface(&mut state, output, &qh);
    }
    if n == 0 {
        tracing::warn!("no outputs found — locking without surfaces");
    }

    event_queue.roundtrip(&mut state).expect("lock roundtrip");
    tracing::info!("lock surfaces created for {} output(s)", n);

    // Main event loop
    loop {
        // Check auth result
        let auth_done = if let Some(ref rx) = state.auth_rx {
            match rx.try_recv() {
                Ok(true) => Some(true),
                Ok(false) => Some(false),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(false),
            }
        } else {
            None
        };

        if let Some(success) = auth_done {
            state.auth_rx = None;
            if success {
                if let Some(ref lock) = state.lock {
                    lock.unlock_and_destroy();
                }
                state.lock = None;
                let _ = conn.flush();
                tracing::info!("session unlocked — exiting");
                break;
            } else {
                state.status = LockStatus::Failed;
                *state.password = String::new();
                state.mark_all_dirty();
            }
        }

        // Render any dirty surfaces
        let surface_count = state.lock_surfaces.len();
        for i in 0..surface_count {
            if state.lock_surfaces[i].needs_render {
                render_surface(&mut state, i, &qh);
            }
        }

        if !state.running {
            break;
        }

        // Flush + poll with 20 ms timeout so we can check auth_rx
        let _ = conn.flush();
        let wl_fd = conn.as_fd().as_raw_fd();
        let mut pfd = libc::pollfd {
            fd: wl_fd,
            events: libc::POLLIN,
            revents: 0,
        };
        unsafe { libc::poll(&mut pfd, 1, 20) };

        if let Err(e) = event_queue.dispatch_pending(&mut state) {
            tracing::error!("dispatch error: {}", e);
            break;
        }
    }

    // Clean up shm mappings
    for ls in &state.lock_surfaces {
        if !ls.shm_ptr.is_null() {
            unsafe { libc::munmap(ls.shm_ptr as *mut _, ls.shm_size) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Baut einen echten us-Keymap-State, damit handle_key die Keysyms
    // genau wie zur Laufzeit auflöst (Keycode = evdev + 8).
    fn us_keymap_state() -> xkb::State {
        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap =
            xkb::Keymap::new_from_names(&ctx, "", "", "us", "", None, xkb::KEYMAP_COMPILE_NO_FLAGS)
                .expect("compile us keymap (needs xkb data installed)");
        xkb::State::new(&keymap)
    }

    fn test_state() -> AppState {
        AppState {
            running: true,
            compositor: None,
            shm: None,
            seat: None,
            lock_manager: None,
            lock: None,
            session_locked: false,
            finished: false,
            pending_outputs: Vec::new(),
            lock_surfaces: Vec::new(),
            xkb_ctx: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            xkb_state: Some(us_keymap_state()),
            password: Zeroizing::new(String::new()),
            username: "tester".to_string(),
            status: LockStatus::Idle,
            auth_rx: None,
        }
    }

    // evdev-Keycodes; handle_key addiert intern +8 auf den X/xkb-Keycode.
    const KEY_ESC: u32 = 1;
    const KEY_BACKSPACE: u32 = 14;
    const KEY_ENTER: u32 = 28;
    const KEY_A: u32 = 30;
    const KEY_S: u32 = 31;
    const KEY_D: u32 = 32;

    #[test]
    fn color_unpacks_rgba_channels() {
        assert_eq!(
            color(0xFFFFFFFF),
            Color::from_rgba(1.0, 1.0, 1.0, 1.0).unwrap()
        );
        assert_eq!(
            color(0x00000000),
            Color::from_rgba(0.0, 0.0, 0.0, 0.0).unwrap()
        );
        assert_eq!(
            color(0xFF112233),
            Color::from_rgba(
                0x11 as f32 / 255.0,
                0x22 as f32 / 255.0,
                0x33 as f32 / 255.0,
                1.0,
            )
            .unwrap()
        );
    }

    #[test]
    fn typing_appends_characters() {
        let mut state = test_state();
        handle_key(&mut state, KEY_A);
        handle_key(&mut state, KEY_S);
        handle_key(&mut state, KEY_D);
        assert_eq!(state.password.as_str(), "asd");
    }

    #[test]
    fn backspace_removes_one_ascii_char() {
        let mut state = test_state();
        state.password = Zeroizing::new("abc".to_string());
        handle_key(&mut state, KEY_BACKSPACE);
        assert_eq!(state.password.as_str(), "ab");
    }

    #[test]
    fn backspace_removes_one_full_utf8_char() {
        let mut state = test_state();
        // "a" + U+00E9 (e-acute, 2 bytes): backspace muss den ganzen
        // Codepoint entfernen, nicht nur ein Byte.
        // "a" + U+00E9 (e-acute) = bytes 0x61, 0xC3 0xA9 in UTF-8.
        let p = String::from_utf8(vec![0x61, 0xc3, 0xa9]).unwrap();
        state.password = Zeroizing::new(p);
        handle_key(&mut state, KEY_BACKSPACE);
        assert_eq!(state.password.as_str(), "a");
    }

    #[test]
    fn backspace_on_empty_password_is_noop() {
        let mut state = test_state();
        handle_key(&mut state, KEY_BACKSPACE);
        assert_eq!(state.password.as_str(), "");
    }

    #[test]
    fn escape_clears_password_and_resets_status() {
        let mut state = test_state();
        state.password = Zeroizing::new("secret".to_string());
        state.status = LockStatus::Failed;
        handle_key(&mut state, KEY_ESC);
        assert_eq!(state.password.as_str(), "");
        assert!(state.status == LockStatus::Idle);
    }

    #[test]
    fn auth_in_progress_blocks_input() {
        let mut state = test_state();
        state.password = Zeroizing::new("ab".to_string());
        let (_tx, rx) = mpsc::channel();
        state.auth_rx = Some(rx);
        handle_key(&mut state, KEY_A);
        handle_key(&mut state, KEY_BACKSPACE);
        assert_eq!(state.password.as_str(), "ab");
    }

    #[test]
    fn return_with_empty_password_does_not_start_auth() {
        let mut state = test_state();
        handle_key(&mut state, KEY_ENTER);
        assert!(state.auth_rx.is_none());
        assert!(state.status == LockStatus::Idle);
    }

    #[test]
    fn typing_resets_failed_status_to_idle() {
        let mut state = test_state();
        state.status = LockStatus::Failed;
        handle_key(&mut state, KEY_A);
        assert!(state.status == LockStatus::Idle);
        assert_eq!(state.password.as_str(), "a");
    }
}
