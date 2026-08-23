fn create_anonymous_shm() -> Option<libc::c_int> {
    #[cfg(target_os = "openbsd")]
    {
        unsafe extern "C" {
            fn shm_mkstemp(template: *mut libc::c_char) -> libc::c_int;
        }

        let mut template = *b"/meridian-lock.XXXXXXXXXX\0";
        // SAFETY: the template is writable, NUL-terminated, and has the six
        // trailing X characters required by OpenBSD's shm_mkstemp(3).
        let fd = unsafe { shm_mkstemp(template.as_mut_ptr().cast()) };
        if fd < 0 {
            tracing::error!(
                error = %std::io::Error::last_os_error(),
                "failed to create OpenBSD lock shared memory"
            );
            return None;
        }
        if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            tracing::error!(
                error = %std::io::Error::last_os_error(),
                "failed to mark OpenBSD lock shared memory close-on-exec"
            );
            if fd >= 0 {
                unsafe { libc::close(fd) };
            }
            return None;
        }
        Some(fd)
    }

    #[cfg(not(target_os = "openbsd"))]
    {
        let name = b"meridian-lock-shm\0";
        // SAFETY: name is a static NUL-terminated C string.
        let fd = unsafe { libc::memfd_create(name.as_ptr().cast(), libc::MFD_CLOEXEC) };
        (fd >= 0).then_some(fd)
    }
}

fn create_shm_buffer(
    shm: &wl_shm::WlShm,
    width: u32,
    height: u32,
    qh: &QueueHandle<AppState>,
) -> Option<(*mut u8, usize, wl_buffer::WlBuffer)> {
    let stride = width * 4;
    let size = (stride * height) as usize;

    let raw_fd = create_anonymous_shm()?;
    if unsafe { libc::ftruncate(raw_fd, size as libc::off_t) } != 0 {
        tracing::error!(
            error = %std::io::Error::last_os_error(),
            "failed to size lock shared memory"
        );
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
        tracing::error!(
            error = %std::io::Error::last_os_error(),
            "failed to map lock shared memory"
        );
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

fn draw_lock_icon(pm: &mut PixmapMut, cx: f32, cy: f32, col: u32, bg: u32) {
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
    fill_circle(pm, cx, by + 8.0, 4.0, bg);
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
    kp.set_color(color(bg));
    pm.stroke_path(&path_keyhole, &kp, &sp2, Transform::identity(), None);
}

struct TextMetrics {
    total_advance: f32,
    ascent: f32,
    descent: f32,
}

fn measure_text(size: f32, text: &str) -> TextMetrics {
    let (width, _) = meridian_ui::measure_text(text, size);
    let (ascent, descent) = meridian_ui::ui_line_metrics(size);
    TextMetrics {
        total_advance: width as f32,
        ascent,
        descent,
    }
}

fn draw_text(pm: &mut PixmapMut, size: f32, pen_x: f32, baseline_y: f32, text: &str, col: u32) {
    let a = ((col >> 24) & 0xff) as u8;
    let r = ((col >> 16) & 0xff) as u8;
    let g = ((col >> 8) & 0xff) as u8;
    let b = (col & 0xff) as u8;
    meridian_ui::paint_text(
        pm,
        text,
        pen_x.round() as i32,
        baseline_y.round() as i32,
        size,
        meridian_tokens::Color::rgba(r, g, b, a),
    );
}

fn draw_text_centered(pm: &mut PixmapMut, size: f32, cx: f32, top_y: f32, text: &str, col: u32) {
    let m = measure_text(size, text);
    let pen_x = cx - m.total_advance / 2.0;
    let baseline_y = top_y + m.ascent;
    draw_text(pm, size, pen_x, baseline_y, text, col);
}
