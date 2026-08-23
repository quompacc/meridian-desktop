fn render_card_frame(
    password_len: usize,
    username: &str,
    status: &LockStatus,
    style: &LockStyle,
) -> Vec<u8> {
    let w = CARD_W as u32;
    let h = CARD_H as u32;
    let mut pm = Pixmap::new(w, h).expect("pixmap");
    let mut pm_mut = pm.as_mut();

    // Redrawing the complete card rectangle restores its rounded corners to
    // the static lock background without repainting the full output.
    fill_rect(&mut pm_mut, 0.0, 0.0, w as f32, h as f32, 0.0, style.bg);

    let cx = w as f32 / 2.0;
    fill_rect(
        &mut pm_mut,
        0.0,
        0.0,
        CARD_W,
        CARD_H,
        style.card_radius,
        style.card,
    );

    // Lock icon
    draw_lock_icon(&mut pm_mut, cx, 50.0, style.accent, style.bg);

    // "Meridian Desktop" title
    draw_text_centered(
        &mut pm_mut,
        20.0,
        cx,
        82.0,
        "Meridian Desktop",
        style.text,
    );

    // Username
    draw_text_centered(&mut pm_mut, 14.0, cx, 114.0, username, style.dim);

    // Password field
    let field_x = cx - FIELD_W / 2.0;
    let field_y = 150.0;
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
    // ext-session-lock forbids committing before the first configure has been
    // acknowledged. The configure handler schedules the initial render, whose
    // buffer attach performs the first surface commit.
    state.lock_surfaces.push(LockSurface {
        surface,
        lock_surface,
        width: 1,
        height: 1,
        needs_render: false,
        background_initialized: false,
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

    let card_pixels = render_card_frame(
        state.password.len(),
        &state.username,
        &state.status,
        &state.style,
    );

    let ls = &mut state.lock_surfaces[idx];

    // Allocate shm if needed
    let output_size = (w as usize) * (h as usize) * 4;
    if ls.shm_ptr.is_null() || ls.shm_size != output_size {
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
                ls.background_initialized = false;
            }
            None => return,
        }
    }

    let initialize_background = !ls.background_initialized;
    let dst = unsafe { std::slice::from_raw_parts_mut(ls.shm_ptr, ls.shm_size) };
    if initialize_background {
        let a = ((state.style.bg >> 24) & 0xff) as u16;
        let premultiply = |channel: u32| -> u8 {
            (((channel as u16) * a + 127) / 255) as u8
        };
        let bg_pixel = [
            premultiply(state.style.bg & 0xff),
            premultiply((state.style.bg >> 8) & 0xff),
            premultiply((state.style.bg >> 16) & 0xff),
            a as u8,
        ];
        for pixel in dst.chunks_exact_mut(4) {
            pixel.copy_from_slice(&bg_pixel);
        }
        ls.background_initialized = true;
    }

    let card_w = CARD_W as u32;
    let card_h = CARD_H as u32;
    let card_x = (w as i64 - card_w as i64) / 2;
    let card_y = (h as i64 - card_h as i64) / 2;
    let src_x = (-card_x).max(0) as u32;
    let src_y = (-card_y).max(0) as u32;
    let dst_x = card_x.max(0) as u32;
    let dst_y = card_y.max(0) as u32;
    let copy_w = (card_w - src_x).min(w.saturating_sub(dst_x));
    let copy_h = (card_h - src_y).min(h.saturating_sub(dst_y));
    for row in 0..copy_h {
        let src_start = (((src_y + row) * card_w + src_x) * 4) as usize;
        let dst_start = (((dst_y + row) * w + dst_x) * 4) as usize;
        let row_bytes = (copy_w * 4) as usize;
        dst[dst_start..dst_start + row_bytes]
            .copy_from_slice(&card_pixels[src_start..src_start + row_bytes]);
    }

    // Attach + damage + commit
    let buf = ls.buffer.as_ref().unwrap();
    ls.surface.attach(Some(buf), 0, 0);
    if initialize_background {
        ls.surface.damage_buffer(0, 0, w as i32, h as i32);
    } else {
        ls.surface.damage_buffer(
            dst_x as i32,
            dst_y as i32,
            copy_w as i32,
            copy_h as i32,
        );
    }
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
