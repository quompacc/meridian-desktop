fn render_frame(
    width: u32,
    height: u32,
    password_len: usize,
    username: &str,
    status: &LockStatus,
    style: &LockStyle,
) -> Vec<u8> {
    let w = width;
    let h = height;
    let mut pm = Pixmap::new(w, h).expect("pixmap");
    let mut pm_mut = pm.as_mut();

    // Background
    fill_rect(&mut pm_mut, 0.0, 0.0, w as f32, h as f32, 0.0, style.bg);

    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;

    // Card
    let card_x = cx - CARD_W / 2.0;
    let card_y = cy - CARD_H / 2.0;
    fill_rect(
        &mut pm_mut,
        card_x,
        card_y,
        CARD_W,
        CARD_H,
        style.card_radius,
        style.card,
    );

    // Lock icon
    draw_lock_icon(&mut pm_mut, cx, card_y + 50.0, style.accent, style.bg);

    // "Meridian Desktop" title
    draw_text_centered(
        &mut pm_mut,
        20.0,
        cx,
        card_y + 82.0,
        "Meridian Desktop",
        style.text,
    );

    // Username
    draw_text_centered(&mut pm_mut, 14.0, cx, card_y + 114.0, username, style.dim);

    // Password field
    let field_x = cx - FIELD_W / 2.0;
    let field_y = card_y + 150.0;
    let field_border_col = if status == &LockStatus::Failed {
        style.err
    } else {
        style.field_border
    };
    fill_rect(
        &mut pm_mut,
        field_x - 1.0,
        field_y - 1.0,
        FIELD_W + 2.0,
        FIELD_H + 2.0,
        style.control_radius + 1.0,
        field_border_col,
    );
    fill_rect(
        &mut pm_mut,
        field_x,
        field_y,
        FIELD_W,
        FIELD_H,
        style.control_radius,
        style.field_bg,
    );

    // Password dots
    let dot_r = 5.0;
    let dot_gap = 14.0;
    let total_dots_w = password_len as f32 * (dot_r * 2.0 + dot_gap) - dot_gap;
    let dots_start_x = cx - total_dots_w / 2.0 + dot_r;
    let dots_y = field_y + FIELD_H / 2.0;
    for i in 0..password_len.min(26) {
        let dx = dots_start_x + i as f32 * (dot_r * 2.0 + dot_gap);
        fill_circle(&mut pm_mut, dx, dots_y, dot_r, style.dot);
    }
    if password_len == 0 {
        // Placeholder text
        let m = measure_text(14.0, "Passwort eingeben");
        draw_text(
            &mut pm_mut,
            14.0,
            field_x + (FIELD_W - m.total_advance) / 2.0,
            field_y + (FIELD_H / 2.0) + (m.ascent - (m.ascent - m.descent) / 2.0),
            "Passwort eingeben",
            style.dim,
        );
    }

    // Status text
    let status_y = field_y + FIELD_H + 12.0;
    let (status_text, status_col) = match status {
        LockStatus::Idle => ("Drücke Enter zum Entsperren", style.dim),
        LockStatus::Pending => ("Authentifizierung …", style.text),
        LockStatus::Failed => ("Falsches Passwort", style.err),
    };
    draw_text_centered(&mut pm_mut, 13.0, cx, status_y, status_text, status_col);

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

    let pixels = render_frame(
        w,
        h,
        state.password.len(),
        &state.username,
        &state.status,
        &state.style,
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
