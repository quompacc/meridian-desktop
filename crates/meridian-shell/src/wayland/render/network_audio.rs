impl MeridianShell {
    pub(crate) fn draw_network_popup(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        if !self.network_popup_open || !self.network_configured {
            return;
        }
        let surface_w = self.network_width;
        let surface_h = self.network_height;
        let card_w = NETWORK_POPUP_WIDTH;
        let card_h = NETWORK_POPUP_HEIGHT;

        let mut card_buf = vec![0u8; (card_w as usize) * (card_h as usize) * 4];
        {
            let mut painter = Painter::new(&mut card_buf, card_w as i32, card_h as i32);
            network_popup::draw_network_popup(
                &mut painter,
                &self.font,
                &self.theme,
                &network_popup::NetworkPopupState {
                    network: self.network_controller.state(),
                    audio: &self.audio_snapshot,
                    battery: &self.battery_snapshot,
                    power_profile: self.power_profile,
                    theme_name: &self.theme_name,
                    power_armed: self
                        .armed_power
                        .as_ref()
                        .map(|(id, _)| id == "power-off")
                        .unwrap_or(false),
                    active_tab: self.network_popup_tab,
                    wifi_networks: &self.wifi_networks,
                },
            );
        }
        round_buffer_corners(
            &mut card_buf,
            card_w as usize,
            card_h as usize,
            crate::ui::tokens::surface_radius_from_config(
                &self.theme,
                meridian_config::ThemeSurface::Popup,
            ),
        );

        let stride = buffer::shm_buffer_stride(surface_w);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.network_buffer,
                surface_w,
                surface_h,
                stride,
            );
            let Some(buf) = buf else {
                warn!("network popup buffer unavailable: reason={:?}", reason);
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.network_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!("network popup canvas unavailable after retry");
                return;
            };
            crate::popup_card::paint_card_with_shadow(
                canvas,
                surface_w,
                surface_h,
                card_w,
                card_h,
                &card_buf,
                crate::ui::tokens::surface_radius_from_config(
                    &self.theme,
                    meridian_config::ThemeSurface::Popup,
                ),
            );
            if let Err(err) = buf.attach_to(self.network_layer.wl_surface()) {
                warn!("network popup buffer attach failed: {}", err);
                return;
            }
            self.network_layer
                .wl_surface()
                .damage_buffer(0, 0, surface_w as i32, surface_h as i32);
            self.network_layer.commit();
            self.network_dirty = false;
            return;
        }
    }

    pub(crate) fn unmap_network_popup(&mut self, reason: CommitReason) {
        debug!(
            "unmap_network_popup: reason={:?} open={} configured={} surface=network attach_none=true commit=true",
            reason, self.network_popup_open, self.network_configured
        );
        self.network_layer.wl_surface().attach(None, 0, 0);
        self.network_layer.commit();
        self.network_dirty = false;
        // network_layer is shared with audio and SNI popups. After unmap we
        // must wait for a fresh configure before committing another buffer,
        // otherwise the next popup re-attaches into a half-mapped surface and
        // the compositor disconnects us with a layer-shell protocol error.
        self.network_configured = false;
    }

    pub(crate) fn draw_audio_popup(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        if !self.audio_popup_open || !self.network_configured {
            return;
        }
        let surface_w = self.audio_width;
        let surface_h = self.audio_height;
        let card_w = AUDIO_POPUP_WIDTH;
        let card_h = AUDIO_POPUP_HEIGHT;

        let mut card_buf = vec![0u8; (card_w as usize) * (card_h as usize) * 4];
        {
            let mut painter = Painter::new(&mut card_buf, card_w as i32, card_h as i32);
            audio_popup::draw_audio_popup(
                &mut painter,
                &self.font,
                &self.theme,
                &self.audio_snapshot,
            );
        }
        round_buffer_corners(
            &mut card_buf,
            card_w as usize,
            card_h as usize,
            crate::ui::tokens::surface_radius_from_config(
                &self.theme,
                meridian_config::ThemeSurface::Popup,
            ),
        );

        let stride = buffer::shm_buffer_stride(surface_w);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.network_buffer,
                surface_w,
                surface_h,
                stride,
            );
            let Some(buf) = buf else {
                warn!("audio popup buffer unavailable: reason={:?}", reason);
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.network_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!("audio popup canvas unavailable after retry");
                return;
            };
            crate::popup_card::paint_card_with_shadow(
                canvas,
                surface_w,
                surface_h,
                card_w,
                card_h,
                &card_buf,
                crate::ui::tokens::surface_radius_from_config(
                    &self.theme,
                    meridian_config::ThemeSurface::Popup,
                ),
            );
            if let Err(err) = buf.attach_to(self.network_layer.wl_surface()) {
                warn!("audio popup buffer attach failed: {}", err);
                return;
            }
            self.network_layer
                .wl_surface()
                .damage_buffer(0, 0, surface_w as i32, surface_h as i32);
            self.network_layer.commit();
            self.audio_dirty = false;
            return;
        }
    }

    pub(crate) fn draw_volume_osd(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        if !self.volume_osd_open || !self.network_configured {
            return;
        }
        let surface_w = self.volume_osd_width;
        let surface_h = self.volume_osd_height;
        let card_w = crate::VOLUME_OSD_WIDTH;
        let card_h = crate::VOLUME_OSD_HEIGHT;

        let mut card_buf = vec![0u8; (card_w as usize) * (card_h as usize) * 4];
        {
            let mut painter = Painter::new(&mut card_buf, card_w as i32, card_h as i32);
            if let Some(ref label) = self.osd_power_profile {
                audio_popup::draw_text_osd(
                    &mut painter,
                    &self.font,
                    &self.theme,
                    "Energieprofil",
                    label,
                );
            } else {
                audio_popup::draw_volume_osd(
                    &mut painter,
                    &self.font,
                    &self.theme,
                    &self.audio_snapshot,
                );
            }
        }
        round_buffer_corners(
            &mut card_buf,
            card_w as usize,
            card_h as usize,
            crate::ui::tokens::surface_radius_from_config(
                &self.theme,
                meridian_config::ThemeSurface::Popup,
            ),
        );

        let stride = buffer::shm_buffer_stride(surface_w);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.network_buffer,
                surface_w,
                surface_h,
                stride,
            );
            let Some(buf) = buf else {
                warn!("volume osd buffer unavailable: reason={:?}", reason);
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.network_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!("volume osd canvas unavailable after retry");
                return;
            };
            crate::popup_card::paint_card_with_shadow(
                canvas,
                surface_w,
                surface_h,
                card_w,
                card_h,
                &card_buf,
                crate::ui::tokens::surface_radius_from_config(
                    &self.theme,
                    meridian_config::ThemeSurface::Popup,
                ),
            );
            if let Err(err) = buf.attach_to(self.network_layer.wl_surface()) {
                warn!("volume osd buffer attach failed: {}", err);
                return;
            }
            self.network_layer
                .wl_surface()
                .damage_buffer(0, 0, surface_w as i32, surface_h as i32);
            self.network_layer.commit();
            return;
        }
    }

    pub(crate) fn unmap_audio_popup(&mut self, reason: CommitReason) {
        debug!(
            "unmap_audio_popup: reason={:?} open={} configured={} surface=network attach_none=true commit=true",
            reason, self.audio_popup_open, self.network_configured
        );
        self.network_layer.wl_surface().attach(None, 0, 0);
        self.network_layer.commit();
        self.audio_dirty = false;
        self.network_configured = false;
    }
}
