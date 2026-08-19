impl MeridianShell {
    pub(crate) fn draw_status_notifier_menu(
        &mut self,
        _qh: &QueueHandle<Self>,
        reason: RepaintReason,
    ) {
        if !self.status_notifier_menu_open || !self.network_configured {
            return;
        }
        let surface_w = self.status_notifier_menu_width;
        let surface_h = self.status_notifier_menu_height;
        let pad2 = 2 * crate::POPUP_SHADOW_PAD as u32;
        let card_w = surface_w.saturating_sub(pad2).max(1);
        let card_h = surface_h.saturating_sub(pad2).max(1);

        let mut card_buf = vec![0u8; (card_w as usize) * (card_h as usize) * 4];
        {
            let mut painter = Painter::new(&mut card_buf, card_w as i32, card_h as i32);
            let title = self
                .status_notifier_menu
                .as_ref()
                .map(|m| m.service.rsplit('.').next().unwrap_or(m.service.as_str()))
                .unwrap_or("");
            status_notifier_popup::draw_status_notifier_menu(
                &mut painter,
                &self.font,
                &self.theme,
                title,
                &self.status_notifier_menu_entries,
                card_h,
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
                warn!("SNI menu buffer unavailable: reason={:?}", reason);
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.network_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!("SNI menu canvas unavailable after retry");
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
                warn!("SNI menu buffer attach failed: {}", err);
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

    pub(crate) fn unmap_status_notifier_menu(&mut self, reason: CommitReason) {
        debug!(
            "unmap_status_notifier_menu: reason={:?} open={} configured={} surface=network attach_none=true commit=true",
            reason, self.status_notifier_menu_open, self.network_configured
        );
        self.network_layer.wl_surface().attach(None, 0, 0);
        self.network_layer.commit();
        self.network_dirty = false;
        self.network_configured = false;
    }

    /// Phase A1.3: paint the front notification onto the dedicated
    /// top-right layer-surface. If the queue is empty the caller should
    /// invoke [`Self::unmap_notification_popup`] instead.
    pub(crate) fn draw_notification_popup(
        &mut self,
        _qh: &QueueHandle<Self>,
        reason: RepaintReason,
    ) {
        let Some(notif) = self.notifications.back().cloned() else {
            self.unmap_notification_popup(CommitReason::UnknownOther);
            return;
        };
        if !self.notification_configured {
            return;
        }
        let surface_w = self.notification_width;
        let surface_h = self.notification_height;
        let card_w = NOTIFICATION_WIDTH;
        let card_h = NOTIFICATION_HEIGHT;

        let mut card_buf = vec![0u8; (card_w as usize) * (card_h as usize) * 4];
        {
            let mut painter = Painter::new(&mut card_buf, card_w as i32, card_h as i32);
            notification_popup::draw_notification(&mut painter, &self.font, &self.theme, &notif);
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
                &mut self.notification_buffer,
                surface_w,
                surface_h,
                stride,
            );
            let Some(buf) = buf else {
                warn!("notification buffer unavailable: reason={:?}", reason);
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.notification_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!("notification canvas unavailable after retry");
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
            if let Err(err) = buf.attach_to(self.notification_layer.wl_surface()) {
                warn!("notification buffer attach failed: {}", err);
                return;
            }
            self.notification_layer.wl_surface().damage_buffer(
                0,
                0,
                surface_w as i32,
                surface_h as i32,
            );
            self.notification_layer.commit();
            self.notification_dirty = false;
            return;
        }
    }

    pub(crate) fn unmap_notification_popup(&mut self, reason: CommitReason) {
        debug!(
            "unmap_notification_popup: reason={:?} configured={} surface=notification attach_none=true commit=true",
            reason, self.notification_configured
        );
        self.notification_layer.wl_surface().attach(None, 0, 0);
        self.notification_layer.commit();
        self.notification_dirty = false;
    }

    pub(crate) fn draw_thumbnail_popup(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        if !self.thumbnail_popup_open || !self.thumbnail_configured {
            return;
        }
        let surface_w = self.thumbnail_width;
        let surface_h = self.thumbnail_height;
        let pad2 = 2 * crate::POPUP_SHADOW_PAD as u32;
        let card_w = surface_w.saturating_sub(pad2).max(1);
        let card_h = surface_h.saturating_sub(pad2).max(1);

        let mut card_buf = vec![0u8; (card_w as usize) * (card_h as usize) * 4];
        {
            let mut painter = Painter::new(&mut card_buf, card_w as i32, card_h as i32);
            crate::thumbnail_popup::draw_thumbnail_popup(
                &mut painter,
                &self.theme,
                &self.thumbnail_cache,
                &self.thumbnail_popup_window_ids,
                card_w,
                card_h,
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
                &mut self.thumbnail_buffer,
                surface_w,
                surface_h,
                stride,
            );
            let Some(buf) = buf else {
                warn!("thumbnail buffer unavailable: reason={:?}", reason);
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.thumbnail_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!("thumbnail canvas unavailable after retry");
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
            if let Err(err) = buf.attach_to(self.thumbnail_layer.wl_surface()) {
                warn!("thumbnail buffer attach failed: {}", err);
                return;
            }
            self.thumbnail_layer.wl_surface().damage_buffer(
                0,
                0,
                surface_w as i32,
                surface_h as i32,
            );
            self.thumbnail_layer.commit();
            self.thumbnail_dirty = false;
            return;
        }
    }

    pub(crate) fn unmap_thumbnail_popup(&mut self, _reason: CommitReason) {
        self.thumbnail_layer.wl_surface().attach(None, 0, 0);
        self.thumbnail_layer.commit();
        self.thumbnail_dirty = false;
    }

    fn write_panel_click_zones_snapshot(&mut self, width: u32, height: u32) {
        let zones: Vec<_> = self
            .panel_state
            .clicks
            .iter()
            .map(|zone| {
                serde_json::json!({
                    "id": zone.id.as_deref(),
                    "action": zone.action.test_name(),
                    "x": zone.rect.x,
                    "y": zone.rect.y,
                    "w": zone.rect.w,
                    "h": zone.rect.h,
                    "center_x": zone.rect.x + zone.rect.w / 2,
                    "center_y": zone.rect.y + zone.rect.h / 2,
                })
            })
            .collect();
        let payload = serde_json::json!({
            "surface": "panel",
            "width": width,
            "height": height,
            "zones": zones,
        });
        let Ok(snapshot) = serde_json::to_string_pretty(&payload) else {
            return;
        };
        if self.panel_click_zones_snapshot.as_deref() == Some(snapshot.as_str()) {
            return;
        }
        self.panel_click_zones_snapshot = Some(snapshot.clone());

        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
        let path = std::path::Path::new(&runtime_dir).join("meridian-panel-click-zones.json");
        let tmp_path = path.with_extension("json.tmp");
        if std::fs::write(&tmp_path, snapshot).is_ok() {
            let _ = std::fs::rename(tmp_path, path);
        }
    }
}
