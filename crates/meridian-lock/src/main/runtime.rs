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
        style: LockStyle::load(),
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
