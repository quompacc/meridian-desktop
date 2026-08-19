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
                    state.compositor = Some(registry.bind(name, version.min(5), qh, ()));
                }
                "wl_shm" => {
                    state.shm = Some(registry.bind(name, 1, qh, ()));
                }
                "wl_seat" => {
                    state.seat = Some(registry.bind(name, version.min(7), qh, ()));
                }
                "zwlr_layer_shell_v1" => {
                    state.layer_shell = Some(registry.bind(name, version.min(4), qh, ()));
                }
                _ => {}
            }
        }
    }
}

// ── Seat / Keyboard ─────────────────────────────────────────────────────────

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
        qh: &QueueHandle<Self>,
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
                let len = size_usize.saturating_sub(1);
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
            } if state.handle_key(key) => {
                state.draw(qh);
            }
            _ => {}
        }
    }
}

// ── Layer surface ───────────────────────────────────────────────────────────

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for AppState {
    fn event(
        state: &mut Self,
        ls: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                ls.ack_configure(serial);
                if let Some(p) = state.popup.as_mut() {
                    if width > 0 {
                        p.width = width;
                    }
                    if height > 0 {
                        p.height = height;
                    }
                    p.configured = true;
                }
                state.draw(qh);
            }
            zwlr_layer_surface_v1::Event::Closed => {
                debug!("polkit: layer surface closed by compositor");
                state.finish(Outcome::Cancelled);
            }
            _ => {}
        }
    }
}

// delegate_noop! panics if an event arrives — only safe for objects
// that never emit events. Use it for the genuinely event-less ones,
// and write tiny no-op Dispatch impls for the rest. wl_shm sends
// Format events, wl_buffer sends Release, wl_output sends a stream of
// geometry events, wl_surface emits Enter/Leave when it crosses
// outputs — none of which we care about.
delegate_noop!(AppState: wl_compositor::WlCompositor);
delegate_noop!(AppState: wl_shm_pool::WlShmPool);
delegate_noop!(AppState: zwlr_layer_shell_v1::ZwlrLayerShellV1);

macro_rules! ignore_events {
    ($iface:ty) => {
        impl Dispatch<$iface, ()> for AppState {
            fn event(
                _state: &mut Self,
                _proxy: &$iface,
                _event: <$iface as wayland_client::Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        }
    };
}

ignore_events!(wl_shm::WlShm);
ignore_events!(wl_buffer::WlBuffer);
ignore_events!(wl_surface::WlSurface);
ignore_events!(wl_output::WlOutput);

// ── Bootstrap ───────────────────────────────────────────────────────────────

pub fn connect() -> Result<(Connection, EventQueue<AppState>), Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let event_queue = conn.new_event_queue::<AppState>();
    let qh = event_queue.handle();
    let display = conn.display();
    display.get_registry(&qh, ());
    Ok((conn, event_queue))
}
