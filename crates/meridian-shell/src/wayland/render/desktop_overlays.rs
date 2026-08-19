impl MeridianShell {
    /// Resize the desktop-menu surface to match whether the settings flyout is open.
    /// Must be called whenever `desktop_context_menu.submenu_open` changes.
    pub(crate) fn resize_desktop_menu_surface(&mut self, submenu_open: bool) {
        use smithay_client_toolkit::shell::wlr_layer::Anchor;
        let n = crate::context_menu::desktop_item_list().len();
        let card_w = crate::context_menu::total_menu_width(submenu_open) as u32;
        let card_h = crate::context_menu::surface_height(n, submenu_open).max(1) as u32;
        self.desktop_menu_width = crate::popup_surface_w(card_w);
        self.desktop_menu_height = crate::popup_surface_h(card_h);
        self.desktop_menu_buffer = None;
        self.desktop_menu_layer
            .set_anchor(Anchor::TOP | Anchor::LEFT);
        self.desktop_menu_layer.set_keyboard_interactivity(
            smithay_client_toolkit::shell::wlr_layer::KeyboardInteractivity::Exclusive,
        );
        self.desktop_menu_layer
            .set_size(self.desktop_menu_width, self.desktop_menu_height);
    }

    pub(crate) fn draw_desktop_menu(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        if !self.desktop_menu_open || !self.desktop_menu_configured {
            return;
        }
        let Some(menu) = self.desktop_context_menu.as_ref() else {
            return;
        };
        let surface_w = self.desktop_menu_width.max(1);
        let surface_h = self.desktop_menu_height.max(1);
        let pad2 = 2 * crate::POPUP_SHADOW_PAD as u32;
        let card_w = surface_w.saturating_sub(pad2).max(1);
        let card_h = surface_h.saturating_sub(pad2).max(1);

        let mut card_buf = vec![0u8; (card_w as usize) * (card_h as usize) * 4];
        let items = crate::context_menu::desktop_item_list();
        let mut panels = vec![Rect {
            x: 0,
            y: 0,
            w: crate::context_menu::MENU_WIDTH,
            h: crate::context_menu::menu_height(items.len()),
        }];
        if menu.submenu_open {
            panels.push(Rect {
                x: crate::context_menu::MENU_WIDTH + crate::context_menu::SUBMENU_GAP,
                y: 0,
                w: crate::context_menu::SUBMENU_WIDTH,
                h: crate::context_menu::submenu_height(),
            });
        }
        let local = crate::context_menu::DesktopContextMenuState {
            x: 0,
            y: 0,
            hover_idx: menu.hover_idx,
            submenu_open: menu.submenu_open,
            submenu_hover_idx: menu.submenu_hover_idx,
        };
        crate::context_menu::draw_desktop_overlay(
            &mut card_buf,
            card_w,
            card_h,
            &local,
            &items,
            &self.theme,
        );
        if self.theme.decorations.glass && self.theme.decorations.glass_blur {
            let border = self.theme.colors.border;
            let border_alpha =
                (self.theme.decorations.glass_frame_alpha.clamp(0.0, 1.0) * 255.0) as u8;
            let border = meridian_config::Color::rgba(border.r, border.g, border.b, border_alpha);
            for panel in &panels {
                crate::popup_card::draw_glass_card_border_in_rect_with_color(
                    &mut card_buf,
                    card_w as i32,
                    card_h as i32,
                    *panel,
                    border,
                    crate::ui::tokens::surface_radius_from_config(
                        &self.theme,
                        meridian_config::ThemeSurface::Popup,
                    ),
                );
            }
        }

        let stride = buffer::shm_buffer_stride(surface_w);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.desktop_menu_buffer,
                surface_w,
                surface_h,
                stride,
            );
            let Some(buf) = buf else {
                warn!("desktop menu buffer unavailable: reason={:?}", reason);
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.desktop_menu_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!("desktop menu canvas unavailable after retry");
                return;
            };
            crate::popup_card::paint_card_panels_with_shadow(
                canvas,
                surface_w,
                surface_h,
                card_w,
                card_h,
                &card_buf,
                &panels,
                crate::ui::tokens::surface_radius_from_config(
                    &self.theme,
                    meridian_config::ThemeSurface::Popup,
                ),
            );
            if let Err(err) = buf.attach_to(self.desktop_menu_layer.wl_surface()) {
                warn!("desktop menu buffer attach failed: {}", err);
                return;
            }
            self.desktop_menu_layer.wl_surface().damage_buffer(
                0,
                0,
                surface_w as i32,
                surface_h as i32,
            );
            self.desktop_menu_layer.commit();
            return;
        }
    }

    pub(crate) fn unmap_desktop_menu(&mut self, _reason: CommitReason) {
        // Re-assert a valid, anchored, non-zero size *before* committing the
        // null buffer. Hiding the menu often coincides with a re-arrange (e.g.
        // the "Launcher öffnen" item opens the fullscreen launcher), and during
        // that arrange smithay can see this single-anchored (TOP|LEFT) surface
        // with a 0 width and post a protocol error that tears down the whole
        // shell. Keeping anchor+size valid on the unmap commit prevents it.
        // Also reset to base dimensions so any spurious configure that fires
        // after unmap does not re-assert an old expanded (submenu-open) width.
        use smithay_client_toolkit::shell::wlr_layer::Anchor;
        let base_card_w = crate::context_menu::MENU_WIDTH as u32;
        let base_card_h =
            crate::context_menu::menu_height(crate::context_menu::desktop_item_list().len()).max(1)
                as u32;
        let base_w = crate::popup_surface_w(base_card_w);
        let base_h = crate::popup_surface_h(base_card_h);
        self.desktop_menu_width = base_w;
        self.desktop_menu_height = base_h;
        self.desktop_menu_layer
            .set_anchor(Anchor::TOP | Anchor::LEFT);
        self.desktop_menu_layer.set_size(base_w, base_h);
        self.desktop_menu_layer.wl_surface().attach(None, 0, 0);
        self.desktop_menu_layer.commit();
    }

    pub(crate) fn draw_consent_modal(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        if !self.consent_open || !self.consent_configured {
            return;
        }
        let width = crate::screenshot_consent::MODAL_WIDTH as u32;
        let height = crate::screenshot_consent::MODAL_HEIGHT as u32;
        let stride = buffer::shm_buffer_stride(width);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.consent_buffer,
                width,
                height,
                stride,
            );
            let Some(buf) = buf else {
                warn!("consent buffer unavailable: reason={:?}", reason);
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.consent_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!("consent canvas unavailable after retry");
                return;
            };
            canvas.fill(0);
            crate::screenshot_consent::draw_consent_overlay(
                canvas,
                width,
                height,
                &self.consent_app_id,
                self.consent_hover,
                &self.theme,
            );
            if let Err(err) = buf.attach_to(self.consent_layer.wl_surface()) {
                warn!("consent buffer attach failed: {}", err);
                return;
            }
            self.consent_layer
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);
            self.consent_layer.commit();
            return;
        }
    }

    pub(crate) fn draw_region_picker(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        if !self.region_picker_open || !self.region_picker_configured {
            return;
        }
        let width = self.region_picker_width.max(1);
        let height = self.region_picker_height.max(1);
        let stride = buffer::shm_buffer_stride(width);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.region_picker_buffer,
                width,
                height,
                stride,
            );
            let Some(buf) = buf else {
                warn!("region picker buffer unavailable: reason={:?}", reason);
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.region_picker_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!("region picker canvas unavailable after retry");
                return;
            };
            canvas.fill(0);
            let selection = if let (Some(start), Some(current)) = (
                self.region_picker_drag_start,
                self.region_picker_drag_current,
            ) {
                crate::region_picker::rect_from_drag(start, current, width, height)
            } else {
                self.region_picker_pending
            };
            crate::region_picker::draw_region_picker_overlay(
                canvas,
                width,
                height,
                selection,
                &self.theme,
            );
            if let Err(err) = buf.attach_to(self.region_picker_layer.wl_surface()) {
                warn!("region picker buffer attach failed: {}", err);
                return;
            }
            self.region_picker_layer
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);
            self.region_picker_layer.commit();
            return;
        }
    }

    pub(crate) fn unmap_region_picker(&mut self, _reason: CommitReason) {
        self.region_picker_layer.wl_surface().attach(None, 0, 0);
        self.region_picker_layer.commit();
    }

    pub(crate) fn unmap_consent(&mut self, _reason: CommitReason) {
        // Keep a valid anchored size on the unmap commit (same protocol-safety
        // reasoning as unmap_desktop_menu).
        self.consent_layer.set_size(
            crate::screenshot_consent::MODAL_WIDTH as u32,
            crate::screenshot_consent::MODAL_HEIGHT as u32,
        );
        self.consent_layer.wl_surface().attach(None, 0, 0);
        self.consent_layer.commit();
    }

    pub(crate) fn draw_wifi_modal(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        if !self.wifi_modal_open || !self.wifi_modal_configured {
            return;
        }
        let width = crate::wifi_password_modal::MODAL_WIDTH as u32;
        let height = crate::wifi_password_modal::MODAL_HEIGHT as u32;
        let ssid = self.wifi_password_prompt.clone().unwrap_or_default();
        let pw_len = self.wifi_password_input.chars().count();
        let stride = buffer::shm_buffer_stride(width);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.wifi_modal_buffer,
                width,
                height,
                stride,
            );
            let Some(buf) = buf else {
                warn!("wifi modal buffer unavailable: reason={:?}", reason);
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.wifi_modal_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!("wifi modal canvas unavailable after retry");
                return;
            };
            canvas.fill(0);
            crate::wifi_password_modal::draw_wifi_password_modal(
                canvas,
                width,
                height,
                &ssid,
                pw_len,
                self.wifi_modal_hover,
                &self.theme,
            );
            if let Err(err) = buf.attach_to(self.wifi_modal_layer.wl_surface()) {
                warn!("wifi modal buffer attach failed: {}", err);
                return;
            }
            self.wifi_modal_layer
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);
            self.wifi_modal_layer.commit();
            return;
        }
    }

    pub(crate) fn unmap_wifi_modal(&mut self, _reason: CommitReason) {
        // Keep a valid anchored size on the unmap commit (same protocol-safety
        // reasoning as unmap_consent).
        self.wifi_modal_layer.set_size(
            crate::wifi_password_modal::MODAL_WIDTH as u32,
            crate::wifi_password_modal::MODAL_HEIGHT as u32,
        );
        self.wifi_modal_layer.wl_surface().attach(None, 0, 0);
        self.wifi_modal_layer.commit();
    }
}
