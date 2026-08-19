struct AnimFrame {
    card_alpha: f32,
    ui_alpha: f32,
}

fn compute_anim_frame(t_anim_secs: f32) -> AnimFrame {
    let t_ms = (t_anim_secs * 1000.0) as u64;

    let card_alpha = ramp_f32(t_ms, CARD_FADE_START_MS, CARD_FADE_END_MS, 0.0, 1.0);
    let ui_alpha = ramp_f32(t_ms, UI_FADE_START_MS, UI_FADE_END_MS, 0.0, 1.0);

    AnimFrame {
        card_alpha,
        ui_alpha,
    }
}

fn anim_frame_is_steady(af: &AnimFrame) -> bool {
    (af.card_alpha - 1.0).abs() < f32::EPSILON && (af.ui_alpha - 1.0).abs() < f32::EPSILON
}

fn ramp_f32(t: u64, start: u64, end: u64, from: f32, to: f32) -> f32 {
    if t <= start {
        from
    } else if t >= end {
        to
    } else {
        let p = (t - start) as f32 / (end - start) as f32;
        from + (to - from) * p
    }
}

#[allow(clippy::too_many_arguments)]
fn run_animation(
    card: &Card,
    db: &mut drm::control::dumbbuffer::DumbBuffer,
    fb: drm::control::framebuffer::Handle,
    painter: &CompassPainter,
    backdrop: &LoginBackdrop,
    w: u32,
    h: u32,
    refresh_hz: u32,
    ui_state: &mut LoginUiState,
    keyboards: &mut [evdev::Device],
    keyboard: &mut Keyboard,
    pointers: &mut [input::PointerDevice],
    pointer: &mut PointerState,
) -> Result<ControlFlow, Box<dyn std::error::Error>> {
    let anim_start = Instant::now();
    let frame_dur = Duration::from_micros(1_000_000 / refresh_hz as u64);
    let mut frame_idx: u64 = 0;
    let mut exit = ControlFlow::Continue;
    let mut steady_frame_drawn = false;
    let mut next_security_key_probe = Instant::now();
    // Render every frame into cached RAM, not straight into the DRM dumb buffer.
    // tiny-skia alpha-blends by reading the destination pixels, and reading the
    // write-combining scanout memory is brutally slow — that is what made the
    // login screen crawl. We do all the CPU work here and push the finished
    // frame to the scanout buffer with a single write-only copy below.
    let mut frame_buf = vec![0u8; (w as usize) * (h as usize) * 4];

    while exit == ControlFlow::Continue {
        let t = anim_start.elapsed();
        let t_secs = t.as_secs_f32();
        let af = compute_anim_frame(t_secs);
        let animating = !anim_frame_is_steady(&af);
        let mut redraw = animating || !steady_frame_drawn;

        // Read keyboard once the UI is fully faded in. Polling earlier would
        // queue keystrokes the user typed against the still-animating splash.
        if af.ui_alpha >= 1.0 {
            let was_failed = matches!(ui_state.phase, InputPhase::Failed(_));
            // Failed-state shake decays back to Editing in tick()
            ui_state.tick();
            if was_failed != matches!(ui_state.phase, InputPhase::Failed(_)) {
                redraw = true;
            }
            if ui_state.clear_expired_power_confirmation() {
                redraw = true;
            }
            if Instant::now() >= next_security_key_probe {
                if ui_state.update_security_key_state() {
                    redraw = true;
                }
                next_security_key_probe = Instant::now() + SECURITY_KEY_POLL_INTERVAL;
            }

            // Ignore key events during Authenticating or Failed — they would
            // queue up against the next Editing window otherwise.
            let accept_input = matches!(ui_state.phase, InputPhase::Editing);
            let actions = poll_keyboards(keyboards, keyboard);
            let keyboard_status = keyboard.status();
            if ui_state.keyboard_status != keyboard_status {
                ui_state.keyboard_status = keyboard_status;
                redraw = true;
            }
            for action in actions {
                if !accept_input {
                    continue;
                }
                redraw = true;
                match ui_state.apply(action) {
                    ControlFlow::Continue => {}
                    ControlFlow::Cancel => {
                        exit = ControlFlow::Cancel;
                        break;
                    }
                    ControlFlow::Submit => {
                        // Hand PAM off to a background thread so the render
                        // loop keeps drawing and the "Anmelden …" hint
                        // appears immediately.
                        ui_state.start_auth();
                        break;
                    }
                    ControlFlow::PowerOff | ControlFlow::Reboot => {}
                }
            }

            // Check if the auth thread reported back this frame.
            if let Some(result) = ui_state.poll_auth() {
                redraw = true;
                match result {
                    AuthResult::Ok(env) => {
                        ui_state.pam_env = env;
                        exit = ControlFlow::Submit;
                    }
                    AuthResult::Failed => ui_state.reject(),
                    AuthResult::Error(e) => {
                        warn!(error = %e, "PAM error — treating as failure");
                        ui_state.reject();
                    }
                }
            }
        }

        let shake_dx = ui_state.shake_offset();
        if matches!(ui_state.phase, InputPhase::Failed(_)) {
            redraw = true;
        }
        if af.ui_alpha >= 1.0 {
            let accept_pointer = matches!(ui_state.phase, InputPhase::Editing);
            let pointer_before = (pointer.x, pointer.y);
            let pointer_actions = poll_pointers(pointers, pointer);
            if (pointer.x, pointer.y) != pointer_before {
                redraw = true;
            }
            for action in pointer_actions {
                if !accept_pointer {
                    continue;
                }
                match action {
                    PointerAction::LeftPress { x, y } => {
                        match click_target_at(
                            w as f32,
                            h as f32,
                            x,
                            y,
                            shake_dx,
                            ui_state.smartcard_login_ready(),
                        ) {
                            Some(ClickTarget::Field(field)) => {
                                ui_state.focus = field;
                                ui_state.pending_power = None;
                                redraw = true;
                            }
                            Some(ClickTarget::Submit) => {
                                ui_state.pending_power = None;
                                ui_state.start_auth();
                                redraw = true;
                            }
                            Some(ClickTarget::PowerOff) => {
                                redraw = true;
                                if let Some(flow) =
                                    ui_state.confirm_power_action(PowerAction::PowerOff)
                                {
                                    exit = flow;
                                    break;
                                }
                            }
                            Some(ClickTarget::Reboot) => {
                                redraw = true;
                                if let Some(flow) =
                                    ui_state.confirm_power_action(PowerAction::Reboot)
                                {
                                    exit = flow;
                                    break;
                                }
                            }
                            None => {}
                        }
                    }
                }
            }
        } else {
            let _ = poll_pointers(pointers, pointer);
        }

        let caret_on = matches!(ui_state.phase, InputPhase::Editing) && af.ui_alpha >= 1.0;

        if exit != ControlFlow::Continue {
            break;
        }

        if !redraw {
            frame_idx += 1;
            let next = anim_start + frame_dur * frame_idx as u32;
            if let Some(wait) = next.checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
            continue;
        }

        {
            backdrop.copy_rgba_to(&mut frame_buf)?;
            let mut pm = PixmapMut::from_bytes(&mut frame_buf, w, h).ok_or("pixmap bind failed")?;

            if af.card_alpha > 0.0 {
                draw_card(
                    &mut pm,
                    w as f32,
                    h as f32,
                    af.card_alpha,
                    shake_dx,
                    ui_state.security_key_present,
                );
            }

            if af.ui_alpha > 0.0 {
                draw_login_ui(
                    &mut pm,
                    w as f32,
                    h as f32,
                    painter,
                    ui_state,
                    af.ui_alpha,
                    caret_on,
                    shake_dx,
                );
                draw_pointer_cursor(&mut pm, pointer.x, pointer.y, af.ui_alpha);
            }
        }

        // tiny-skia is RGBA; DRM XRGB8888 on LE wants BGRX. Swap in cached RAM…
        for px in frame_buf.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        // …then a single write-only copy into the scanout buffer.
        {
            let mut mapping = card.map_dumb_buffer(db)?;
            mapping.as_mut().copy_from_slice(&frame_buf);
        }

        let clip = ClipRect::new(0, 0, w as u16, h as u16);
        let _ = card.dirty_framebuffer(fb, &[clip]);
        if !animating {
            steady_frame_drawn = true;
        }

        frame_idx += 1;
        let next = anim_start + frame_dur * frame_idx as u32;
        if let Some(wait) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
    }
    Ok(exit)
}
