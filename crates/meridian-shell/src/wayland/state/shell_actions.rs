impl MeridianShell {
    pub(crate) fn close_status_notifier_menu(&mut self, reason: CommitReason) -> bool {
        if !self.status_notifier_menu_open {
            return false;
        }
        self.status_notifier_menu_open = false;
        self.status_notifier_menu = None;
        self.status_notifier_menu_entries.clear();
        self.network_layer
            .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        self.unmap_status_notifier_menu(reason);
        true
    }

    pub(crate) fn open_thumbnail_popup(
        &mut self,
        qh: &QueueHandle<Self>,
        window_ids: &[String],
        icon_center: Option<i32>,
    ) {
        self.thumbnail_popup_window_ids = window_ids
            .iter()
            .take(crate::THUMBNAIL_MAX_WINDOWS)
            .cloned()
            .collect();
        self.thumbnail_icon_center = icon_center;

        // Always request fresh thumbnails — cached data may reflect a previous
        // window state (size change, content scroll, etc.). Stale cache shows
        // the user an outdated preview which looks like a crop bug.
        for id in window_ids.iter().take(crate::THUMBNAIL_MAX_WINDOWS) {
            let cmd = meridian_ipc::ShellCommand::CaptureWindowThumbnail {
                id: id.clone(),
                max_width: crate::THUMBNAIL_THUMB_W,
                max_height: crate::THUMBNAIL_THUMB_H,
            };
            let _ = self.ipc.send(&cmd);
        }

        let popup_w = crate::thumbnail_popup::popup_width_for(
            &self.thumbnail_cache,
            &self.thumbnail_popup_window_ids,
        );
        let left_margin = icon_center
            .map(|c| (c - popup_w as i32 / 2).max(0))
            .unwrap_or(0);

        self.thumbnail_popup_open = true;
        self.thumbnail_layer.set_anchor(
            smithay_client_toolkit::shell::wlr_layer::Anchor::BOTTOM
                | smithay_client_toolkit::shell::wlr_layer::Anchor::LEFT,
        );
        self.thumbnail_layer
            .set_margin(0, 0, crate::SHELL_POPUP_BOTTOM_MARGIN, left_margin);
        self.thumbnail_layer.set_exclusive_zone(0);
        self.thumbnail_layer.set_size(
            crate::popup_surface_w(popup_w),
            crate::popup_surface_h(crate::THUMBNAIL_POPUP_HEIGHT),
        );
        self.thumbnail_layer.set_keyboard_interactivity(
            smithay_client_toolkit::shell::wlr_layer::KeyboardInteractivity::None,
        );
        self.thumbnail_width = crate::popup_surface_w(popup_w);
        self.thumbnail_height = crate::popup_surface_h(crate::THUMBNAIL_POPUP_HEIGHT);
        self.thumbnail_dirty = true;
        self.draw_thumbnail_popup(qh, crate::wayland::RepaintReason::Pointer);
    }

    /// Recompute popup width from current cache. If size changed, request a
    /// resize from the compositor (which will fire a configure event that
    /// triggers redraw with the actually-granted size). Otherwise just redraw.
    pub(crate) fn refresh_thumbnail_popup(&mut self, qh: &QueueHandle<Self>) {
        if !self.thumbnail_popup_open {
            return;
        }
        let new_w = crate::thumbnail_popup::popup_width_for(
            &self.thumbnail_cache,
            &self.thumbnail_popup_window_ids,
        );
        let new_surface_w = crate::popup_surface_w(new_w);
        if new_surface_w != self.thumbnail_width {
            let left_margin = self
                .thumbnail_icon_center
                .map(|c| (c - new_w as i32 / 2).max(0))
                .unwrap_or(0);
            self.thumbnail_layer
                .set_margin(0, 0, crate::SHELL_POPUP_BOTTOM_MARGIN, left_margin);
            self.thumbnail_layer.set_size(
                crate::popup_surface_w(new_w),
                crate::popup_surface_h(crate::THUMBNAIL_POPUP_HEIGHT),
            );
            self.thumbnail_width = new_surface_w;
        }
        self.draw_thumbnail_popup(qh, crate::wayland::RepaintReason::Ipc);
    }

    pub(crate) fn close_thumbnail_popup(&mut self, reason: crate::wayland::CommitReason) {
        if !self.thumbnail_popup_open {
            return;
        }
        self.thumbnail_popup_open = false;
        self.thumbnail_popup_window_ids.clear();
        self.unmap_thumbnail_popup(reason);
    }

    /// Returns true (and clears the armed state) if `id` matches the
    /// currently-armed power button and it is still within the timeout
    /// window. Caller should then execute the destructive action.
    pub(crate) fn try_consume_armed_power(&mut self, id: &str) -> bool {
        let armed_now = self
            .armed_power
            .as_ref()
            .map(|(armed_id, t)| {
                armed_id == id && t.elapsed().as_millis() < crate::POWER_ARM_TIMEOUT_MS
            })
            .unwrap_or(false);
        if armed_now {
            self.armed_power = None;
        }
        armed_now
    }

    /// Arm a power button (1st click of a confirm-twice action). Replaces any
    /// previously-armed button and triggers a launcher repaint so the user
    /// sees the countdown ring start filling.
    pub(crate) fn arm_power(&mut self, qh: &QueueHandle<Self>, id: &str) {
        self.armed_power = Some((id.to_string(), std::time::Instant::now()));
        self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
    }

    pub(crate) fn close_launcher_after_launch(
        &mut self,
        qh: &QueueHandle<Self>,
        reason: RepaintReason,
    ) {
        if !self.launcher_state.open {
            return;
        }
        self.launcher_state.close();
        self.launcher_settings_open = false;
        self.settings_pinned_adding = false;
        self.launcher_selected_idx = None;
        self.search_query.clear();
        self.app_view_scroll_y = 0;
        self.hovered_bento_idx = None;
        self.hovered_app_card_idx = None;
        self.launcher_layer
            .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        self.unmap_launcher(CommitReason::Input);
        self.draw_panel(qh, reason);
    }

    pub(crate) fn apply_wallpaper(
        &mut self,
        qh: &QueueHandle<Self>,
        path: String,
        mode: meridian_config::WallpaperMode,
    ) {
        meridian_config::MeridianConfig::save_wallpaper(&path, mode);
        self.wallpaper_path = Some(path);
        self.wallpaper_mode = mode;
        self.ipc.send(&meridian_ipc::ShellCommand::ReloadConfig);
        tracing::info!(
            "Wallpaper applied: path={:?} mode={:?}",
            self.wallpaper_path,
            mode
        );
        if self.launcher_settings_open {
            self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
        }
    }

    pub(crate) fn save_pinned_apps(&self) {
        let configs: Vec<meridian_config::PinnedAppConfig> = self
            .pinned_apps
            .iter()
            .map(|a| meridian_config::PinnedAppConfig {
                label: a.label.clone(),
                program: a.program.clone(),
                icon: a.icon_name.clone(),
            })
            .collect();
        meridian_config::MeridianConfig::save_pinned_apps(&configs);
    }

    pub(crate) fn save_hidden_apps(&self) {
        let dir = hidden_apps_path();
        if let Some(parent) = std::path::Path::new(&dir).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let content: String = self
            .hidden_execs
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = std::fs::write(&dir, content);
    }

    pub(crate) fn spawn_file_picker(&mut self) {
        if self.wallpaper_picker_rx.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.wallpaper_picker_rx = Some(rx);
        let wayland_display =
            std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-1".into());
        std::thread::spawn(move || {
            let picker = wallpaper_picker_command();
            let out = std::process::Command::new(picker.program)
                .args(picker.args)
                .env("WAYLAND_DISPLAY", &wayland_display)
                .output();
            match out {
                Ok(o) if o.status.success() => {
                    let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if !path.is_empty() {
                        let _ = tx.send(path);
                    }
                }
                Ok(o) => tracing::warn!(
                    "wallpaper picker exited {:?}: {}",
                    o.status,
                    String::from_utf8_lossy(&o.stderr).trim()
                ),
                Err(e) => tracing::warn!("wallpaper picker spawn failed: {}", e),
            }
        });
    }

    pub(crate) fn poll_wallpaper_picker(&mut self) -> Option<String> {
        let rx = self.wallpaper_picker_rx.as_ref()?;
        match rx.try_recv() {
            Ok(path) => {
                self.wallpaper_picker_rx = None;
                Some(path)
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.wallpaper_picker_rx = None;
                None
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
        }
    }
    pub(crate) fn apply_theme(&mut self, qh: &QueueHandle<Self>, name: String) {
        let mut theme_manager = meridian_config::ThemeManager::new();
        if let Err(e) = theme_manager.set_theme(&name) {
            tracing::warn!("apply_theme: failed to load {:?}: {}", name, e);
            return;
        }
        self.theme = theme_manager.current().config.clone();
        self.theme_name = name.clone();
        self.available_themes = theme_manager.available_themes();
        self.icon_cache = super::init::assets::build_icon_cache(&self.theme, &self.pinned_apps);
        crate::panel_view::warm_status_notifier_icons(
            &mut self.icon_cache,
            &self.status_notifier_items,
        );
        self.launcher_icons_warmed = false;
        meridian_config::MeridianConfig::save_theme(&name);
        // THEME-1: KDE/Qt (Breeze/KColorScheme) and GTK apps read legacy config
        // files (kdeglobals / gtk settings.ini / gsettings), not the appearance
        // portal. Re-export them so a live theme switch reaches those apps too.
        // (Already-running KColorScheme apps only re-read kdeglobals at startup,
        // so this mainly affects apps launched after the switch.)
        crate::theme_export::export_theme(&self.theme);
        self.ipc.send(&ShellCommand::ReloadConfig);
        tracing::info!("Theme applied: {}", name);
        self.panel_dirty = true;
        self.launcher_dirty = true;
        self.calendar_dirty = true;
        self.workspace_dirty = true;
        self.network_dirty = true;
        self.audio_dirty = true;
        self.thumbnail_dirty |= self.thumbnail_popup_open;
        self.draw_panel(qh, crate::wayland::RepaintReason::Pointer);
        if self.launcher_state.open {
            self.draw_launcher(qh, crate::wayland::RepaintReason::Pointer);
        }
    }
}
