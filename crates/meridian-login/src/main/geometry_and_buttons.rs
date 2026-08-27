fn click_target_at(
    w: f32,
    h: f32,
    x: f32,
    y: f32,
    shake_dx: f32,
    smartcard_mode: bool,
) -> Option<ClickTarget> {
    let (card_left_raw, card_top, cw, _) = card_rect(w, h);
    let card_left = card_left_raw + shake_dx;
    let inner_left = card_left + CARD_PAD;
    let inner_top = card_top + CARD_PAD;
    let inner_w = cw - 2.0 * CARD_PAD;
    let user_box_top = inner_top + USER_BOX_OFFSET_Y;
    let pwd_box_top = inner_top + PASSWORD_BOX_OFFSET_Y;

    if smartcard_mode {
        let pin = smartcard_pin_rect(w, h, shake_dx);
        if point_in_rect(x, y, pin.0, pin.1, pin.2, pin.3) {
            return Some(ClickTarget::Field(Field::Password));
        }
    } else if point_in_rect(x, y, inner_left, user_box_top, inner_w, INPUT_BOX_HEIGHT) {
        return Some(ClickTarget::Field(Field::Username));
    } else if point_in_rect(x, y, inner_left, pwd_box_top, inner_w, INPUT_BOX_HEIGHT) {
        return Some(ClickTarget::Field(Field::Password));
    }

    let login = login_button_rect(w, h, shake_dx);
    if point_in_rect(x, y, login.0, login.1, login.2, login.3) {
        return Some(ClickTarget::Submit);
    }

    let (restart, poweroff) = power_button_rects(w, h, shake_dx);
    if point_in_rect(x, y, restart.0, restart.1, restart.2, restart.3) {
        Some(ClickTarget::Reboot)
    } else if point_in_rect(x, y, poweroff.0, poweroff.1, poweroff.2, poweroff.3) {
        Some(ClickTarget::PowerOff)
    } else {
        None
    }
}

fn login_button_rect(w: f32, h: f32, shake_dx: f32) -> Rect {
    let (card_left, card_top, cw, _) = card_rect(w, h);
    (
        card_left + shake_dx + CARD_PAD,
        card_top + CARD_PAD + LOGIN_BUTTON_OFFSET_Y,
        cw - 2.0 * CARD_PAD,
        LOGIN_BUTTON_HEIGHT,
    )
}

fn smartcard_pin_rect(w: f32, h: f32, shake_dx: f32) -> Rect {
    let (card_left_raw, card_top, cw, _) = card_rect(w, h);
    let card_left = card_left_raw + shake_dx;
    let inner_top = card_top + CARD_PAD;
    (
        card_left + cw / 2.0 - SMARTCARD_PIN_WIDTH / 2.0,
        inner_top + SMARTCARD_PIN_BOX_OFFSET_Y,
        SMARTCARD_PIN_WIDTH,
        INPUT_BOX_HEIGHT,
    )
}

fn power_button_rects(w: f32, h: f32, shake_dx: f32) -> PowerButtonRects {
    let _ = shake_dx;
    let tokens = Greeter::DEFAULT;
    let button_width = tokens.power_button_width as f32;
    let button_height = tokens.power_button_height as f32;
    let gap = tokens.power_button_gap as f32;
    let edge_pad = tokens.power_button_edge_pad as f32;
    let total_w = button_width * 2.0 + gap;
    let x0 = w - edge_pad - total_w;
    let y = h - edge_pad - button_height;
    (
        (x0, y, button_width, button_height),
        (
            x0 + button_width + gap,
            y,
            button_width,
            button_height,
        ),
    )
}

fn power_action_at(w: f32, h: f32, x: f32, y: f32) -> Option<PowerAction> {
    let (restart, poweroff) = power_button_rects(w, h, 0.0);
    if point_in_rect(x, y, restart.0, restart.1, restart.2, restart.3) {
        Some(PowerAction::Reboot)
    } else if point_in_rect(x, y, poweroff.0, poweroff.1, poweroff.2, poweroff.3) {
        Some(PowerAction::PowerOff)
    } else {
        None
    }
}

fn color_with_alpha(c: Color, alpha: u8) -> Color {
    Color::from_rgba8(
        (c.red() * 255.0) as u8,
        (c.green() * 255.0) as u8,
        (c.blue() * 255.0) as u8,
        alpha,
    )
}

fn mix_color(a: Color, b: Color, t: f32, alpha: u8) -> Color {
    let lerp = |x: f32, y: f32| ((x + (y - x) * t) * 255.0).clamp(0.0, 255.0) as u8;
    Color::from_rgba8(
        lerp(a.red(), b.red()),
        lerp(a.green(), b.green()),
        lerp(a.blue(), b.blue()),
        alpha,
    )
}

fn alpha_byte(alpha: f32, max: f32) -> u8 {
    (alpha.clamp(0.0, 1.0) * max).clamp(0.0, 255.0) as u8
}

fn theme_color(alpha: f32, color: meridian_config::Color, max_alpha: f32) -> Color {
    Color::from_rgba8(color.r, color.g, color.b, alpha_byte(alpha, max_alpha))
}

fn modal_fill_alpha(scale: f32) -> f32 {
    let treatment = login_theme()
        .decorations
        .surface_treatment(ThemeSurface::Modal);
    (treatment.fill_alpha as f32 * scale).clamp(0.0, 255.0)
}

fn modal_frame_alpha(scale: f32) -> f32 {
    let treatment = login_theme()
        .decorations
        .surface_treatment(ThemeSurface::Modal);
    (treatment.frame_alpha as f32 * scale).clamp(0.0, 255.0)
}

fn metro_surface(alpha: f32) -> Color {
    theme_color(alpha, login_theme().colors.surface, modal_fill_alpha(0.75))
}

fn metro_background(alpha: f32) -> Color {
    theme_color(
        alpha,
        login_theme().colors.background,
        modal_fill_alpha(0.55),
    )
}

fn metro_accent(alpha: f32) -> Color {
    theme_color(alpha, login_theme().colors.accent, 255.0)
}

fn metro_text(alpha: f32) -> Color {
    theme_color(alpha, login_theme().colors.text, 255.0)
}

fn metro_text_dim(alpha: f32) -> Color {
    theme_color(alpha, login_theme().colors.text_dim, 255.0)
}

fn metro_border(alpha: f32) -> Color {
    theme_color(alpha, login_theme().colors.border, modal_frame_alpha(1.0))
}

fn metro_error(alpha: f32) -> Color {
    theme_color(alpha, login_theme().colors.error, 255.0)
}

fn metro_success(alpha: f32) -> Color {
    theme_color(alpha, login_theme().colors.success, 255.0)
}

fn draw_soft_card_shadow(pm: &mut PixmapMut, left: f32, top: f32, w: f32, h: f32, alpha: f32) {
    let strength = if light_appearance() {
        0.035
    } else {
        CARD_SHADOW_ALPHA
    };
    for i in 0..14 {
        let t = i as f32 / 13.0;
        let spread = 3.0 + CARD_SHADOW_BLUR * (1.85 * t);
        let dy = CARD_SHADOW_OFFSET_Y + 2.0 + CARD_SHADOW_BLUR * (0.34 * t);
        let opacity = (1.0 - t).powf(1.65) * strength * 255.0 * alpha;
        let path = rounded_rect_path(
            left - spread / 2.0,
            top + dy - spread * 0.18,
            w + spread,
            h + spread * 0.36,
            card_radius() + spread * 0.45,
        );
        let mut paint = Paint::default();
        // guard:allow: card drop shadow — black is depth/materiality, not a theme colour.
        paint.set_color(Color::from_rgba8(0, 0, 0, opacity.clamp(0.0, 255.0) as u8));
        paint.anti_alias = true;
        pm.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn draw_card_stroke(pm: &mut PixmapMut, path: &tiny_skia::Path, color: Color, width: f32) {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    let stroke = Stroke {
        width,
        ..Default::default()
    };
    pm.stroke_path(path, &paint, &stroke, Transform::identity(), None);
}

fn draw_login_button(
    pm: &mut PixmapMut,
    painter: &CompassPainter,
    rect: Rect,
    label: &str,
    accent: Color,
    alpha: f32,
    emphasized: bool,
) {
    let path = rounded_rect_path(rect.0, rect.1, rect.2, rect.3, control_radius());
    let fill = theme_color(alpha, login_theme().colors.background, 176.0);
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
        if emphasized {
            color_with_alpha(accent, alpha_byte(alpha, 240.0))
        } else {
            metro_border(alpha)
        },
        if emphasized {
            Greeter::DEFAULT.power_button_emphasis_border_width as f32
        } else {
            Greeter::DEFAULT.power_button_border_width as f32
        },
    );

    painter.render_text_centered(
        pm,
        TextStyle::SansRegular(Greeter::DEFAULT.power_button_label_size as f32),
        label,
        rect.0 + rect.2 / 2.0,
        rect.1 + rect.3 / 2.0,
        metro_text(alpha),
    );
}

fn draw_power_buttons(
    pm: &mut PixmapMut,
    w: f32,
    h: f32,
    painter: &CompassPainter,
    alpha: f32,
    shake_dx: f32,
    pending: Option<PowerAction>,
    focused: Option<PowerAction>,
    hovered: Option<PowerAction>,
) {
    let (restart, poweroff) = power_button_rects(w, h, shake_dx);
    draw_login_button(
        pm,
        painter,
        restart,
        if pending == Some(PowerAction::Reboot) {
            "Bestätigen"
        } else {
            "Neustart"
        },
        metro_accent(1.0),
        alpha,
        pending == Some(PowerAction::Reboot)
            || focused == Some(PowerAction::Reboot)
            || hovered == Some(PowerAction::Reboot),
    );
    draw_login_button(
        pm,
        painter,
        poweroff,
        if pending == Some(PowerAction::PowerOff) {
            "Bestätigen"
        } else {
            "Ausschalten"
        },
        metro_error(1.0),
        alpha,
        pending == Some(PowerAction::PowerOff)
            || focused == Some(PowerAction::PowerOff)
            || hovered == Some(PowerAction::PowerOff),
    );
}

fn point_in_rect(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}
