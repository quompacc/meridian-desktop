macro_rules! xwm_window_lifecycle_methods {
    () => {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwm
            .as_mut()
            .expect("xwm_state called but X11Wm is not initialised")
    }

    fn new_window(&mut self, _xwm: XwmId, window: X11Surface) {
        debug!(
            event = "xwayland.new_window",
            window_id = window.window_id(),
            override_redirect = window.is_override_redirect(),
            geometry = ?window.geometry(),
            "xwayland window announced"
        );
    }

    fn new_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let window_id = window.window_id();
        let mapped_window_id = window.mapped_window_id();
        let geometry = window.geometry();
        let title = window.title();
        let class = window.class();
        let instance = window.instance();
        let window_type = window.window_type().map(|ty| format!("{:?}", ty));
        let transient_for = window.is_transient_for();
        let transient_for_mapped = transient_for.and_then(|parent_id| {
            find_x11_surface_by_window_id(self, parent_id)
                .and_then(|parent| parent.mapped_window_id())
        });
        let is_popup = window.is_popup();
        self.xwayland_or_diag.insert(
            window_id,
            XwaylandOrDiagEntry {
                window_id,
                mapped_window_id,
                announce_at: std::time::Instant::now(),
                map_at: None,
                title: title.clone(),
                class: class.clone(),
                instance: instance.clone(),
                window_type: window_type.clone(),
                transient_for,
                transient_for_mapped,
                is_popup,
                last_geometry: geometry,
                last_map_location: None,
                last_configure_request: None,
                last_configure_notify: None,
                last_pointer_event: None,
                last_release_diag: None,
            },
        );
        trace!(
            event = "xwayland.or_diag.new_override_redirect_window",
            window_id,
            mapped_window_id = ?mapped_window_id,
            geometry = ?geometry,
            title = %title,
            class = %class,
            instance = %instance,
            window_type = ?window_type,
            transient_for = ?transient_for,
            transient_for_mapped = ?transient_for_mapped,
            is_popup,
            "xwayland.or_diag: OR window announced"
        );
        debug!(
            event = "xwayland.new_override_redirect_window",
            window_id,
            override_redirect = window.is_override_redirect(),
            geometry = ?geometry,
            "xwayland override-redirect window announced"
        );
    }

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let window_id = window.window_id();
        let is_override_redirect = window.is_override_redirect();
        let initial_geometry = window.geometry();
        debug!(
            event = "xwayland.map_window_request.start",
            window_id,
            override_redirect = is_override_redirect,
            geometry = ?initial_geometry,
            "handling xwayland map window request"
        );
        if let Err(e) = window.set_mapped(true) {
            error!("map_window_request: set_mapped failed: {}", e);
            return;
        }
        let geo = window.geometry();
        // Place at a sensible default if the window hasn't reported a size yet.
        let loc = if geo.size.w > 0 && geo.size.h > 0 {
            geo.loc
        } else {
            (100, 100).into()
        };
        let requested_rect = Rectangle::new(loc, geo.size);
        let output_geometry = select_output_geometry_for_rect(self, requested_rect);
        let clamped_loc = output_geometry
            .map(|geometry| panel_safe_normal_xwayland_rect(requested_rect, geometry))
            .map(|rect| rect.loc)
            .unwrap_or(loc);
        debug!(
            event = "xwayland.map_window_request.geometry",
            window_id,
            requested_rect = ?requested_rect,
            output_geometry = ?output_geometry,
            clamp_applied = output_geometry.is_some(),
            final_loc = ?clamped_loc,
            map_path = "managed",
            "resolved xwayland managed map geometry"
        );
        let win = Window::new_x11_window(window.clone());
        let active = self.workspaces.active;
        let opened = window_list_entry(&win);
        self.workspaces
            .space_at_mut(active)
            .map_element(win, clamped_loc, true);
        if let Some(wl_surface) = window.wl_surface() {
            let is_maximized = window.is_maximized();
            let is_fullscreen = window.is_fullscreen();
            let mut decoration_target = SurfaceDecorationSyncTarget {
                decoration_manager: &mut self.decoration_manager,
                wl_surface: &wl_surface,
            };
            apply_managed_map_ssd(&mut decoration_target, is_maximized, is_fullscreen);
            info!(
                event = "xwayland.ssd.applied_at_map_request",
                window_id,
                maximized = is_maximized,
                fullscreen = is_fullscreen,
                wl_surface_id = wl_surface.id().protocol_id(),
                "applied SSD state in map_window_request"
            );
        } else {
            info!(
                event = "xwayland.ssd.no_wl_surface",
                window_id, "managed xwayland window had no wl_surface for ssd sync"
            );
        }
        if let Some((id, title)) = opened {
            self.broadcast_window_opened(id, title);
        }
        self.mark_all_outputs_dirty("xwayland-map-window");
    }

    fn map_window_notify(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let window_id = window.window_id();
        let is_override_redirect = window.is_override_redirect();
        let geo = window.geometry();
        let loc = if geo.size.w > 0 && geo.size.h > 0 {
            geo.loc
        } else {
            (0, 0).into()
        };
        let win = Window::new_x11_window(window.clone());
        let active = self.workspaces.active;
        self.workspaces
            .space_at_mut(active)
            .map_element(win, loc, false);
        if let Some(wl_surface) = window.wl_surface() {
            let mut decoration_target = SurfaceDecorationSyncTarget {
                decoration_manager: &mut self.decoration_manager,
                wl_surface: &wl_surface,
            };
            apply_override_redirect_ssd(&mut decoration_target);
            info!(
                event = "xwayland.ssd.override_redirect_applied",
                window_id,
                wl_surface_id = wl_surface.id().protocol_id(),
                "applied no-ssd state for override-redirect window"
            );
        }
        update_or_diag_entry(self, &window, |entry| {
            entry.mapped_window_id = window.mapped_window_id();
            entry.map_at = Some(std::time::Instant::now());
            entry.last_geometry = geo;
            entry.last_map_location = Some(loc);
        });
        trace!(
            event = "xwayland.or_diag.mapped_override_redirect_window",
            window_id,
            mapped_window_id = ?window.mapped_window_id(),
            geometry = ?geo,
            map_location = ?loc,
            activate = false,
            keyboard_focus = ?self.keyboard_focus_diag_target(),
            "xwayland.or_diag: OR window mapped"
        );
        debug!(
            event = "xwayland.mapped_override_redirect_window",
            window_id,
            override_redirect = is_override_redirect,
            geometry = ?geo,
            final_loc = ?loc,
            map_path = "override_redirect",
            "mapped xwayland override-redirect window"
        );
        self.mark_all_outputs_dirty("xwayland-map-override");
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let window_id = window.window_id();
        let is_override_redirect = window.is_override_redirect();
        if let Some(wl_surface) = window.wl_surface() {
            self.decoration_manager.remove(&wl_surface);
        }
        if is_override_redirect {
            let (elapsed_since_announce_ms, elapsed_since_map_ms, last_state) = self
                .xwayland_or_diag
                .get(&window_id)
                .map(|entry| {
                    (
                        Some(entry.announce_at.elapsed().as_millis()),
                        entry.map_at.map(|t| t.elapsed().as_millis()),
                        Some(format!("{:?}", entry)),
                    )
                })
                .unwrap_or((None, None, None));
            trace!(
                event = "xwayland.or_diag.unmapped_window",
                window_id,
                elapsed_since_announce_ms = ?elapsed_since_announce_ms,
                elapsed_since_map_ms = ?elapsed_since_map_ms,
                last_state = ?last_state,
                keyboard_focus = ?self.keyboard_focus_diag_target(),
                "xwayland.or_diag: OR window unmapped"
            );
        }
        debug!(
            event = "xwayland.unmapped_window.start",
            window_id,
            override_redirect = is_override_redirect,
            geometry = ?window.geometry(),
            "handling xwayland unmap"
        );
        // Search every workspace: an X11 window can unmap while a different
        // workspace is active, and active-only lookup would skip its cleanup.
        if let Some((workspace, win)) = find_x11_window_with_workspace(self, &window) {
            if let Some((id, _)) = window_list_entry(&win) {
                self.broadcast_window_closed(id.clone());
                self.clear_window_runtime_state(&id);
                debug!(
                    event = "xwayland.unmapped_window.closed",
                    window_id,
                    published_id = id,
                    "broadcasted window closed for xwayland unmap"
                );
            }
            self.workspaces.space_at_mut(workspace).unmap_elem(&win);
            self.mark_all_outputs_dirty("xwayland-unmap-window");
        }
        if !is_override_redirect {
            let _ = window.set_mapped(false);
        }
        debug!(
            event = "xwayland.unmapped_window.done",
            window_id,
            override_redirect = is_override_redirect,
            mapped_flag_cleared = !is_override_redirect,
            "completed xwayland unmap handling"
        );
    }

    fn destroyed_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let window_id = window.window_id();
        if let Some(wl_surface) = window.wl_surface() {
            self.decoration_manager.remove(&wl_surface);
        }
        // Mirror the XDG destroyed path: a window destroyed without a
        // preceding unmap on the active workspace (closed while minimized,
        // or while another workspace is active) must still leave the window
        // list and clear its runtime state, or it lingers as a taskbar ghost.
        let key = x11_window_key(&window);
        self.broadcast_window_closed(key.clone());
        self.clear_window_runtime_state(&key);
        if let Some((workspace, win)) = find_x11_window_with_workspace(self, &window) {
            self.workspaces.space_at_mut(workspace).unmap_elem(&win);
            self.mark_all_outputs_dirty("xwayland-destroy-window");
        }
        if window.is_override_redirect() {
            let (elapsed_since_announce_ms, elapsed_since_map_ms, last_state) = self
                .xwayland_or_diag
                .get(&window_id)
                .map(|entry| {
                    (
                        Some(entry.announce_at.elapsed().as_millis()),
                        entry.map_at.map(|t| t.elapsed().as_millis()),
                        Some(format!("{:?}", entry)),
                    )
                })
                .unwrap_or((None, None, None));
            trace!(
                event = "xwayland.or_diag.destroyed_window",
                window_id,
                elapsed_since_announce_ms = ?elapsed_since_announce_ms,
                elapsed_since_map_ms = ?elapsed_since_map_ms,
                last_state = ?last_state,
                keyboard_focus = ?self.keyboard_focus_diag_target(),
                "xwayland.or_diag: OR window destroyed"
            );
            self.xwayland_or_diag.remove(&window_id);
        }
        debug!(
            event = "xwayland.destroyed_window",
            window_id,
            override_redirect = window.is_override_redirect(),
            geometry = ?window.geometry(),
            "xwayland window destroyed"
        );
    }
    };
}
