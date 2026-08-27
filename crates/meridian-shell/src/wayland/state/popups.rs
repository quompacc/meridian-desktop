impl MeridianShell {
    pub(crate) fn open_system_settings_from_ipc(&mut self) {
        if !self.launcher_state.open {
            self.toggle_launcher();
        }
        self.launcher_settings_open = true;
        self.settings_category = crate::settings_view::SettingsCategory::SystemOverview;
        self.request_settings_refresh(crate::settings_view::SettingsCategory::SystemOverview);
        self.launcher_dirty = true;
        self.panel_dirty = true;
    }

    fn toggle_launcher(&mut self) {
        let open_before = self.launcher_state.open;
        if !open_before && self.calendar_popup_open {
            self.close_calendar_popup(CommitReason::Input);
        }
        if !open_before && self.workspace_popup_open {
            self.close_workspace_popup(CommitReason::Input);
        }
        if !open_before && self.network_popup_open {
            self.close_network_popup(CommitReason::Input);
        }
        if !open_before && self.audio_popup_open {
            self.close_audio_popup(CommitReason::Input);
        }
        self.launcher_state.toggle();
        let open_after = self.launcher_state.open;
        if self.launcher_state.open {
            // Warm the grid app-icons on first open (deferred from startup).
            self.warm_launcher_icons();
            // Refresh the app list off-thread so newly-installed apps show up
            // without ever blocking the open (LAUNCH-2). The cached list renders
            // now; the fresh one swaps in within a tick.
            self.request_launcher_apps_refresh();
            // Fullscreen layer surface, transparent except for the launcher card.
            // This gives the software shadow room to render around the card
            // without moving the visible launcher away from the panel.
            self.launcher_layer
                .set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
            self.launcher_layer.set_margin(0, 0, 0, 0);
            self.launcher_layer.set_exclusive_zone(0);
            self.launcher_layer.set_size(0, 0);
            self.launcher_layer
                .set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
            tracing::debug!(
                "launcher focus request: keyboard_interactivity=Exclusive (fullscreen)"
            );
            self.launcher_is_fullscreen = true;
            self.launcher_configured = false;
            self.commit_surface(CommitSurfaceKind::Launcher, CommitReason::Input);
            self.launcher_state.reshuffle();
        } else {
            self.launcher_is_fullscreen = false;
            self.launcher_settings_open = false;
            self.settings_category = crate::settings_view::SettingsCategory::default();
            self.launcher_layer
                .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
            tracing::debug!("launcher focus release: keyboard_interactivity=OnDemand");
        }
        // Force a panel re-commit on launcher toggle to avoid stale/missing panel attachment.
        self.panel_last_signature = None;
        self.launcher_dirty = true;
        self.panel_dirty = true;
        // Temporarily promoted to info! to diagnose the "launcher won't open"
        // report on real hardware (debug logging is off in prod). Revert once fixed.
        tracing::info!(
            "toggle_launcher: open_before={} open_after={} panel_configured={} launcher_configured={} launcher_size={}x{} panel_dirty={} launcher_dirty={} keyboard_focus={:?}",
            open_before,
            open_after,
            self.panel_configured,
            self.launcher_configured,
            self.launcher_width,
            self.launcher_height,
            self.panel_dirty,
            self.launcher_dirty,
            self.keyboard_focus
        );
    }

    pub(crate) fn open_settings_category(
        &mut self,
        qh: &QueueHandle<Self>,
        category: crate::settings_view::SettingsCategory,
    ) {
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
        if !self.launcher_state.open {
            self.toggle_launcher();
        }
        self.launcher_settings_open = true;
        self.settings_category = category;
        self.display_mode_dropdown_open = None;
        self.request_settings_refresh(category);
        self.launcher_dirty = true;
        self.panel_dirty = true;
        self.draw_panel(qh, RepaintReason::Pointer);
        self.draw_launcher(qh, RepaintReason::Pointer);
    }

    fn open_sound_settings_from_tray(&mut self, reason: CommitReason) {
        if self.calendar_popup_open {
            self.close_calendar_popup(reason);
        }
        if self.workspace_popup_open {
            self.close_workspace_popup(reason);
        }
        if self.network_popup_open {
            self.close_network_popup(reason);
        }
        if self.audio_popup_open {
            self.close_audio_popup(reason);
        }
        if !self.launcher_state.open {
            self.toggle_launcher();
        }
        self.launcher_settings_open = true;
        self.settings_category = crate::settings_view::SettingsCategory::Sound;
        self.request_settings_refresh(crate::settings_view::SettingsCategory::Sound);
        self.launcher_dirty = true;
        self.panel_dirty = true;
    }

    fn open_network_settings_from_tray(&mut self, reason: CommitReason) {
        if self.calendar_popup_open {
            self.close_calendar_popup(reason);
        }
        if self.workspace_popup_open {
            self.close_workspace_popup(reason);
        }
        if self.network_popup_open {
            self.close_network_popup(reason);
        }
        if self.audio_popup_open {
            self.close_audio_popup(reason);
        }
        if !self.launcher_state.open {
            self.toggle_launcher();
        }
        self.launcher_settings_open = true;
        self.settings_category = crate::settings_view::SettingsCategory::Network;
        self.request_settings_refresh(crate::settings_view::SettingsCategory::Network);
        self.launcher_dirty = true;
        self.panel_dirty = true;
    }

    pub(super) fn toggle_calendar_popup(&mut self, reason: CommitReason) {
        if self.calendar_popup_open {
            self.close_calendar_popup(reason);
            return;
        }

        if self.network_popup_open {
            self.close_network_popup(reason);
        }
        if self.workspace_popup_open {
            self.close_workspace_popup(reason);
        }

        if self.launcher_state.open {
            self.launcher_state.close();
            self.launcher_layer
                .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
            self.unmap_launcher(reason);
        }

        self.calendar_popup_open = true;
        self.calendar_layer
            .set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
        self.calendar_layer.set_margin(
            0,
            crate::CALENDAR_POPUP_RIGHT_MARGIN,
            crate::SHELL_POPUP_BOTTOM_MARGIN,
            0,
        );
        self.calendar_layer.set_exclusive_zone(0);
        self.calendar_layer.set_size(
            crate::popup_surface_w(crate::CALENDAR_POPUP_WIDTH),
            crate::popup_surface_h(crate::CALENDAR_POPUP_HEIGHT),
        );
        self.calendar_layer
            .set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        self.calendar_width = crate::popup_surface_w(crate::CALENDAR_POPUP_WIDTH);
        self.calendar_height = crate::popup_surface_h(crate::CALENDAR_POPUP_HEIGHT);
        self.calendar_dirty = true;
        tracing::debug!(
            "toggle_calendar_popup: open_after={} configured={} size={}x{} keyboard_focus={:?}",
            self.calendar_popup_open,
            self.calendar_configured,
            self.calendar_width,
            self.calendar_height,
            self.keyboard_focus
        );
    }

    pub(crate) fn close_calendar_popup(&mut self, reason: CommitReason) -> bool {
        if !self.calendar_popup_open {
            return false;
        }
        self.calendar_popup_open = false;
        self.calendar_layer
            .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        self.unmap_calendar_popup(reason);
        tracing::debug!(
            "close_calendar_popup: open_after={} configured={} keyboard_focus={:?}",
            self.calendar_popup_open,
            self.calendar_configured,
            self.keyboard_focus
        );
        true
    }

    pub(super) fn toggle_workspace_popup(&mut self, reason: CommitReason) {
        if self.workspace_popup_open {
            self.close_workspace_popup(reason);
            return;
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
        if self.network_popup_open {
            self.close_network_popup(reason);
        }

        self.workspace_popup_open = true;
        self.workspace_hover_idx = None;
        self.workspace_layer
            .set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
        self.workspace_layer
            .set_margin(
                0,
                crate::WORKSPACE_POPUP_RIGHT_MARGIN,
                crate::SHELL_POPUP_BOTTOM_MARGIN,
                0,
            );
        self.workspace_layer.set_exclusive_zone(0);
        self.workspace_layer.set_size(
            crate::popup_surface_w(crate::WORKSPACE_POPUP_WIDTH),
            crate::popup_surface_h(crate::WORKSPACE_POPUP_HEIGHT),
        );
        self.workspace_layer
            .set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        self.workspace_width = crate::popup_surface_w(crate::WORKSPACE_POPUP_WIDTH);
        self.workspace_height = crate::popup_surface_h(crate::WORKSPACE_POPUP_HEIGHT);
        self.workspace_dirty = true;
        tracing::debug!(
            "toggle_workspace_popup: open_after={} configured={} size={}x{} keyboard_focus={:?}",
            self.workspace_popup_open,
            self.workspace_configured,
            self.workspace_width,
            self.workspace_height,
            self.keyboard_focus
        );
    }

    pub(crate) fn close_workspace_popup(&mut self, reason: CommitReason) -> bool {
        if !self.workspace_popup_open {
            return false;
        }
        self.workspace_popup_open = false;
        self.workspace_hover_idx = None;
        self.workspace_layer
            .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        self.unmap_workspace_popup(reason);
        tracing::debug!(
            "close_workspace_popup: open_after={} configured={} keyboard_focus={:?}",
            self.workspace_popup_open,
            self.workspace_configured,
            self.keyboard_focus
        );
        true
    }

    pub(super) fn toggle_network_popup(&mut self, reason: CommitReason) {
        if self.network_popup_open {
            self.close_network_popup(reason);
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
        if self.audio_popup_open {
            self.close_audio_popup(reason);
        }

        self.network_popup_open = true;
        // Refresh the live network state on open so the Status tab always shows
        // the current primary connection (e.g. right after connecting Wi-Fi or
        // unplugging the cable), not the last timer-polled snapshot.
        self.network_controller.poll();
        self.network_layer
            .set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
        self.network_layer.set_margin(
            0,
            crate::NETWORK_POPUP_RIGHT_MARGIN,
            crate::SHELL_POPUP_BOTTOM_MARGIN,
            0,
        );
        self.network_layer.set_exclusive_zone(0);
        self.network_layer.set_size(
            crate::popup_surface_w(crate::NETWORK_POPUP_WIDTH),
            crate::popup_surface_h(crate::NETWORK_POPUP_HEIGHT),
        );
        self.network_layer
            .set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        self.network_width = crate::popup_surface_w(crate::NETWORK_POPUP_WIDTH);
        self.network_height = crate::popup_surface_h(crate::NETWORK_POPUP_HEIGHT);
        self.network_dirty = true;
        // If we just unmapped (transitioning from another shared-layer popup),
        // flush the pending state so the compositor sends a fresh configure
        // before any buffer commit. draw_network_popup will skip until the
        // configure handler flips network_configured back to true.
        if !self.network_configured {
            self.network_layer.commit();
        }
        tracing::debug!(
            "toggle_network_popup: open_after={} configured={} size={}x{} keyboard_focus={:?}",
            self.network_popup_open,
            self.network_configured,
            self.network_width,
            self.network_height,
            self.keyboard_focus
        );
    }
}
