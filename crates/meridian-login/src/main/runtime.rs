fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let appearance = read_appearance();
    let theme = load_login_theme(appearance);
    LIGHT_APPEARANCE.store(appearance.is_light(), std::sync::atomic::Ordering::Relaxed);
    let _ = LOGIN_THEME.set(theme);
    info!("meridian-login starting (Phase 7)");
    let mut greeter_assets = None;

    loop {
        if !run_login_cycle(&mut greeter_assets)? {
            return Ok(());
        }
        info!("desktop session ended; reinitializing greeter");
    }
}

fn run_login_cycle(
    greeter_assets: &mut Option<GreeterAssets>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let reacquire_started_at = Instant::now();
    let (drm_card, mut card) = open_display_card()?;
    info!(path = %drm_card, "preparing login DRM card (auto-selected display GPU)");

    let res = card.resource_handles()?;
    let conn_info = res
        .connectors()
        .iter()
        .filter_map(|&h| match card.get_connector(h, false) {
            Ok(connector) => Some(connector),
            Err(err) => {
                warn!(connector = ?h, error = %err, "failed to inspect DRM connector");
                None
            }
        })
        .find(|c| c.state() == connector::State::Connected)
        .ok_or("no connected connector")?;
    let mode = select_boot_mode(conn_info.modes()).ok_or("connector has no usable mode")?;
    let (w, h) = mode.size();
    let (w, h) = (w as u32, h as u32);
    info!(width = w, height = h, refresh = mode.vrefresh(), "drm mode");

    let crtc = if let Some(enc_h) = conn_info.current_encoder() {
        card.get_encoder(enc_h)?.crtc()
    } else {
        None
    }
    .or_else(|| res.crtcs().first().copied())
    .ok_or("no CRTC available")?;

    let mut db = card.create_dumb_buffer((w, h), DrmFourcc::Xrgb8888, 32)?;
    let mut fb = card.add_framebuffer(&db, 24, 32)?;

    let assets_reused = greeter_assets
        .as_ref()
        .is_some_and(|assets| assets.matches_output(w, h));
    if !assets_reused {
        *greeter_assets = Some(GreeterAssets::new(w, h)?);
    }
    let assets = greeter_assets
        .as_ref()
        .expect("greeter assets were initialized");
    info!(reused = assets_reused, "greeter static assets ready");

    // Pre-fill the dumb buffer with the settle frame BEFORE set_crtc so
    // the kernel never scans out a zeroed (black) buffer. Without this,
    // the modeset would briefly show black between "bootsplash fb
    // unmapped" and "first dirty_framebuffer push" → visible flash.
    {
        let mut mapping = card.map_dumb_buffer(&mut db)?;
        let buf = mapping.as_mut();
        assets.backdrop.copy_rgba_to(buf)?;
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
    }

    // Prepare the complete greeter frame while the bootsplash is still being
    // scanned out. Only then request handover, acquire master, and commit. On
    // OpenBSD the splash closes its released primary-node fd before acking, so
    // this keeps the unavoidable no-owner interval as short as possible.
    let handover_started_at = Instant::now();
    let bootsplash_released = match bootsplash_handover() {
        Ok(()) => {
            info!("bootsplash handover acked (master released)");
            true
        }
        Err(e) => {
            warn!(error = %e, "bootsplash handover failed (not running?); proceeding");
            false
        }
    };
    if bootsplash_released {
        #[cfg(target_os = "openbsd")]
        {
            // OpenBSD does not grant effective KMS permission to a primary-node
            // fd that was opened while the previous master was still alive.
            // Reopen only after the splash has closed its fd, then rebuild the
            // small kernel-side buffer from the already prepared CPU backdrop.
            drop(card);
            let (reopened_path, reopened_card) = open_display_card()?;
            card = reopened_card;
            db = card.create_dumb_buffer((w, h), DrmFourcc::Xrgb8888, 32)?;
            fb = card.add_framebuffer(&db, 24, 32)?;
            {
                let mut mapping = card.map_dumb_buffer(&mut db)?;
                let buf = mapping.as_mut();
                assets.backdrop.copy_rgba_to(buf)?;
                for px in buf.chunks_exact_mut(4) {
                    px.swap(0, 2);
                }
            }
            info!(path = %reopened_path, "reopened login DRM card after OpenBSD handover");
        }
        card.acquire_master_lock()?;
        info!("acquired drm master after bootsplash handover");
    }

    card.set_crtc(crtc, Some(fb), (0, 0), &[conn_info.handle()], Some(mode))?;
    let clip = ClipRect::new(0, 0, w as u16, h as u16);
    let _ = card.dirty_framebuffer(fb, &[clip]);
    info!(
        reacquire_ms = reacquire_started_at.elapsed().as_millis() as u64,
        handover_to_frame_ms = handover_started_at.elapsed().as_millis() as u64,
        assets_reused,
        "settle frame committed"
    );

    match bootsplash_exit() {
        Ok(()) => info!("bootsplash exit signalled"),
        Err(e) => warn!(error = %e, "bootsplash exit signal failed"),
    }

    let mut ui_state = LoginUiState::default();
    let mut keyboards = open_keyboards()?;
    let mut pointers = open_pointers()?;
    let mut keyboard = Keyboard::new().map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    ui_state.keyboard_status = keyboard.status();
    ui_state.update_security_key_state();
    let mut pointer = PointerState::new(w, h);

    let exit = run_animation(
        &card,
        &mut db,
        fb,
        &assets.painter,
        &assets.backdrop,
        w,
        h,
        mode.vrefresh().max(60),
        &mut ui_state,
        &mut keyboards,
        &mut keyboard,
        &mut pointers,
        &mut pointer,
    )?;

    // Release the keyboards BEFORE we spawn the compositor and enter the
    // handover wait. input.rs grabs every keyboard device with
    // EVIOCGRAB so the password never leaks to the kernel TTY; if we
    // keep the fds open while the compositor runs, libinput in the
    // compositor sees the devices but receives zero key events (the
    // grab is per-fd and only releases on close). Dropping here lets
    // the user type into apps immediately after auth.
    drop(keyboards);
    drop(keyboard);
    drop(pointers);
    info!("released input devices");

    if matches!(exit, ControlFlow::PowerOff | ControlFlow::Reboot) {
        let action = match exit {
            ControlFlow::PowerOff => PowerAction::PowerOff,
            ControlFlow::Reboot => PowerAction::Reboot,
            _ => unreachable!(),
        };
        match card.release_master_lock() {
            Ok(()) => info!("released drm master before power action"),
            Err(e) => warn!(error = %e, "release_master before power action failed"),
        }
        drop(card);
        let result = run_power_action(action);
        return Ok(restore_greeter_after_power_result(action, &result));
    }

    // On successful auth, spawn the compositor as the authenticated user
    // BEFORE releasing master so the new process is already running by the
    // time we let go of the display. Phase 8 will add an IPC handshake to
    // hold the buffer until the compositor's first frame is committed.
    //
    // Phase 7b: the PAM session (held by ui_state.auth_driver) must stay
    // alive for as long as the compositor runs — pam_systemd's logind
    // session is what backs libseat. So after spawning we wait() on the
    // compositor, and only then drop the AuthDriver to close the session.
    let username = ui_state.username.clone();
    let auth_driver = ui_state.auth_driver.take();
    let pam_env = std::mem::take(&mut ui_state.pam_env);

    let compositor_child = match exit {
        ControlFlow::Submit => {
            info!(
                user = %username,
                pam_env_count = pam_env.len(),
                "auth ok — launching compositor"
            );
            match session::launch_compositor_for(&username, &pam_env) {
                Ok(child) => {
                    info!(pid = child.id(), "compositor spawned");
                    Some(child)
                }
                Err(e) => {
                    warn!(error = %e, "compositor spawn failed");
                    None
                }
            }
        }
        ControlFlow::Cancel => {
            info!("login cancelled");
            None
        }
        ControlFlow::PowerOff | ControlFlow::Reboot => None,
        ControlFlow::Continue => {
            info!("inactivity timeout reached");
            None
        }
    };

    // Phase 8: keep the DRM master + framebuffer alive until the compositor
    // signals it is ready to take the screen (via `handover` on the login
    // IPC socket). When that signal arrives we release master + close the
    // fd, which lets the compositor's libseat acquire cleanly. If the
    // compositor crashes early or never signals, the HANDOVER_DEADLINE
    // fallback releases anyway so we don't strand the user on a frozen
    // login frame.
    let (ipc_tx, ipc_rx) = mpsc::channel::<IpcEvent>();
    let ipc_socket_path = match compositor_child.as_ref() {
        Some(_) => match nix::unistd::User::from_name(&username) {
            Ok(Some(user)) => {
                match spawn_login_ipc_server(ipc_tx, user.uid.as_raw(), user.gid.as_raw()) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        warn!(error = %e, "login ipc server bind failed; releasing drm immediately");
                        None
                    }
                }
            }
            Ok(None) => {
                warn!(user = %username, "user lookup returned None; skipping ipc server");
                None
            }
            Err(e) => {
                warn!(error = %e, user = %username, "user lookup failed; skipping ipc server");
                None
            }
        },
        // No compositor to hand over to — skip the IPC server, fall through
        // to immediate release below.
        None => None,
    };

    let mut card_opt: Option<Card> = Some(card);
    let mut master_released = false;
    let mut first_frame_seen = false;
    let spawn_wait_started_at = Instant::now();
    let mut handover_received_at: Option<Instant> = None;

    // Two-stage release mirroring the bootsplash → login pattern:
    //   1. on `handover`: drop DRM master but keep the fd open so the
    //      scanout buffer stays referenced by the kernel — the visible
    //      pixels do not change yet.
    //   2. on `exit`: close the fd. By now the compositor's first frame
    //      is on screen and owns the scanout, so closing our fb is safe.
    // Without (1), the compositor's first commit fights us for master.
    // Without (2)-being-deferred-to-exit, the kernel may drop our fb
    // before the compositor's first commit lands → black flash.
    let restart_greeter = should_restart_greeter(exit, compositor_child.is_some());
    if let Some(mut child) = compositor_child {
        info!(pid = child.id(), "waiting for compositor handover + exit");
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    info!(pid = child.id(), status = ?status, "compositor exited");
                    if let Some(card) = card_opt.take() {
                        if !master_released {
                            warn!("compositor exited without handover; releasing drm now");
                            let _ = card.release_master_lock();
                        }
                        drop(card);
                    }
                    break;
                }
                Ok(None) => {}
                Err(e) => {
                    warn!(error = %e, "try_wait on compositor failed");
                    break;
                }
            }

            match ipc_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(IpcEvent::Handover(ack)) => {
                    if !master_released {
                        if let Some(card) = card_opt.as_ref() {
                            match card.release_master_lock() {
                                Ok(()) => {
                                    info!(
                                        spawn_to_handover_ms =
                                            spawn_wait_started_at.elapsed().as_millis() as u64,
                                        "ipc handover: released drm master (fd kept alive)"
                                    );
                                }
                                Err(e) => warn!(error = %e, "release_master failed"),
                            }
                        }
                        master_released = true;
                    }
                    // OpenBSD's DRM master handoff is stricter than Linux's:
                    // keeping the old primary-node fd alive after DROPMASTER
                    // can leave the new atomic client without effective KMS
                    // permission. Close it before acknowledging handover.
                    #[cfg(target_os = "openbsd")]
                    if let Some(card) = card_opt.take() {
                        drop(card);
                        info!("OpenBSD handover: closed released login DRM fd");
                    }
                    handover_received_at.get_or_insert_with(Instant::now);
                    let _ = ack.send(());
                }
                Ok(IpcEvent::Exit(ack)) => {
                    if !first_frame_seen {
                        info!(
                            spawn_to_first_frame_ms =
                                spawn_wait_started_at.elapsed().as_millis() as u64,
                            handover_to_first_frame_ms =
                                handover_received_at.map(|at| at.elapsed().as_millis() as u64),
                            "ipc exit: compositor first frame on screen; closing card0 fd"
                        );
                        first_frame_seen = true;
                    }
                    if let Some(card) = card_opt.take() {
                        drop(card);
                    }
                    let _ = ack.send(());
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if spawn_wait_started_at.elapsed() >= HANDOVER_DEADLINE {
                        if !master_released {
                            if let Some(card) = card_opt.as_ref() {
                                warn!(
                                    deadline_s = HANDOVER_DEADLINE.as_secs(),
                                    "handover deadline missed; releasing drm master"
                                );
                                let _ = card.release_master_lock();
                            }
                            master_released = true;
                        }
                        if !first_frame_seen {
                            if let Some(card) = card_opt.take() {
                                warn!("handover deadline missed; closing card0 fd");
                                drop(card);
                            }
                            first_frame_seen = true;
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    warn!("login ipc thread vanished; falling back to immediate release");
                    if let Some(card) = card_opt.take() {
                        let _ = card.release_master_lock();
                        drop(card);
                    }
                    master_released = true;
                    first_frame_seen = true;
                }
            }
        }
    } else if exit == ControlFlow::Submit {
        // Auth ok but spawn failed — don't leave the session dangling.
        warn!("auth ok but compositor not spawned; tearing session down immediately");
        if let Some(card) = card_opt.take() {
            let _ = card.release_master_lock();
            drop(card);
        }
    } else {
        // Cancel / inactivity — release master and exit cleanly.
        if let Some(card) = card_opt.take() {
            let _ = card.release_master_lock();
            drop(card);
        }
    }

    if let Some((path, identity)) = ipc_socket_path {
        match cleanup_socket_path(&path, identity) {
            Ok(true) | Ok(false) => {}
            Err(err) => warn!(path = %path.display(), error = %err, "login ipc cleanup failed"),
        }
    }

    if let Some(driver) = auth_driver {
        info!(user = %username, "closing PAM session");
        driver.close();
    }

    if restart_greeter {
        info!("returning to greeter after desktop session");
    } else {
        info!("meridian-login exiting");
    }
    Ok(restart_greeter)
}

fn should_restart_greeter(exit: ControlFlow, compositor_started: bool) -> bool {
    compositor_started || exit == ControlFlow::Submit
}
