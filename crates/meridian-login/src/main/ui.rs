fn draw_card(
    pm: &mut PixmapMut,
    w: f32,
    h: f32,
    alpha: f32,
    shake_dx: f32,
    security_key_present: bool,
) {
    let (left, top, cw, ch) = card_rect(w, h);
    let left = left + shake_dx;
    let path = rounded_rect_path(left, top, cw, ch, card_radius());
    draw_soft_card_shadow(pm, left, top, cw, ch, alpha);
    let mut fill = Paint::default();
    fill.set_color(theme_color(alpha, login_theme().colors.surface, 222.0));
    fill.anti_alias = true;
    pm.fill_path(&path, &fill, FillRule::Winding, Transform::identity(), None);

    let border = if security_key_present {
        color_with_alpha(metro_success(1.0), alpha_byte(alpha, 132.0))
    } else {
        theme_color(alpha, login_theme().colors.border, 72.0)
    };
    draw_card_stroke(pm, &path, border, 1.0);

    let inner = rounded_rect_path(
        left + 1.5,
        top + 1.5,
        cw - 3.0,
        ch - 3.0,
        card_radius() - 1.5,
    );
    draw_card_stroke(
        pm,
        &inner,
        theme_color(alpha, login_theme().colors.text, 18.0),
        1.0,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_login_ui(
    pm: &mut PixmapMut,
    w: f32,
    h: f32,
    painter: &CompassPainter,
    ui: &LoginUiState,
    alpha: f32,
    caret_on: bool,
    shake_dx: f32,
    hovered_power: Option<PowerAction>,
) {
    let (card_left_raw, card_top, cw, ch) = card_rect(w, h);
    let card_left = card_left_raw + shake_dx;
    let inner_top = card_top + CARD_PAD;
    let cx = card_left + cw / 2.0;

    let text_color = metro_text(alpha);
    let label_color = metro_text_dim(alpha);
    let hint_color = metro_text_dim(alpha * 0.92);
    let caret_color = metro_accent(alpha);
    let box_fill = theme_color(alpha, login_theme().colors.background, 176.0);
    let box_outline = theme_color(alpha, login_theme().colors.border, 62.0);
    let smartcard_ready = ui.smartcard_login_ready();

    draw_brand_mark(pm, cx, inner_top + BRAND_MARK_OFFSET_Y, alpha);
    painter.render_text_centered(
        pm,
        TextStyle::SansRegular(28.0),
        "M E R I D I A N",
        cx,
        inner_top + TITLE_OFFSET_Y,
        theme_color(alpha, login_theme().colors.text, 238.0),
    );
    painter.render_text_centered(
        pm,
        TextStyle::SansRegular(13.0),
        "WILLKOMMEN",
        cx,
        inner_top + SUBTITLE_OFFSET_Y,
        theme_color(alpha, login_theme().colors.accent, 220.0),
    );

    if smartcard_ready {
        draw_yubikey_icon(
            pm,
            cx,
            inner_top + USER_BOX_OFFSET_Y - 18.0,
            alpha,
            ui.security_key_present,
        );

        let pin_rect = smartcard_pin_rect(w, h, shake_dx);
        draw_input_box(
            pm,
            pin_rect.0,
            pin_rect.1,
            pin_rect.2,
            pin_rect.3,
            box_fill,
            box_outline,
            true,
            alpha,
        );
        draw_lock_icon(pm, pin_rect.0 + 26.0, pin_rect.1 + pin_rect.3 / 2.0, alpha);
        let pwd_text_x = pin_rect.0 + INPUT_TEXT_PAD_X;
        let pwd_baseline = pin_rect.1 + INPUT_BOX_HEIGHT - INPUT_BASELINE_PAD_BOTTOM;
        let dots = "•".repeat(ui.password.chars().count());
        let display = if dots.is_empty() {
            "Smartcard-PIN"
        } else {
            &dots
        };
        let after_pwd = painter.render_text_left(
            pm,
            TextStyle::SansRegular(17.0),
            display,
            pwd_text_x,
            pwd_baseline,
            if dots.is_empty() {
                label_color
            } else {
                text_color
            },
        );
        if caret_on {
            draw_caret(
                pm,
                caret_x(dots.is_empty(), pwd_text_x, after_pwd),
                pwd_baseline,
                17.0,
                caret_color,
            );
        }
    } else {
        let inner_left = card_left + CARD_PAD;
        let inner_w = cw - 2.0 * CARD_PAD;
        let user_box_top = inner_top + USER_BOX_OFFSET_Y;
        let pwd_box_top = inner_top + PASSWORD_BOX_OFFSET_Y;

        draw_input_box(
            pm,
            inner_left,
            user_box_top,
            inner_w,
            INPUT_BOX_HEIGHT,
            box_fill,
            box_outline,
            ui.focus == Field::Username,
            alpha,
        );
        draw_user_icon(
            pm,
            inner_left + 26.0,
            user_box_top + INPUT_BOX_HEIGHT / 2.0,
            alpha,
        );
        let user_text_x = inner_left + INPUT_TEXT_PAD_X;
        let user_baseline = user_box_top + INPUT_BOX_HEIGHT - INPUT_BASELINE_PAD_BOTTOM;
        let user_display = if ui.username.is_empty() {
            "Benutzername"
        } else {
            &ui.username
        };
        let after_user = painter.render_text_left(
            pm,
            TextStyle::SansRegular(17.0),
            user_display,
            user_text_x,
            user_baseline,
            if ui.username.is_empty() {
                label_color
            } else {
                text_color
            },
        );
        if caret_on && ui.focus == Field::Username {
            draw_caret(
                pm,
                caret_x(ui.username.is_empty(), user_text_x, after_user),
                user_baseline,
                17.0,
                caret_color,
            );
        }

        draw_input_box(
            pm,
            inner_left,
            pwd_box_top,
            inner_w,
            INPUT_BOX_HEIGHT,
            box_fill,
            box_outline,
            ui.focus == Field::Password,
            alpha,
        );
        draw_lock_icon(
            pm,
            inner_left + 26.0,
            pwd_box_top + INPUT_BOX_HEIGHT / 2.0,
            alpha,
        );
        let pwd_text_x = inner_left + INPUT_TEXT_PAD_X;
        let pwd_baseline = pwd_box_top + INPUT_BOX_HEIGHT - INPUT_BASELINE_PAD_BOTTOM;
        let dots = "•".repeat(ui.password.chars().count());
        let pwd_display = if dots.is_empty() { "Passwort" } else { &dots };
        let after_pwd = painter.render_text_left(
            pm,
            TextStyle::SansRegular(17.0),
            pwd_display,
            pwd_text_x,
            pwd_baseline,
            if dots.is_empty() {
                label_color
            } else {
                text_color
            },
        );
        if caret_on && ui.focus == Field::Password {
            draw_caret(
                pm,
                caret_x(dots.is_empty(), pwd_text_x, after_pwd),
                pwd_baseline,
                17.0,
                caret_color,
            );
        }
    }

    let submit_rect = login_button_rect(w, h, shake_dx);
    let submit_label = match ui.phase {
        InputPhase::Authenticating => "Anmelden …",
        InputPhase::Failed(_) => "Erneut versuchen",
        InputPhase::Editing => "Anmelden",
    };
    draw_submit_button(pm, painter, submit_rect, submit_label, alpha);

    let hint_text = ui.hint();
    let hint_color_phase = match ui.phase {
        InputPhase::Failed(_) => metro_error(alpha),
        InputPhase::Editing | InputPhase::Authenticating => hint_color,
    };
    painter.render_text_centered(
        pm,
        TextStyle::SansRegular(12.0),
        &hint_text,
        cx,
        card_top + ch + HINT_OFFSET_Y,
        hint_color_phase,
    );
    draw_power_buttons(
        pm,
        w,
        h,
        painter,
        alpha,
        shake_dx,
        ui.pending_power_action(),
        ui.power_focus,
        hovered_power,
    );
}
