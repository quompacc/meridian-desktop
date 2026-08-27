impl MeridianShell {
    pub(crate) fn tick_timer_interval(&self) -> Duration {
        if self.needs_fast_tick() {
            Duration::from_millis(250)
        } else {
            Duration::from_secs(1)
        }
    }

    pub(crate) fn notification_timer_interval(&self) -> Duration {
        if !self.notifications.is_empty() || self.wallpaper_picker_rx.is_some() {
            Duration::from_millis(250)
        } else {
            Duration::from_secs(1)
        }
    }

    fn needs_fast_tick(&self) -> bool {
        self.armed_power.is_some()
            || self.thumbnail_dirty && self.thumbnail_popup_open
            || !self.thumbnail_popup_open
                && self.thumbnail_hover_app_idx.is_some()
                && self.thumbnail_hover_since.is_some()
    }

    pub(crate) fn panel_active_workspace(&self) -> u8 {
        select_panel_active_workspace(
            self.active_workspace,
            self.output_workspace_state_available,
            self.focused_output_id,
            &self.output_workspaces,
        )
    }

    pub(crate) fn tick(&mut self, qh: &QueueHandle<Self>) {
        let now = Instant::now();
        if self.volume_osd_open && !self.audio_volume_dragging {
            if let Some(hide_at) = self.volume_osd_hide_at {
                if now >= hide_at {
                    self.close_volume_osd(CommitReason::EventLoopTick);
                }
            }
        }
        if now.duration_since(self.last_tick) >= Duration::from_secs(1) {
            self.last_tick = now;
            let clock = time::formatted_time();
            if clock != self.last_clock {
                self.last_clock = clock;
                self.draw_panel(qh, RepaintReason::Clock);
                if self.calendar_popup_open {
                    self.draw_calendar_popup(qh, RepaintReason::Clock);
                }
            }
            // Battery changes slowly; poll on the 1s tick and only repaint the
            // panel when the snapshot actually changed.
            let battery = crate::battery::BatterySnapshot::poll();
            if battery != self.battery_snapshot {
                self.battery_snapshot = battery;
                self.draw_panel(qh, RepaintReason::Clock);
            }
            // AUDIO-1: the user-session PipeWire/WirePlumber stack can come up
            // AFTER the shell's one-shot startup poll, leaving the panel showing
            // a stale muted icon until the user clicks the tray. Re-poll here
            // until the stack settles (running + a default sink), then stop — so
            // the panel self-heals within ~1s of PipeWire becoming ready without
            // spawning wpctl forever. Bounded by audio_poll_until for machines
            // that never expose a sink.
            if !self.audio_settled && now < self.audio_poll_until {
                let audio = crate::audio::AudioSnapshot::poll();
                self.audio_settled = audio.is_settled();
                if audio != self.audio_snapshot {
                    self.audio_snapshot = audio;
                    self.draw_panel(qh, RepaintReason::Clock);
                }
            }
        }
        // Swap in a finished background app-list rescan (LAUNCH-2). Cheap
        // try_recv every tick so a fresh list appears promptly after open.
        self.poll_launcher_apps_refresh(qh);
        // Apply a finished off-thread icon warm (LAUNCH-3). Cheap try_recv.
        self.poll_launcher_icons_warm(qh);
        // Settings pages render cached state immediately. Any slower platform
        // query or image decode completes here without blocking input.
        self.poll_settings_refresh(qh);

        self.maybe_log_repaint_stats(now);
        self.maybe_log_commit_stats(now);
        self.maybe_log_render_stats(now);

        if self.ipc.should_reconnect() {
            self.ipc.reconnect();
        }

        if !self.workspace_state_received
            && !self.ipc.is_connected()
            && !self.workspace_ipc_unavailable_logged
        {
            debug!("IPC workspace state unavailable; using fallback workspace 1");
            self.workspace_ipc_unavailable_logged = true;
        }

        if !self.occupied_state_available && !self.occupied_unavailable_logged {
            debug!("window snapshot unavailable; occupied state fallback active-only");
            self.occupied_unavailable_logged = true;
        }

        // Thumbnail hover delay: open popup if 400ms passed while hovering a pinned app
        if !self.thumbnail_popup_open {
            if let (Some(idx), Some(since)) =
                (self.thumbnail_hover_app_idx, self.thumbnail_hover_since)
            {
                let elapsed = since.elapsed().as_millis();
                if elapsed >= crate::THUMBNAIL_HOVER_DELAY_MS {
                    if let Some(app) = self.pinned_apps.get(idx).cloned() {
                        let ws = self.panel_active_workspace();
                        let window_ids =
                            crate::wayland::state::pinned_app_window_ids(&app, &self.windows, ws);
                        if !window_ids.is_empty() {
                            // Wait until prefetched thumbs land (or open after
                            // timeout) so the popup starts at the correct width
                            // instead of opening at max-placeholder size and
                            // snapping smaller a tick later.
                            let all_cached = window_ids
                                .iter()
                                .take(crate::THUMBNAIL_MAX_WINDOWS)
                                .all(|id| self.thumbnail_cache.contains_key(id.as_str()));
                            let timed_out = elapsed >= crate::THUMBNAIL_OPEN_TIMEOUT_MS;
                            if all_cached || timed_out {
                                let icon_center = self.panel_state.clicks.iter()
                                    .find(|z| matches!(z.action, crate::ClickAction::LaunchPinnedApp(i) if i == idx))
                                    .map(|z| z.rect.x + z.rect.w / 2);
                                self.open_thumbnail_popup(qh, &window_ids, icon_center);
                            }
                        }
                    }
                }
            }
        }

        // Resize + redraw thumbnail popup when new thumbnails arrive
        if self.thumbnail_dirty && self.thumbnail_popup_open {
            self.refresh_thumbnail_popup(qh);
        }

        // Drive the power-button countdown ring: on timeout, clear the armed
        // state and redraw so the ring vanishes; while armed, redraw every
        // tick so the ring visibly drains.
        if let Some((_, armed_at)) = &self.armed_power {
            let elapsed = armed_at.elapsed().as_millis();
            if elapsed >= crate::POWER_ARM_TIMEOUT_MS {
                self.armed_power = None;
                self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
            } else {
                self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
            }
        }
    }

    pub(crate) fn request_settings_refresh(
        &mut self,
        category: crate::settings_view::SettingsCategory,
    ) {
        if !crate::settings_refresh::supports(category) {
            return;
        }
        if category == crate::settings_view::SettingsCategory::Wallpaper
            && !self.wallpaper_thumbnails.is_empty()
        {
            return;
        }
        if !self.settings_refresh_inflight.insert(category) {
            return;
        }
        crate::settings_refresh::spawn(
            category,
            self.available_wallpapers.clone(),
            self.settings_refresh_tx.clone(),
        );
    }

    fn poll_settings_refresh(&mut self, qh: &QueueHandle<Self>) {
        while let Ok(result) = self.settings_refresh_rx.try_recv() {
            self.settings_refresh_inflight.remove(&result.category);
            match result.data {
                crate::settings_refresh::SettingsData::SystemInfo(value) => {
                    self.system_info = value;
                }
                crate::settings_refresh::SettingsData::Printers(value) => {
                    self.printer_snapshot = value;
                }
                crate::settings_refresh::SettingsData::Audio(value) => {
                    self.audio_settled = value.is_settled();
                    self.audio_snapshot = value;
                }
                crate::settings_refresh::SettingsData::Network { profiles, wifi } => {
                    self.network_profiles = profiles;
                    self.wifi_networks = wifi;
                }
                crate::settings_refresh::SettingsData::Bluetooth(value) => {
                    self.bluetooth_snapshot = value;
                }
                crate::settings_refresh::SettingsData::DefaultApps { index, current } => {
                    self.default_apps_index = Some(index);
                    self.default_apps_current = current;
                    self.default_apps_loaded = true;
                }
                crate::settings_refresh::SettingsData::WallpaperThumbnails(value) => {
                    self.wallpaper_thumbnails = value;
                }
            }
            if self.launcher_state.open
                && self.launcher_settings_open
                && self.settings_category == result.category
            {
                self.draw_launcher(qh, RepaintReason::Ipc);
            }
        }
    }

    pub(crate) fn poll_ipc(&mut self) -> bool {
        let mut changed = false;
        for event in self.ipc.poll() {
            self.apply_ipc_event(event);
            changed = true;
        }
        changed
    }

    fn open_desktop_context_menu_from_ipc(&mut self, x: i32, y: i32) {
        let desktop_w = self
            .desktop_width
            .max(crate::context_menu::MENU_WIDTH as u32) as i32;
        let desktop_h = self.desktop_height.max(1) as i32;
        let (x, y) = crate::context_menu::desktop_clamp_position(x, y, desktop_w, desktop_h);
        let n_desktop = crate::context_menu::desktop_item_list().len();
        let menu_height = crate::context_menu::surface_height(n_desktop, false) as u32;

        self.desktop_context_menu = Some(crate::context_menu::DesktopContextMenuState {
            x,
            y,
            hover_idx: None,
            submenu_open: false,
            submenu_hover_idx: None,
        });
        self.desktop_menu_open = true;
        self.desktop_menu_opened_at = Some(std::time::Instant::now());
        self.desktop_menu_width = crate::popup_surface_w(crate::context_menu::MENU_WIDTH as u32);
        self.desktop_menu_height = crate::popup_surface_h(menu_height.max(1));
        self.desktop_menu_buffer = None;
        self.desktop_menu_layer
            .set_anchor(Anchor::TOP | Anchor::LEFT);
        // Compensate the shadow pad so the menu still opens at the click
        // position. Margins clamp to 0 if subtraction would go negative;
        // that only happens within POPUP_SHADOW_PAD of the screen edge.
        let mx = (x.max(0) - crate::POPUP_SHADOW_PAD).max(0);
        let my = (y.max(0) - crate::POPUP_SHADOW_PAD).max(0);
        self.desktop_menu_layer.set_margin(my, 0, 0, mx);
        self.desktop_menu_layer
            .set_size(self.desktop_menu_width, self.desktop_menu_height);
    }

    /// Refresh after a deliberate default-app write. Page entry itself uses
    /// the non-blocking Settings worker above.
    pub(crate) fn refresh_default_apps_snapshot(&mut self) {
        self.default_apps_index = Some(crate::default_apps::MimeAppIndex::load_system());
        self.default_apps_current = crate::default_apps::snapshot_current_defaults();
        self.default_apps_loaded = true;
    }

    pub(crate) fn open_consent_modal(&mut self, request_id: String, app_id: String) {
        self.consent_request_id = Some(request_id);
        self.consent_app_id = app_id;
        self.consent_open = true;
        self.consent_hover = None;
        // Re-assert the modal's geometry on every open: after the first close +
        // re-open cycle sctk's pending set_size is gone, so without this the next
        // commit attempts width=0 and wlr-layer-shell sends error 1, killing the
        // surface for the rest of the session. The IPC redraw path picks up
        // consent_open and draws on the next configure / tick.
        self.consent_layer.set_size(
            crate::screenshot_consent::MODAL_WIDTH as u32,
            crate::screenshot_consent::MODAL_HEIGHT as u32,
        );
    }

    pub(crate) fn respond_consent(&mut self, allowed: bool) {
        if let Some(request_id) = self.consent_request_id.take() {
            let _ = self
                .ipc
                .send(&meridian_ipc::ShellCommand::ScreenshotConsentResponse {
                    request_id,
                    allowed,
                });
        }
        self.consent_open = false;
        self.consent_app_id.clear();
        self.consent_hover = None;
        self.unmap_consent(crate::wayland::CommitReason::Input);
    }

    /// Open the centered Wi-Fi password modal for `ssid`. Closes the network
    /// tray popup first (both are Overlay layers; we never want two grabbing
    /// the keyboard). Caller draws it (it has a QueueHandle).
    pub(crate) fn open_wifi_password_modal(&mut self, ssid: String) {
        if self.network_popup_open {
            self.close_network_popup(crate::wayland::CommitReason::Input);
        }
        self.wifi_password_prompt = Some(ssid);
        self.wifi_password_input.clear();
        self.wifi_modal_open = true;
        self.wifi_modal_hover = None;
        // Re-assert geometry on every open: after a close+reopen cycle sctk's
        // pending set_size is gone, so the next commit would attempt width=0 and
        // wlr-layer-shell would kill the surface. Same guard as the consent modal.
        self.wifi_modal_layer.set_size(
            crate::wifi_password_modal::MODAL_WIDTH as u32,
            crate::wifi_password_modal::MODAL_HEIGHT as u32,
        );
    }

    /// Cancel the Wi-Fi password modal without connecting.
    pub(crate) fn close_wifi_password_modal(&mut self) {
        self.wifi_password_prompt = None;
        self.wifi_password_input.clear();
        self.wifi_modal_open = false;
        self.wifi_modal_hover = None;
        self.unmap_wifi_modal(crate::wayland::CommitReason::Input);
    }

    /// Connect to the modal's SSID with the typed password, then close. A blank
    /// password is treated as cancel (nmcli would just fail on a secured net).
    pub(crate) fn submit_wifi_password_modal(&mut self) {
        if let Some(ssid) = self.wifi_password_prompt.take() {
            if !self.wifi_password_input.is_empty() {
                crate::network::connect_wifi(&ssid, Some(&self.wifi_password_input));
            }
        }
        self.wifi_password_input.clear();
        self.wifi_modal_open = false;
        self.wifi_modal_hover = None;
        self.unmap_wifi_modal(crate::wayland::CommitReason::Input);
    }

    /// Switch the network tray popup between the Status and WLAN tabs. Entering
    /// the WLAN tab refreshes the (cached, fast) scan + saved-profile list so
    /// the list and "secured/known" decisions are current.
    pub(crate) fn switch_network_tab(
        &mut self,
        qh: &QueueHandle<Self>,
        tab: crate::network_popup::NetworkTab,
    ) {
        if self.network_popup_tab == tab {
            return;
        }
        self.network_popup_tab = tab;
        if tab == crate::network_popup::NetworkTab::Wifi {
            self.network_profiles = crate::network::list_saved_connections();
            self.wifi_networks = crate::network::scan_wifi_networks();
        } else {
            // Back to the Status tab: re-poll so it reflects the current primary
            // connection (it may have changed while the WLAN tab was open).
            self.network_controller.poll();
        }
        self.network_dirty = true;
        self.draw_network_popup(qh, crate::wayland::RepaintReason::Pointer);
    }

    /// Act on a click of Wi-Fi list row `idx` in the tray popup: connect to an
    /// open/known network straight away, or open the password modal for a
    /// secured one. Mirrors the Settings page `WifiConnect` dispatch.
    pub(crate) fn connect_wifi_from_popup(&mut self, qh: &QueueHandle<Self>, idx: usize) {
        let Some(net) = self.wifi_networks.get(idx) else {
            return;
        };
        let ssid = net.ssid.clone();
        let known = self.network_profiles.iter().any(|p| p.name == ssid);
        if net.secured && !known {
            self.open_wifi_password_modal(ssid);
            self.draw_wifi_modal(qh, crate::wayland::RepaintReason::Pointer);
        } else {
            crate::network::connect_wifi(&ssid, None);
        }
    }

    pub(crate) fn open_region_picker(&mut self, request_id: String, app_id: String) {
        self.region_picker_request_id = Some(request_id);
        self.region_picker_app_id = app_id;
        self.region_picker_local = false;
        self.region_picker_open = true;
        self.region_picker_drag_start = None;
        self.region_picker_drag_current = None;
        self.region_picker_pending = None;
        // sctk loses the layer surface's pending anchor + size after a
        // close+reopen cycle; re-assert them here so the next commit can
        // map the surface again.
        self.reassert_region_picker_layer_state();
    }

    /// Open the region picker in local-screenshot mode: on confirm the
    /// picked region drives a shell-side `ext_image_copy_capture` and the
    /// cropped PNG is saved to ~/Pictures/Screenshots/. Used by the panel
    /// screenshot button.
    pub(crate) fn open_region_picker_local(&mut self, qh: &QueueHandle<Self>) {
        self.region_picker_request_id = None;
        self.region_picker_app_id.clear();
        self.region_picker_local = true;
        self.region_picker_open = true;
        self.region_picker_drag_start = None;
        self.region_picker_drag_current = None;
        self.region_picker_pending = None;
        self.reassert_region_picker_layer_state();
        // Draw immediately so the panel-button click produces a visible
        // overlay on the first attempt — without this the picker only
        // appears after the next configure round (= second click).
        self.draw_region_picker(qh, crate::wayland::RepaintReason::Pointer);
    }

    /// Re-set the picker layer surface's anchor + (zero) size so a commit
    /// after the first close+reopen cycle still has valid pending state.
    /// Anchored on all four edges so the compositor stretches it to fill
    /// the output; size 0x0 means "use my anchored bounding box".
    fn reassert_region_picker_layer_state(&mut self) {
        use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity};
        self.region_picker_layer
            .set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        self.region_picker_layer.set_exclusive_zone(-1);
        self.region_picker_layer
            .set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        self.region_picker_layer.set_size(0, 0);
    }

    /// Kick off a shell-side ext_image_copy_capture for the picked region,
    /// targeted at ~/Pictures/Screenshots/meridian-<unix>.png. The crop is
    /// applied inside the screencopy Ready handler before PNG encoding.
    pub(crate) fn start_local_screenshot(
        &mut self,
        qh: &QueueHandle<Self>,
        region: meridian_ipc::ScreenshotRegion,
    ) {
        if self.screenshot_capture.is_some() {
            return;
        }
        let (Some(mgr), Some(src_mgr)) = (
            self.screencopy_manager.as_ref(),
            self.capture_source_manager.as_ref(),
        ) else {
            tracing::warn!("screenshot: ext_image_copy_capture not available");
            return;
        };
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let dir = std::path::PathBuf::from(&home)
            .join("Pictures")
            .join("Screenshots");
        let _ = std::fs::create_dir_all(&dir);
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("meridian-{}.png", secs));
        // P1-1 (AUDIT_2026-08-19): the region picker (and thus its region
        // coordinates) lives on the primary output because its layer surface
        // was created without an explicit output. Match that name against the
        // wl_output globals; without IPC state or a known name fall back to
        // the first output (previous single-output behaviour).
        let wanted_name = select_local_capture_output_name(&self.output_workspaces);
        let outputs: Vec<_> = self.output_state.outputs().collect();
        let wl_output = match wanted_name {
            Some(name) => {
                let matched = outputs.iter().find(|output| {
                    self.output_state
                        .info(output)
                        .and_then(|info| info.name.clone())
                        .as_deref()
                            == Some(name)
                });
                matched.cloned().or_else(|| {
                    tracing::debug!(
                        "screenshot: capture output {name} not found among wl_outputs; using first"
                    );
                    outputs.first().cloned()
                })
            }
            None => outputs.first().cloned(),
        };
        let Some(wl_output) = wl_output else {
            tracing::warn!("screenshot: no output available");
            return;
        };
        use wayland_protocols::ext::{
            image_capture_source::v1::client::ext_image_capture_source_v1::ExtImageCaptureSourceV1,
            image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::Options,
        };
        let capture_source: ExtImageCaptureSourceV1 = src_mgr.create_source(&wl_output, qh, ());
        let session = mgr.create_session(&capture_source, Options::empty(), qh, ());
        capture_source.destroy();
        self.screenshot_capture = Some(crate::wayland::screencopy::ScreenshotCapture {
            session,
            path,
            width: 0,
            height: 0,
            format: None,
            constraints_done: false,
            pool: None,
            buffer: None,
            frame: None,
            fd: None,
            mapped_ptr: std::ptr::null_mut(),
            mapped_len: 0,
            region: Some(region),
        });
    }

    pub(crate) fn respond_region(
        &mut self,
        qh: &QueueHandle<Self>,
        region: Option<meridian_ipc::ScreenshotRegion>,
    ) {
        if let Some(request_id) = self.region_picker_request_id.take() {
            let _ = self
                .ipc
                .send(&meridian_ipc::ShellCommand::ScreenshotRegionResponse { request_id, region });
        } else if self.region_picker_local {
            // Local panel-button path: confirmed region drives a shell-side
            // screencopy; Esc / no-selection drops silently.
            if let Some(region) = region {
                self.start_local_screenshot(qh, region);
            }
        }
        self.region_picker_local = false;
        self.region_picker_open = false;
        self.region_picker_app_id.clear();
        self.region_picker_drag_start = None;
        self.region_picker_drag_current = None;
        self.region_picker_pending = None;
        self.unmap_region_picker(crate::wayland::CommitReason::Input);
    }

    fn close_desktop_context_menu_from_ipc(&mut self) {
        if self.desktop_menu_open || self.desktop_context_menu.is_some() {
            self.desktop_context_menu = None;
            self.desktop_menu_open = false;
            self.desktop_menu_opened_at = None;
            self.unmap_desktop_menu(CommitReason::UnknownOther);
        }
    }

    fn desktop_menu_in_open_debounce(&self) -> bool {
        self.desktop_menu_opened_at
            .map(|t| t.elapsed() < std::time::Duration::from_millis(150))
            .unwrap_or(false)
    }
}
