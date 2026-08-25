impl MeridianShell {
    pub(crate) fn close_network_popup(&mut self, reason: CommitReason) -> bool {
        if !self.network_popup_open {
            return false;
        }
        self.network_popup_open = false;
        self.quick_settings_volume_pending = None;
        self.network_layer
            .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        self.unmap_network_popup(reason);
        tracing::debug!(
            "close_network_popup: open_after={} configured={} keyboard_focus={:?}",
            self.network_popup_open,
            self.network_configured,
            self.keyboard_focus
        );
        true
    }

    /// Update only the in-memory value while dragging. Rendering is immediate;
    /// the comparatively expensive platform mixer command runs once on release.
    pub(crate) fn preview_quick_settings_volume(
        &mut self,
        qh: &QueueHandle<Self>,
        percent: u8,
    ) {
        let percent = percent.min(100);
        self.quick_settings_volume_pending = Some(percent);
        if let Some(device) = self.audio_snapshot.default_output.as_mut() {
            device.volume_percent = Some(percent);
        }
        self.draw_network_popup(qh, RepaintReason::Pointer);
        self.draw_panel(qh, RepaintReason::Pointer);
    }

    pub(crate) fn commit_quick_settings_volume(&mut self) {
        if let Some(percent) = self.quick_settings_volume_pending.take() {
            crate::audio::set_default_sink_volume(percent);
        }
    }

    pub(super) fn toggle_audio_popup(&mut self, reason: CommitReason) {
        if self.audio_popup_open {
            self.close_audio_popup(reason);
            return;
        }
        if self.volume_osd_open {
            self.close_volume_osd(reason);
        }

        if self.launcher_state.open {
            self.launcher_state.close();
            self.launcher_layer
                .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
            self.unmap_launcher(reason);
        }
        if self.calendar_popup_open {
            self.close_calendar_popup(reason);
        }
        if self.workspace_popup_open {
            self.close_workspace_popup(reason);
        }
        if self.network_popup_open {
            self.close_network_popup(reason);
        }

        self.audio_snapshot = crate::audio::AudioSnapshot::poll();
        self.audio_popup_open = true;
        self.network_layer
            .set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
        self.network_layer.set_margin(
            0,
            crate::AUDIO_POPUP_RIGHT_MARGIN,
            crate::SHELL_POPUP_BOTTOM_MARGIN,
            0,
        );
        self.network_layer.set_exclusive_zone(0);
        self.network_layer.set_size(
            crate::popup_surface_w(crate::AUDIO_POPUP_WIDTH),
            crate::popup_surface_h(crate::AUDIO_POPUP_HEIGHT),
        );
        self.network_layer
            .set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        self.audio_width = crate::popup_surface_w(crate::AUDIO_POPUP_WIDTH);
        self.audio_height = crate::popup_surface_h(crate::AUDIO_POPUP_HEIGHT);
        self.audio_dirty = true;
        if !self.network_configured {
            self.network_layer.commit();
        }
        tracing::debug!(
            "toggle_audio_popup: open_after={} configured={} size={}x{} keyboard_focus={:?}",
            self.audio_popup_open,
            self.network_configured,
            self.audio_width,
            self.audio_height,
            self.keyboard_focus
        );
    }

    pub(crate) fn close_audio_popup(&mut self, reason: CommitReason) -> bool {
        if !self.audio_popup_open {
            return false;
        }
        self.audio_popup_open = false;
        self.audio_volume_dragging = false;
        self.network_layer
            .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        self.unmap_audio_popup(reason);
        tracing::debug!(
            "close_audio_popup: open_after={} configured={} keyboard_focus={:?}",
            self.audio_popup_open,
            self.network_configured,
            self.keyboard_focus
        );
        true
    }

    /// Show (or refresh) the compact volume OSD: centred above the panel,
    /// auto-hiding after VOLUME_OSD_VISIBLE_MS. Shares the network_layer surface,
    /// so any network/audio popup is closed first. Never grabs keyboard focus.
    pub(crate) fn show_volume_osd(&mut self, qh: &QueueHandle<Self>) {
        if self.audio_popup_open {
            self.close_audio_popup(CommitReason::Input);
        }
        if self.network_popup_open {
            self.close_network_popup(CommitReason::Input);
        }
        self.audio_snapshot = crate::audio::AudioSnapshot::poll();
        self.osd_power_profile = None;
        self.volume_osd_hide_at = Some(
            std::time::Instant::now()
                + std::time::Duration::from_millis(crate::VOLUME_OSD_VISIBLE_MS),
        );
        if self.volume_osd_open {
            // Already mapped: just refresh the level and the timer.
            self.draw_volume_osd(qh, RepaintReason::Pointer);
            return;
        }
        self.volume_osd_open = true;
        self.network_layer.set_anchor(Anchor::BOTTOM);
        self.network_layer
            .set_margin(0, 0, crate::VOLUME_OSD_BOTTOM_MARGIN, 0);
        self.network_layer.set_exclusive_zone(0);
        self.network_layer.set_size(
            crate::popup_surface_w(crate::VOLUME_OSD_WIDTH),
            crate::popup_surface_h(crate::VOLUME_OSD_HEIGHT),
        );
        self.network_layer
            .set_keyboard_interactivity(KeyboardInteractivity::None);
        self.volume_osd_width = crate::popup_surface_w(crate::VOLUME_OSD_WIDTH);
        self.volume_osd_height = crate::popup_surface_h(crate::VOLUME_OSD_HEIGHT);
        if !self.network_configured {
            self.network_layer.commit();
        }
        self.draw_volume_osd(qh, RepaintReason::Pointer);
    }

    /// Show the OSD with a power-profile name (Eco/Standard/Full). Reuses the
    /// volume OSD surface + auto-hide timer; the renderer picks text vs volume
    /// based on `osd_power_profile`.
    pub(crate) fn show_power_profile_osd(&mut self, qh: &QueueHandle<Self>, label: String) {
        if self.audio_popup_open {
            self.close_audio_popup(CommitReason::Input);
        }
        if self.network_popup_open {
            self.close_network_popup(CommitReason::Input);
        }
        self.osd_power_profile = Some(label);
        self.volume_osd_hide_at = Some(
            std::time::Instant::now()
                + std::time::Duration::from_millis(crate::VOLUME_OSD_VISIBLE_MS),
        );
        if self.volume_osd_open {
            self.draw_volume_osd(qh, RepaintReason::Pointer);
            return;
        }
        self.volume_osd_open = true;
        self.network_layer.set_anchor(Anchor::BOTTOM);
        self.network_layer
            .set_margin(0, 0, crate::VOLUME_OSD_BOTTOM_MARGIN, 0);
        self.network_layer.set_exclusive_zone(0);
        self.network_layer.set_size(
            crate::popup_surface_w(crate::VOLUME_OSD_WIDTH),
            crate::popup_surface_h(crate::VOLUME_OSD_HEIGHT),
        );
        self.network_layer
            .set_keyboard_interactivity(KeyboardInteractivity::None);
        self.volume_osd_width = crate::popup_surface_w(crate::VOLUME_OSD_WIDTH);
        self.volume_osd_height = crate::popup_surface_h(crate::VOLUME_OSD_HEIGHT);
        if !self.network_configured {
            self.network_layer.commit();
        }
        self.draw_volume_osd(qh, RepaintReason::Pointer);
    }

    pub(crate) fn close_volume_osd(&mut self, reason: CommitReason) {
        if !self.volume_osd_open {
            return;
        }
        self.volume_osd_open = false;
        self.osd_power_profile = None;
        self.volume_osd_hide_at = None;
        self.audio_volume_dragging = false;
        self.network_layer.wl_surface().attach(None, 0, 0);
        self.network_layer.commit();
        self.network_configured = false;
        let _ = reason;
    }

    /// Apply a volume picked by dragging the OSD slider: set the mixer, update
    /// the snapshot optimistically, reset the auto-hide timer, and redraw.
    pub(crate) fn apply_osd_volume(&mut self, qh: &QueueHandle<Self>, percent: u8) {
        crate::audio::set_default_sink_volume(percent);
        if let Some(device) = self.audio_snapshot.default_output.as_mut() {
            device.volume_percent = Some(percent);
        }
        self.volume_osd_hide_at = Some(
            std::time::Instant::now()
                + std::time::Duration::from_millis(crate::VOLUME_OSD_VISIBLE_MS),
        );
        self.draw_volume_osd(qh, RepaintReason::Pointer);
        self.draw_panel(qh, RepaintReason::Pointer);
    }

    pub(crate) fn open_status_notifier_menu(
        &mut self,
        qh: &QueueHandle<Self>,
        menu_state: status_notifier::StatusNotifierMenuState,
    ) {
        if self.launcher_state.open {
            self.launcher_state.close();
            self.launcher_layer
                .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
            self.unmap_launcher(CommitReason::Input);
        }
        if self.calendar_popup_open {
            self.close_calendar_popup(CommitReason::Input);
        }
        if self.workspace_popup_open {
            self.close_workspace_popup(CommitReason::Input);
        }
        if self.network_popup_open {
            self.close_network_popup(CommitReason::Input);
        }
        if self.audio_popup_open {
            self.close_audio_popup(CommitReason::Input);
        }

        let entries = menu_state.menu.display_entries();
        let height = status_notifier_popup::menu_height(entries.len());
        self.status_notifier_menu_width =
            crate::popup_surface_w(status_notifier_popup::SNI_MENU_WIDTH);
        self.status_notifier_menu_height = crate::popup_surface_h(height);
        self.status_notifier_menu_entries = entries;
        self.status_notifier_menu = Some(menu_state);
        self.status_notifier_menu_open = true;
        self.network_layer
            .set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
        self.network_layer.set_margin(
            0,
            crate::SNI_MENU_RIGHT_MARGIN,
            crate::SHELL_POPUP_BOTTOM_MARGIN,
            0,
        );
        self.network_layer.set_exclusive_zone(0);
        self.network_layer.set_size(
            self.status_notifier_menu_width,
            self.status_notifier_menu_height,
        );
        self.network_layer
            .set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        self.network_dirty = true;
        if !self.network_configured {
            self.network_layer.commit();
        }
        self.draw_status_notifier_menu(qh, RepaintReason::Ipc);
    }
}
