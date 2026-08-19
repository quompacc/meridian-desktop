impl MeridianState {
    pub fn mark_all_outputs_dirty(&mut self, reason: &str) {
        let Some(drm) = self.drm_backend.as_mut() else {
            return;
        };
        let mut marked = 0usize;
        for output in drm.outputs.iter_mut() {
            drm.dirty_stats
                .record_dirty_mark_event(output.output_id, reason);
            if !output.needs_repaint {
                output.needs_repaint = true;
                drm.dirty_stats.record_dirty_set(output.output_id);
                marked += 1;
            }
        }
        if marked > 0 {
            tracing::trace!(
                "marked all outputs dirty: reason={} count={}",
                reason,
                marked
            );
        }
    }

    pub fn mark_output_dirty(&mut self, output_id: OutputId, reason: &str) {
        let Some(drm) = self.drm_backend.as_mut() else {
            return;
        };
        drm.dirty_stats.record_dirty_mark_event(output_id, reason);
        if let Some(output) = drm
            .outputs
            .iter_mut()
            .find(|output| output.output_id == output_id)
        {
            if !output.needs_repaint {
                output.needs_repaint = true;
                drm.dirty_stats.record_dirty_set(output_id);
                tracing::trace!(
                    "marked output dirty: reason={} output_id={} output={}",
                    reason,
                    output_id.0,
                    output.output.name()
                );
            }
        }
    }

    pub fn mark_output_dirty_by_name(&mut self, output_name: &str, reason: &str) {
        let Some(drm) = self.drm_backend.as_mut() else {
            return;
        };
        if let Some(output) = drm
            .outputs
            .iter_mut()
            .find(|output| output.output.name() == output_name)
        {
            drm.dirty_stats
                .record_dirty_mark_event(output.output_id, reason);
            if !output.needs_repaint {
                output.needs_repaint = true;
                drm.dirty_stats.record_dirty_set(output.output_id);
                tracing::trace!(
                    "marked output dirty: reason={} output_id={} output={}",
                    reason,
                    output.output_id.0,
                    output_name
                );
            }
        }
    }

    pub fn new(
        event_loop: &mut EventLoop<'static, Self>,
        display: Display<Self>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let display_handle = display.handle();

        let compositor_state = CompositorState::new::<Self>(&display_handle);
        let xdg_shell_state = XdgShellState::new::<Self>(&display_handle);
        let decoration_state = smithay::wayland::shell::xdg::decoration::XdgDecorationState::new::<
            Self,
        >(&display_handle);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&display_handle);
        let shm_state = ShmState::new::<Self>(&display_handle, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&display_handle);
        let data_device_state = DataDeviceState::new::<Self>(&display_handle);
        let primary_selection_state = PrimarySelectionState::new::<Self>(&display_handle);
        let xwayland_shell_state = XWaylandShellState::new::<Self>(&display_handle);
        let text_input_manager_state = TextInputManagerState::new::<Self>(&display_handle);
        // cursor-shape-v1: GTK4/Qt6 apps request a NAMED cursor from the
        // compositor (drawn via the Named path at our size) instead of each app
        // uploading its own cursor buffer — which sized them wrong (huge in
        // apps). Delegation is already covered by delegate_dispatch2!.
        let cursor_shape_manager_state =
            smithay::wayland::cursor_shape::CursorShapeManagerState::new::<Self>(&display_handle);
        let input_method_manager_state =
            InputMethodManagerState::new::<Self, _>(&display_handle, |_client| true);
        let xdg_activation_state = XdgActivationState::new::<Self>(&display_handle);
        // wp_presentation global intentionally NOT registered: see field doc
        // on MeridianState. Plumbing-only mode hangs firefox caret-blink.
        let presentation_state: Option<PresentationState> = None;
        let fractional_scale_manager_state =
            FractionalScaleManagerState::new::<Self>(&display_handle);
        let viewporter_state = ViewporterState::new::<Self>(&display_handle);

        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(&display_handle, "seat-0");
        // Respect the system keyboard layout (e.g. German). Smithay's default
        // XkbConfig is empty, which makes libxkbcommon fall back to "us"; the
        // session env carries no XKB_DEFAULT_LAYOUT, so read it from the system
        // config (Arch: /etc/vconsole.conf, Debian: /etc/default/keyboard).
        let (xkb_model, xkb_layout, xkb_variant, xkb_options) = system_xkb_settings();
        let xkb_config = smithay::input::keyboard::XkbConfig {
            model: &xkb_model,
            layout: &xkb_layout,
            variant: &xkb_variant,
            options: if xkb_options.is_empty() {
                None
            } else {
                Some(xkb_options.clone())
            },
            ..Default::default()
        };
        tracing::info!(layout = %xkb_layout, model = %xkb_model, "seat keyboard xkb layout");
        seat.add_keyboard(xkb_config, 200, 25).unwrap();
        seat.add_pointer();

        let loop_handle = event_loop.handle();
        let idle_notifier = IdleNotifierState::<Self>::new(&display_handle, loop_handle.clone());
        let idle_inhibit_state = IdleInhibitManagerState::new::<Self>(&display_handle);
        let dmabuf_state = DmabufState::new();
        let session_lock_state =
            SessionLockManagerState::new::<Self, _>(&display_handle, |_client| true);
        let output_power_global =
            display_handle.create_global::<Self, ZwlrOutputPowerManagerV1, _>(1, ());
        let socket_name = Self::init_wayland_listener(display, event_loop)?;
        let loop_signal = event_loop.get_signal();

        let image_capture_source_state = ImageCaptureSourceState::new();
        let output_capture_source_state =
            OutputCaptureSourceState::new::<MeridianState>(&display_handle);
        let image_copy_capture_state = ImageCopyCaptureState::new::<MeridianState>(&display_handle);

        let meridian_config = MeridianConfig::load();
        let idle_timeout = meridian_config
            .general
            .idle_timeout_secs
            .map(Duration::from_secs);
        let output_config_entries = meridian_config.outputs.clone();
        let output_layout = super::OutputLayout::from_config_entries(&output_config_entries);
        let mut theme_manager = ThemeManager::new();
        let _ = apply_config_overrides(&mut theme_manager, &meridian_config);

        let mut wallpaper_manager = WallpaperManager::new();
        wallpaper_manager.apply_theme(theme_manager.current());
        // Persist appearance so the boot chain (bootsplash, login) matches
        // the active theme on the next boot.
        let _ = meridian_boot_common::write_appearance(theme_appearance(theme_manager.current()));

        Ok(Self {
            start_time: Instant::now(),
            display_handle,
            loop_handle,
            loop_signal,
            socket_name,
            seat,
            workspaces: WorkspaceManager::new(),
            outputs: Vec::new(),
            output_layout,
            output_config_entries,
            output_registry: OutputRegistry::new(),
            workspace_output_state: WorkspaceOutputState::default(),
            popups: PopupManager::default(),
            theme_manager,
            wallpaper_manager,
            wm_workspaces: (0..9).map(|_| WmWorkspace::new()).collect(),
            ipc: IpcServer::new(),
            keybind_config: meridian_config.keybinds,
            decoration_manager: DecorationManager::new(),
            compositor_state,
            xdg_shell_state,
            decoration_state,
            layer_shell_state,
            shm_state,
            seat_state,
            output_manager_state,
            data_device_state,
            primary_selection_state,
            xwayland_shell_state,
            text_input_manager_state,
            cursor_shape_manager_state,
            input_method_manager_state,
            xdg_activation_state,
            presentation_state,
            fractional_scale_manager_state,
            viewporter_state,
            idle_notifier,
            idle_inhibit_state,
            idle_inhibitors: IdleInhibitorSet::new(),
            dmabuf_state,
            dmabuf_global: None,
            dmabuf_default_feedback: None,
            #[cfg(not(target_os = "openbsd"))]
            syncobj_state: None,
            session_lock_state,
            lock_manager: LockManager::new(),
            output_power_manager: OutputPowerManager::new(),
            output_power_resources: HashMap::new(),
            output_power_global,
            xwm: None,
            drm_backend: None,
            maximize_restore_locations: std::collections::HashMap::new(),
            half_snap_restore_locations: std::collections::HashMap::new(),
            active_window_snap_states: std::collections::HashMap::new(),
            minimized_windows: std::collections::HashMap::new(),
            xwayland_or_diag: std::collections::HashMap::new(),
            cursor_status: smithay::input::pointer::CursorImageStatus::default_named(),
            image_capture_source_state,
            output_capture_source_state,
            image_copy_capture_state,
            screencopy_sessions: Vec::new(),
            pending_screencopy_frames: Vec::new(),
            pending_thumbnail_requests: Vec::new(),
            pending_screenshot_requests: Vec::new(),
            pending_screenshot_consent: Vec::new(),
            pending_screenshot_region: Vec::new(),
            last_activity: Instant::now(),
            idle_blanked: false,
            idle_timeout,
        })
    }

    fn post_output_state_change(
        &mut self,
        action: &str,
        output_id: Option<OutputId>,
        output_name: Option<&str>,
    ) {
        if let Some(output_name) = output_name {
            let removed_from_registry = self.output_registry.by_name(output_name).is_none();
            if removed_from_registry && self.lock_manager.drop_surface(output_name) {
                tracing::debug!(
                    "dropped lock surface marker for removed output={}",
                    output_name
                );
            }
            if removed_from_registry {
                let maybe_ready_locker = self.lock_manager.forget_pending_target(output_name);
                if let Some(locker) = maybe_ready_locker {
                    locker.lock();
                    let _ = self.lock_manager.confirm_locked();
                    self.refresh_lock_focus();
                    tracing::info!("session lock confirmed (last pending target was disconnected)");
                }
            }
        }
        self.sync_outputs_with_workspace_state();
        self.refresh_lock_focus();
        self.mark_all_outputs_dirty("output-state-change");
        tracing::debug!(
            "output hotplug state changed: action={} output_id={:?} output_name={:?}",
            action,
            output_id.map(|id| id.0),
            output_name
        );
        self.reconcile_layer_shell_outputs_after_output_change(action, output_name);
        tracing::debug!(
            "layer-shell recovery reconciled after output change: action={} output_id={:?} output_name={:?}",
            action,
            output_id.map(|id| id.0),
            output_name
        );
        self.broadcast_output_workspace_snapshot();
        tracing::debug!(
            "output workspace snapshot broadcasted after output change: action={} output_id={:?} output_name={:?}",
            action,
            output_id.map(|id| id.0),
            output_name
        );
    }
}
