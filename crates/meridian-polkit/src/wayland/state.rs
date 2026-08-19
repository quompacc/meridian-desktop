impl AppState {
    pub fn new(theme: ThemeConfig, pam_tx: cchannel::Sender<PamResult>) -> Self {
        Self {
            running: true,
            theme,
            compositor: None,
            shm: None,
            seat: None,
            layer_shell: None,
            xkb_ctx: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            xkb_state: None,
            active: None,
            popup: None,
            pam_tx,
        }
    }

    pub fn on_auth_request(&mut self, req: AuthRequest, qh: &QueueHandle<Self>) {
        // First-come-first-served. If a popup is already up, decline the
        // new one — polkit will retry. Most desktops queue; we'll add a
        // queue if it becomes a real annoyance.
        if self.active.is_some() {
            warn!(cookie = %req.cookie, "polkit: popup already active, declining new request");
            let _ = req.reply.send(Outcome::Cancelled);
            return;
        }
        // Refresh the theme from disk on every auth request so live
        // theme switches in the shell (config rewrite) take effect on
        // the next popup, not on next agent restart.
        self.reload_theme();
        let identity = req.identities.first().cloned().unwrap_or(Identity {
            uid: 0,
            username: "root".to_string(),
        });
        info!(
            action_id = %req.action_id,
            cookie = %req.cookie,
            identity = %identity.username,
            "polkit: opening auth popup"
        );
        self.active = Some(ActiveAuth {
            action_id: req.action_id,
            message: req.message,
            cookie: req.cookie,
            identity,
            password: Zeroizing::new(String::new()),
            status: ui::Status::Idle,
            retries: 0,
            reply: Some(req.reply),
        });
        self.ensure_popup(qh);
        // First draw arrives via the layer_surface Configure event.
    }

    fn reload_theme(&mut self) {
        let config = MeridianConfig::load();
        let mut manager = ThemeManager::new();
        if !config.general.theme.is_empty() && config.general.theme != manager.current().name {
            if let Err(err) = manager.set_theme(&config.general.theme) {
                warn!(
                    "polkit: failed to reload theme {:?}: {} (keeping previous)",
                    config.general.theme, err
                );
                return;
            }
        }
        let new_name = manager.current().name.clone();
        self.theme = manager.current().config.clone();
        debug!(theme = %new_name, "polkit: theme reloaded");
    }

    pub fn on_cancel_from_polkit(&mut self, cookie: String) {
        if let Some(active) = &self.active {
            if active.cookie == cookie {
                debug!(cookie = %cookie, "polkit: external cancel; closing popup");
                self.finish(Outcome::Cancelled);
            }
        }
    }

    pub fn on_pam_result(&mut self, result: PamResult) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        if active.cookie != result.cookie {
            return;
        }
        if result.ok {
            let uid = active.identity.uid;
            let username = active.identity.username.clone();
            info!(uid, username = %username, "polkit: PAM success");
            self.finish(Outcome::Authenticated { uid, username });
        } else {
            active.retries += 1;
            active.status = ui::Status::Failed;
            active.password = Zeroizing::new(String::new());
            warn!(retries = active.retries, "polkit: PAM failure");
            if active.retries >= 3 {
                info!("polkit: too many failures; declining");
                self.finish(Outcome::Cancelled);
            }
            // caller redraws via draw(&qh) after on_pam_result returns
        }
    }

    fn finish(&mut self, outcome: Outcome) {
        if let Some(mut active) = self.active.take() {
            if let Some(reply) = active.reply.take() {
                let _ = reply.send(outcome);
            }
        }
        self.destroy_popup();
    }

    fn ensure_popup(&mut self, qh: &QueueHandle<Self>) {
        if self.popup.is_some() {
            return;
        }
        let Some(compositor) = self.compositor.clone() else {
            warn!("polkit: no compositor global yet");
            return;
        };
        let Some(layer_shell) = self.layer_shell.clone() else {
            warn!("polkit: no layer_shell global yet");
            return;
        };
        let surface = compositor.create_surface(qh, ());
        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            None, // any output (the compositor picks the focused one)
            zwlr_layer_shell_v1::Layer::Overlay,
            "meridian-polkit".to_string(),
            qh,
            (),
        );
        layer_surface.set_size(POPUP_W, POPUP_H);
        layer_surface
            .set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive);
        // No anchor → centered by the compositor.
        surface.commit();
        self.popup = Some(PopupSurface {
            surface,
            layer_surface,
            width: POPUP_W,
            height: POPUP_H,
            configured: false,
            shm_ptr: std::ptr::null_mut(),
            shm_size: 0,
            buffer: None,
        });
    }

    fn destroy_popup(&mut self) {
        if let Some(mut p) = self.popup.take() {
            if let Some(buf) = p.buffer.take() {
                buf.destroy();
            }
            if !p.shm_ptr.is_null() {
                unsafe { libc::munmap(p.shm_ptr as *mut c_void, p.shm_size) };
            }
            p.layer_surface.destroy();
            p.surface.destroy();
        }
    }

    /// Render path that has access to a real QueueHandle.
    pub fn draw(&mut self, qh: &QueueHandle<Self>) {
        let (w, h) = match self.popup.as_ref() {
            Some(p) if p.configured => (p.width, p.height),
            _ => return,
        };
        let stride = (w * 4) as i32;
        let size = (stride as u32 * h) as usize;

        // (Re)allocate shm if needed
        if self
            .popup
            .as_ref()
            .map(|p| p.shm_size != size || p.buffer.is_none())
            .unwrap_or(false)
        {
            // Tear down old buffer
            {
                let p = self.popup.as_mut().unwrap();
                if let Some(buf) = p.buffer.take() {
                    buf.destroy();
                }
                if !p.shm_ptr.is_null() {
                    unsafe { libc::munmap(p.shm_ptr as *mut c_void, p.shm_size) };
                    p.shm_ptr = std::ptr::null_mut();
                    p.shm_size = 0;
                }
            }
            let owned_fd = match create_anonymous_shm() {
                Ok(fd) => fd,
                Err(err) => {
                    warn!(%err, "shared-memory file creation failed");
                    return;
                }
            };
            unsafe {
                if libc::ftruncate(owned_fd.as_raw_fd(), size as i64) < 0 {
                    warn!("ftruncate failed");
                    return;
                }
            }
            let ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    size,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    owned_fd.as_raw_fd(),
                    0,
                )
            };
            if ptr == libc::MAP_FAILED {
                warn!("mmap failed");
                return;
            }
            let shm = self.shm.as_ref().unwrap();
            let pool = shm.create_pool(owned_fd.as_fd(), size as i32, qh, ());
            let buf = pool.create_buffer(
                0,
                w as i32,
                h as i32,
                stride,
                wl_shm::Format::Argb8888,
                qh,
                (),
            );
            pool.destroy();
            let p = self.popup.as_mut().unwrap();
            p.shm_ptr = ptr as *mut u8;
            p.shm_size = size;
            p.buffer = Some(buf);
        }

        let popup = self.popup.as_ref().unwrap();
        let active = match &self.active {
            Some(a) => a,
            None => return,
        };

        let pixels: &mut [u8] = unsafe { std::slice::from_raw_parts_mut(popup.shm_ptr, size) };
        let view = ui::View {
            title: "Authentifizierung erforderlich",
            message: if active.message.is_empty() {
                "Eine Anwendung benötigt Administratorrechte."
            } else {
                active.message.as_str()
            },
            username: &active.identity.username,
            password_len: active.password.chars().count(),
            status: active.status,
            hint: "Enter zum Bestätigen · Esc zum Abbrechen",
        };
        ui::render(pixels, w, h, &self.theme, &view);

        if let Some(buf) = &popup.buffer {
            popup.surface.attach(Some(buf), 0, 0);
            popup.surface.damage_buffer(0, 0, w as i32, h as i32);
            popup.surface.commit();
        }
    }

    fn handle_key(&mut self, linux_key: u32) -> bool {
        let Some(active) = self.active.as_mut() else {
            return false;
        };
        let keycode = linux_key + 8;
        let Some(xkb_state) = self.xkb_state.as_ref() else {
            return false;
        };
        let keysym = xkb_state.key_get_one_sym(keycode.into());
        match keysym {
            xkb::Keysym::Return | xkb::Keysym::KP_Enter => {
                if active.password.is_empty() {
                    return false;
                }
                active.status = ui::Status::Checking;
                let username = active.identity.username.clone();
                let cookie = active.cookie.clone();
                let password =
                    std::mem::replace(&mut active.password, Zeroizing::new(String::new()));
                // Stash for redraw: show "Checking" but keep password
                // count at 0. We took it out so the user can keep
                // typing while PAM is running; on failure we wipe.
                let cookie_for_helper = cookie.clone();
                let tx = self.pam_tx.clone();
                std::thread::spawn(move || {
                    let ok = crate::auth::authenticate_via_helper(
                        &username,
                        &cookie_for_helper,
                        &password,
                    );
                    let _ = tx.send(PamResult { cookie, ok });
                });
                true
            }
            xkb::Keysym::Escape => {
                self.finish(Outcome::Cancelled);
                true
            }
            xkb::Keysym::BackSpace => {
                active.password.pop();
                if active.status == ui::Status::Failed {
                    active.status = ui::Status::Idle;
                }
                true
            }
            _ => {
                let utf8 = xkb_state.key_get_utf8(keycode.into());
                if !utf8.is_empty() && !utf8.chars().any(|c| c.is_control()) {
                    active.password.push_str(&utf8);
                    if active.status == ui::Status::Failed {
                        active.status = ui::Status::Idle;
                    }
                    true
                } else {
                    false
                }
            }
        }
    }
}

// ── Registry ────────────────────────────────────────────────────────────────
