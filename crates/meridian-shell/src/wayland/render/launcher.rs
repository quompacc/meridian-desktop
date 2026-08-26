impl MeridianShell {
    pub(crate) fn draw_launcher(&mut self, _qh: &QueueHandle<Self>, reason: RepaintReason) {
        debug!(
            "draw_launcher: reason={:?} open={} configured={} launcher_dirty={} commit_expected={}",
            reason,
            self.launcher_state.open,
            self.launcher_configured,
            self.launcher_dirty,
            self.launcher_configured && self.launcher_state.open
        );
        if !self.launcher_configured || !self.launcher_state.open {
            // info! (temp) — diagnosing "launcher won't open": a skip with
            // open=true configured=false means the layer configure never arrived.
            info!(
                "draw_launcher skipped: reason={:?} open={} configured={}",
                reason, self.launcher_state.open, self.launcher_configured
            );
            return;
        }
        self.repaint_stats.record_launcher(reason);

        let width = if self.launcher_is_fullscreen {
            self.launcher_width
        } else {
            LAUNCHER_WIDTH
        };
        let height = if self.launcher_is_fullscreen {
            self.launcher_height
        } else {
            LAUNCHER_HEIGHT
        };
        debug!(
            "draw_launcher size: configured={}x{} effective={}x{} desired={}x{}",
            self.launcher_width,
            self.launcher_height,
            width,
            height,
            LAUNCHER_WIDTH,
            LAUNCHER_HEIGHT
        );
        self.render_stats.launcher.renders += 1;

        let stride = buffer::shm_buffer_stride(width);
        for attempt in 0..CANVAS_RETRY_ATTEMPTS {
            let buf = buffer::buffer_for(
                &mut self.pool,
                &mut self.launcher_buffer,
                width,
                height,
                stride,
            );
            let Some(buf) = buf else {
                warn!(
                    "launcher buffer unavailable: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };
            let Some(canvas) = buf.canvas(&mut self.pool) else {
                self.launcher_buffer = None;
                if attempt + 1 < CANVAS_RETRY_ATTEMPTS {
                    continue;
                }
                warn!(
                    "launcher canvas unavailable after retry: reason={:?} width={} height={}",
                    reason, width, height
                );
                return;
            };

            let active = self.ui_preview_widget_state.as_ref();
            let state_fn = |path: &[usize]| -> meridian_ui::WidgetState {
                match active {
                    Some((p, s)) if p.as_slice() == path => *s,
                    _ => meridian_ui::WidgetState::Idle,
                }
            };

            // Render launcher content to a fixed-size buffer.
            let lw = LAUNCHER_WIDTH as usize;
            let lh = LAUNCHER_HEIGHT as usize;
            let mut content = vec![0u8; lw * lh * 4];
            let armed_power: Option<(&str, f32)> = self.armed_power.as_ref().map(|(id, at)| {
                let p = (at.elapsed().as_millis() as f32 / crate::POWER_ARM_TIMEOUT_MS as f32)
                    .clamp(0.0, 1.0);
                (id.as_str(), p)
            });
            if self.launcher_settings_open {
                let system_info = crate::sysinfo::SystemInfo::gather();
                crate::settings_view::draw_settings_launcher(
                    &mut content,
                    LAUNCHER_WIDTH,
                    LAUNCHER_HEIGHT,
                    self.settings_category,
                    &self.settings_search,
                    &self.available_themes,
                    &self.theme_name,
                    &self.available_wallpapers,
                    &self.wallpaper_thumbnails,
                    self.wallpaper_path.as_deref(),
                    self.wallpaper_mode,
                    self.cursor_size,
                    self.available_cursor_themes.as_slice(),
                    self.cursor_theme.as_str(),
                    self.idle_timeout_secs,
                    &self.pinned_apps,
                    &self.output_workspaces,
                    self.display_mode_dropdown_open,
                    &self.printer_snapshot,
                    &self.audio_snapshot,
                    &system_info,
                    self.network_controller.state(),
                    self.network_profiles.as_slice(),
                    &self.bluetooth_snapshot,
                    self.wifi_networks.as_slice(),
                    self.settings_pinned_adding,
                    &self.launcher_state.apps,
                    &self.icon_cache,
                    armed_power,
                    self.default_apps_index.as_ref(),
                    &self.default_apps_current,
                    self.default_apps_picker_open,
                    &self.theme,
                    &state_fn,
                );
            } else {
                crate::app_view::draw_command_palette(
                    &mut content,
                    LAUNCHER_WIDTH,
                    LAUNCHER_HEIGHT,
                    &self.pinned_apps,
                    &self.launcher_state.apps,
                    self.launcher_state.category,
                    &self.search_query,
                    self.app_view_scroll_y,
                    self.launcher_selected_idx,
                    armed_power,
                    &self.icon_cache,
                    &self.hidden_execs,
                    self.hovered_app_card_idx,
                    self.hovered_bento_idx,
                    self.settings_hovered,
                    self.hovered_power_btn,
                    &self.theme,
                );
            }
            if let Some(ref cm) = self.context_menu {
                let items = crate::context_menu::item_list(
                    cm.is_terminal,
                    cm.is_pinned,
                    cm.running_window_id.is_some(),
                );
                crate::context_menu::draw_overlay(
                    &mut content,
                    LAUNCHER_WIDTH,
                    LAUNCHER_HEIGHT,
                    cm,
                    &items,
                    &[],
                    &[],
                    &self.theme,
                );
            }

            let launcher_radius = crate::ui::tokens::surface_radius_from_config(
                &self.theme,
                meridian_config::ThemeSurface::Launcher,
            );
            round_buffer_corners(&mut content, lw, lh, launcher_radius);

            if self.launcher_is_fullscreen {
                // Blit LAUNCHER_WxH content into the full-screen canvas at visual offset.
                let fw = width as usize;
                let vx = self.launcher_visual_x.max(0) as usize;
                let vy = self.launcher_visual_y.max(0) as usize;
                canvas.fill(0);
                // Soft drop shadow around the rounded glass launcher. Keep the
                // card interior clear so its transparent body cannot reveal a
                // dark shadow veil over the compositor-owned glass backdrop.
                crate::soft_shadow::draw_soft_shadow(
                    canvas,
                    width as i32,
                    height as i32,
                    vx as i32,
                    vy as i32,
                    lw as i32,
                    lh as i32,
                    launcher_radius as f32,
                    meridian_tokens::Elevation::LAUNCHER.blur,
                    meridian_tokens::Elevation::LAUNCHER.alpha,
                    meridian_tokens::Elevation::LAUNCHER.offset_y,
                    true,
                );
                // Composite the (premultiplied) content over the shadow so the
                // transparent rounded corners reveal the shadow underneath.
                for y in 0..lh {
                    let src = &content[y * lw * 4..(y + 1) * lw * 4];
                    let dst_off = (vy + y) * fw * 4 + vx * 4;
                    if dst_off + lw * 4 > canvas.len() {
                        continue;
                    }
                    let dst = &mut canvas[dst_off..dst_off + lw * 4];
                    for x in 0..lw {
                        let s = &src[x * 4..x * 4 + 4];
                        let sa = s[3] as u32;
                        if sa == 255 {
                            dst[x * 4..x * 4 + 4].copy_from_slice(s);
                        } else if sa != 0 {
                            let inv = 255 - sa;
                            for k in 0..4 {
                                dst[x * 4 + k] =
                                    (s[k] as u32 + dst[x * 4 + k] as u32 * inv / 255) as u8;
                            }
                        }
                    }
                }
            } else {
                canvas[..lw * lh * 4].copy_from_slice(&content);
            }

            self.launcher_layer
                .wl_surface()
                .damage_buffer(0, 0, width as i32, height as i32);
            if let Err(err) = buf.attach_to(self.launcher_layer.wl_surface()) {
                warn!(
                    "launcher buffer attach failed: reason={:?} width={} height={} error={}",
                    reason, width, height, err
                );
                return;
            }
            self.commit_surface(
                CommitSurfaceKind::Launcher,
                Self::commit_reason_from_repaint(reason, false),
            );
            debug!(
                "draw_launcher committed: reason={:?} width={} height={}",
                reason, width, height
            );
            self.launcher_dirty = false;
            return;
        }
    }

    pub(crate) fn unmap_launcher(&mut self, reason: CommitReason) {
        debug!(
            "unmap_launcher: reason={:?} open={} configured={} surface=launcher attach_none=true commit=true",
            reason,
            self.launcher_state.open,
            self.launcher_configured
        );
        self.launcher_layer.wl_surface().attach(None, 0, 0);
        self.commit_surface(CommitSurfaceKind::Launcher, reason);
        self.launcher_dirty = false;
    }
}
