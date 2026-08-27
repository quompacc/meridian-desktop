fn caret_x(is_empty: bool, text_x: f32, after_text_x: f32) -> f32 {
    if is_empty {
        text_x
    } else {
        after_text_x
    }
}

fn draw_submit_button(
    pm: &mut PixmapMut,
    painter: &CompassPainter,
    rect: Rect,
    label: &str,
    alpha: f32,
) {
    let path = rounded_rect_path(rect.0, rect.1, rect.2, rect.3, control_radius());
    let mut paint = Paint::default();
    paint.set_color(theme_color(
        alpha,
        login_theme().colors.accent,
        244.0,
    ));
    paint.anti_alias = true;
    pm.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    draw_card_stroke(
        pm,
        &path,
        theme_color(alpha, login_theme().colors.accent, 72.0),
        1.0,
    );
    painter.render_text_centered(
        pm,
        TextStyle::SansRegular(17.0),
        label,
        rect.0 + rect.2 / 2.0,
        rect.1 + rect.3 / 2.0,
        theme_color(alpha, login_theme().colors.text, 250.0),
    );
}

fn draw_brand_mark(pm: &mut PixmapMut, cx: f32, cy: f32, alpha: f32) {
    let line = theme_color(alpha, login_theme().colors.accent, 104.0);
    for radius in [18.0_f32, 27.0, 36.0] {
        if let Some(circle) = PathBuilder::from_circle(cx, cy, radius) {
            draw_card_stroke(pm, &circle, line, 1.0);
        }
    }

    let mut axes = PathBuilder::new();
    axes.move_to(cx, cy - 38.0);
    axes.line_to(cx, cy + 38.0);
    axes.move_to(cx - 38.0, cy);
    axes.line_to(cx + 38.0, cy);
    if let Some(path) = axes.finish() {
        draw_card_stroke(pm, &path, line, 1.0);
    }

    let mut rose = PathBuilder::new();
    rose.move_to(cx, cy - 26.0);
    rose.line_to(cx - 4.0, cy - 4.0);
    rose.line_to(cx, cy);
    rose.line_to(cx + 4.0, cy - 4.0);
    rose.close();
    rose.move_to(cx + 26.0, cy);
    rose.line_to(cx + 4.0, cy - 4.0);
    rose.line_to(cx, cy);
    rose.line_to(cx + 4.0, cy + 4.0);
    rose.close();
    rose.move_to(cx, cy + 26.0);
    rose.line_to(cx + 4.0, cy + 4.0);
    rose.line_to(cx, cy);
    rose.line_to(cx - 4.0, cy + 4.0);
    rose.close();
    rose.move_to(cx - 26.0, cy);
    rose.line_to(cx - 4.0, cy + 4.0);
    rose.line_to(cx, cy);
    rose.line_to(cx - 4.0, cy - 4.0);
    rose.close();
    if let Some(path) = rose.finish() {
        let mut paint = Paint::default();
        paint.set_color(theme_color(alpha, login_theme().colors.accent, 150.0));
        paint.anti_alias = true;
        pm.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    if let Some(dot) = PathBuilder::from_circle(cx, cy, 3.2) {
        let mut paint = Paint::default();
        paint.set_color(theme_color(alpha, login_theme().colors.accent, 245.0));
        paint.anti_alias = true;
        pm.fill_path(&dot, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_user_icon(pm: &mut PixmapMut, cx: f32, cy: f32, alpha: f32) {
    let color = theme_color(alpha, login_theme().colors.text_dim, 205.0);
    if let Some(head) = PathBuilder::from_circle(cx, cy - 7.0, 5.5) {
        draw_card_stroke(pm, &head, color, 1.4);
    }
    let mut body = PathBuilder::new();
    body.move_to(cx - 9.0, cy + 10.0);
    body.quad_to(cx - 8.0, cy + 1.0, cx, cy + 1.0);
    body.quad_to(cx + 8.0, cy + 1.0, cx + 9.0, cy + 10.0);
    body.close();
    if let Some(path) = body.finish() {
        draw_card_stroke(pm, &path, color, 1.4);
    }
}

fn draw_lock_icon(pm: &mut PixmapMut, cx: f32, cy: f32, alpha: f32) {
    let color = theme_color(alpha, login_theme().colors.text_dim, 205.0);
    let body = rounded_rect_path(cx - 7.0, cy - 1.0, 14.0, 12.0, 2.0);
    draw_card_stroke(pm, &body, color, 1.4);
    let mut shackle = PathBuilder::new();
    shackle.move_to(cx - 4.5, cy - 1.0);
    shackle.line_to(cx - 4.5, cy - 5.0);
    shackle.quad_to(cx - 4.5, cy - 10.0, cx, cy - 10.0);
    shackle.quad_to(cx + 4.5, cy - 10.0, cx + 4.5, cy - 5.0);
    shackle.line_to(cx + 4.5, cy - 1.0);
    if let Some(path) = shackle.finish() {
        draw_card_stroke(pm, &path, color, 1.4);
    }
}

fn draw_yubikey_icon(pm: &mut PixmapMut, cx: f32, y: f32, alpha: f32, present: bool) {
    let x = cx - 30.0;
    let accent = if present {
        metro_success(alpha)
    } else {
        metro_border(alpha)
    };
    let body = mix_color(
        metro_background(1.0),
        if present {
            metro_success(1.0)
        } else {
            metro_surface(1.0)
        },
        if present { 0.18 } else { 0.08 },
        alpha_byte(alpha, 224.0),
    );

    let usb = rounded_rect_path(cx - 17.0, y - 13.0, 34.0, 18.0, 2.0);
    let mut usb_fill = Paint::default();
    usb_fill.set_color(mix_color(
        metro_surface(1.0),
        metro_text_dim(1.0),
        0.22,
        alpha_byte(alpha, 230.0),
    ));
    usb_fill.anti_alias = true;
    pm.fill_path(
        &usb,
        &usb_fill,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    draw_card_stroke(pm, &usb, metro_border(alpha), 1.0);

    for offset in [-9.0, 0.0, 9.0] {
        let contact = rounded_rect_path(cx + offset - 2.0, y - 8.0, 4.0, 8.0, 1.0);
        let mut contact_fill = Paint::default();
        contact_fill.set_color(color_with_alpha(
            metro_accent(1.0),
            alpha_byte(alpha, 160.0),
        ));
        contact_fill.anti_alias = true;
        pm.fill_path(
            &contact,
            &contact_fill,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    let card = rounded_rect_path(x, y, 60.0, 82.0, 6.0);
    let mut fill = Paint::default();
    fill.set_color(body);
    fill.anti_alias = true;
    pm.fill_path(&card, &fill, FillRule::Winding, Transform::identity(), None);
    draw_card_stroke(pm, &card, accent, if present { 2.0 } else { 1.0 });

    let top_cut = rounded_rect_path(cx - 13.0, y + 7.0, 26.0, 8.0, 2.0);
    let mut cut_fill = Paint::default();
    cut_fill.set_color(metro_background(alpha));
    cut_fill.anti_alias = true;
    pm.fill_path(
        &top_cut,
        &cut_fill,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    let touch = rounded_rect_path(cx - 16.0, y + 29.0, 32.0, 32.0, 16.0);
    let mut touch_fill = Paint::default();
    touch_fill.set_color(mix_color(
        metro_background(1.0),
        if present {
            metro_success(1.0)
        } else {
            metro_surface(1.0)
        },
        if present { 0.28 } else { 0.1 },
        alpha_byte(alpha, 238.0),
    ));
    touch_fill.anti_alias = true;
    pm.fill_path(
        &touch,
        &touch_fill,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    draw_card_stroke(pm, &touch, accent, 2.0);

    let dot = rounded_rect_path(cx - 3.0, y + 70.0, 6.0, 6.0, 3.0);
    let mut dot_fill = Paint::default();
    dot_fill.set_color(if present {
        metro_success(alpha)
    } else {
        metro_text_dim(alpha * 0.65)
    });
    dot_fill.anti_alias = true;
    pm.fill_path(
        &dot,
        &dot_fill,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_input_box(
    pm: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fill: Color,
    outline: Color,
    focused: bool,
    alpha: f32,
) {
    let path = rounded_rect_path(x, y, w, h, control_radius());
    let mut fill_paint = Paint::default();
    fill_paint.set_color(fill);
    fill_paint.anti_alias = true;
    pm.fill_path(
        &path,
        &fill_paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    draw_card_stroke(
        pm,
        &path,
        if focused { metro_accent(alpha) } else { outline },
        if focused { 1.5 } else { 1.0 },
    );
}

fn draw_caret(pm: &mut PixmapMut, x: f32, baseline_y: f32, font_size: f32, color: Color) {
    let top = baseline_y - 0.75 * font_size;
    let bottom = baseline_y + 0.1 * font_size;
    let mut pb = PathBuilder::new();
    pb.move_to(x, top);
    pb.line_to(x, bottom);
    let path = pb.finish().unwrap();
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    let stroke = Stroke {
        width: 1.5,
        ..Default::default()
    };
    pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
}

fn draw_pointer_cursor(pm: &mut PixmapMut, x: f32, y: f32, alpha: f32) {
    let alpha = (alpha * 255.0) as u8;
    let mut pb = PathBuilder::new();
    pb.move_to(x, y);
    pb.line_to(x, y + 22.0);
    pb.line_to(x + 5.5, y + 17.0);
    pb.line_to(x + 9.0, y + 25.0);
    pb.line_to(x + 13.0, y + 23.0);
    pb.line_to(x + 9.5, y + 15.5);
    pb.line_to(x + 17.0, y + 15.5);
    pb.close();
    let Some(path) = pb.finish() else {
        return;
    };

    let mut fill = Paint::default();
    // guard:allow: fallback pointer cursor — white fill + near-black outline must
    // stay theme-independent so the cursor is visible on ANY background.
    fill.set_color(Color::from_rgba8(235, 241, 252, alpha));
    fill.anti_alias = true;
    pm.fill_path(&path, &fill, FillRule::Winding, Transform::identity(), None);

    let mut stroke_paint = Paint::default();
    // guard:allow: see above — pointer cursor outline, theme-independent.
    stroke_paint.set_color(Color::from_rgba8(5, 8, 14, alpha));
    stroke_paint.anti_alias = true;
    let stroke = Stroke {
        width: 1.25,
        ..Default::default()
    };
    pm.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
}

fn card_rect(w: f32, h: f32) -> (f32, f32, f32, f32) {
    let cw = (w * 0.31).clamp(500.0, 640.0);
    let ch = (h * 0.46).clamp(460.0, 520.0);
    let left = w / 2.0 - cw / 2.0;
    let top = h / 2.0 - ch / 2.0;
    (left, top, cw, ch)
}

fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish().unwrap()
}
