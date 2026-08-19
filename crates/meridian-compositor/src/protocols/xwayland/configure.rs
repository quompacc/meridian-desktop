macro_rules! xwm_configure_methods {
    () => {
    fn configure_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        x: Option<i32>,
        y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        reorder: Option<Reorder>,
    ) {
        let window_id = window.window_id();
        let is_override_redirect = window.is_override_redirect();
        let base_rect = window.geometry();
        let requested_rect = configure_request_rect(base_rect, x, y, w, h);
        if !is_override_redirect {
            let classification_output = select_output_geometry_for_rect(self, requested_rect);
            if let Some(output_geometry) = classification_output {
                let workarea = normal_window_workarea_from_output_geometry(output_geometry);
                let workarea_rect = Rectangle::new(
                    (workarea.x, workarea.y).into(),
                    (workarea.width.max(1), workarea.height.max(1)).into(),
                );
                let output_rect = Rectangle::new(
                    (output_geometry.x, output_geometry.y).into(),
                    (output_geometry.width.max(1), output_geometry.height.max(1)).into(),
                );
                let action = classify_managed_configure_request(
                    is_override_redirect,
                    window.is_maximized(),
                    window.is_fullscreen(),
                    requested_rect,
                    workarea_rect,
                    output_rect,
                );
                match action {
                    ManagedConfigureRequestAction::DenyNoOp => {
                        if let Err(e) = window.configure(None) {
                            error!("configure_request: configure(None) failed: {}", e);
                        }
                        debug!(
                            event = "xwayland.configure_request.deny_noop",
                            window_id,
                            is_maximized = window.is_maximized(),
                            is_fullscreen = window.is_fullscreen(),
                            requested_rect = ?requested_rect,
                            workarea_rect = ?workarea_rect,
                            output_rect = ?output_rect,
                            "configure request matched existing maximize/fullscreen rect; acked without change"
                        );
                        return;
                    }
                    ManagedConfigureRequestAction::ImplicitUnmaximize => {
                        if let Err(err) = window.set_maximized(false) {
                            error!(
                                "configure_request: implicit set_maximized(false) failed: {}",
                                err
                            );
                        }
                        self.maximize_restore_locations
                            .remove(&x11_window_key(&window));
                        debug!(
                            event = "xwayland.configure_request.implicit_unmaximize",
                            window_id,
                            requested_rect = ?requested_rect,
                            workarea_rect = ?workarea_rect,
                            "implicit unmaximize triggered by non-workarea ConfigureRequest on maximized managed window"
                        );
                    }
                    ManagedConfigureRequestAction::ImplicitUnfullscreen => {
                        if let Err(err) = window.set_fullscreen(false) {
                            error!(
                                "configure_request: implicit set_fullscreen(false) failed: {}",
                                err
                            );
                        }
                        self.maximize_restore_locations
                            .remove(&x11_fullscreen_restore_key(&window));
                        debug!(
                            event = "xwayland.configure_request.implicit_unfullscreen",
                            window_id,
                            requested_rect = ?requested_rect,
                            output_rect = ?output_rect,
                            "implicit unfullscreen triggered by non-output-shape ConfigureRequest on fullscreen managed window"
                        );
                    }
                    ManagedConfigureRequestAction::PassThrough => {}
                }
            }
        }
        let output_geometry = select_output_geometry_for_rect(self, requested_rect);
        let frame_insets = window
            .wl_surface()
            .map(|surface| {
                self.decoration_manager.decoration_inset(
                    &surface,
                    &self.theme_manager.current().config.decorations,
                )
            })
            .unwrap_or((0, 0, 0, 0));
        let adjusted_rect = if is_override_redirect {
            requested_rect
        } else {
            output_geometry
                .map(|geometry| {
                    panel_safe_normal_xwayland_rect_with_insets(
                        requested_rect,
                        geometry,
                        frame_insets,
                    )
                })
                .unwrap_or(requested_rect)
        };
        let adjusted_geo = Rectangle::new(adjusted_rect.loc, adjusted_rect.size);
        let clamp_applied = !is_override_redirect && output_geometry.is_some();
        debug!(
            event = "xwayland.configure_request",
            window_id,
            override_redirect = is_override_redirect,
            base_rect = ?base_rect,
            requested_x = ?x,
            requested_y = ?y,
            requested_w = ?w,
            requested_h = ?h,
            requested_rect = ?requested_rect,
            adjusted_rect = ?adjusted_rect,
            output_geometry = ?output_geometry,
            reorder = ?reorder,
            clamp_applied,
            clamp_skipped_override_redirect = is_override_redirect,
            "handling xwayland configure request"
        );
        let configure_called = true;
        let mut configure_ok = false;
        let mut configure_error = None::<String>;
        let mut outputs_dirty = false;
        if let Err(e) = window.configure(adjusted_geo) {
            configure_error = Some(e.to_string());
            error!("configure_request: configure failed: {}", e);
        } else {
            configure_ok = true;
            outputs_dirty = true;
        }
        if is_override_redirect {
            let above_hint = reorder_above_hint(reorder);
            trace!(
                event = "xwayland.or_diag.configure_request",
                window_id,
                geometry_before = ?base_rect,
                request_x = ?x,
                request_y = ?y,
                request_w = ?w,
                request_h = ?h,
                reorder = ?reorder,
                above_hint = ?above_hint,
                configure_called,
                configure_ok,
                configure_error = ?configure_error,
                "xwayland.or_diag: OR configure request"
            );
            update_or_diag_entry(self, &window, |entry| {
                entry.last_geometry = window.geometry();
                entry.last_configure_request = Some(XwaylandOrDiagConfigureRequest {
                    x,
                    y,
                    w,
                    h,
                    reorder: reorder.map(|value| format!("{:?}", value)),
                    above_hint,
                    configure_called,
                    configure_ok,
                    configure_error,
                });
            });
        }

        if is_override_redirect {
            let active = self.workspaces.active;
            let maybe_window = self
                .workspaces
                .space_at(active)
                .elements()
                .find(|w| matches!(w.x11_surface(), Some(x) if x == &window))
                .cloned();
            if let Some(mapped_window) = maybe_window {
                let restacked = match reorder {
                    Some(Reorder::Top) => {
                        self.workspaces
                            .space_at_mut(active)
                            .raise_element(&mapped_window, false);
                        true
                    }
                    Some(Reorder::Above(above)) => restack_override_redirect_above_hint(
                        self,
                        active,
                        &mapped_window,
                        above,
                        "configure_request",
                    ),
                    Some(Reorder::Bottom) => {
                        self.workspaces
                            .space_at_mut(active)
                            .lower_element(&mapped_window);
                        true
                    }
                    Some(Reorder::Below(below)) => {
                        debug!(
                            event = "xwayland.override_redirect.restack_below_ignored",
                            source = "configure_request",
                            below,
                            "override-redirect below restack hint currently ignored"
                        );
                        false
                    }
                    None => false,
                };
                if restacked {
                    outputs_dirty = true;
                }
            }
        }

        if outputs_dirty {
            self.mark_all_outputs_dirty("xwayland-configure-request");
        }
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        geometry: Rectangle<i32, Logical>,
        above: Option<u32>,
    ) {
        let window_id = window.window_id();
        let is_override_redirect = window.is_override_redirect();
        let active = self.workspaces.active;
        let maybe = self
            .workspaces
            .space_at(active)
            .elements()
            .find(|w| matches!(w.x11_surface(), Some(x) if x == &window))
            .cloned();
        if let Some(win) = maybe {
            let output_geometry = if is_override_redirect {
                None
            } else {
                select_output_geometry_for_rect(self, geometry)
            };
            let frame_insets = window
                .wl_surface()
                .map(|surface| {
                    self.decoration_manager.decoration_inset(
                        &surface,
                        &self.theme_manager.current().config.decorations,
                    )
                })
                .unwrap_or((0, 0, 0, 0));
            let loc = if window.is_override_redirect() {
                geometry.loc
            } else {
                output_geometry
                    .map(|geometry_for_output| {
                        panel_safe_normal_xwayland_rect_with_insets(
                            geometry,
                            geometry_for_output,
                            frame_insets,
                        )
                    })
                    .map(|rect| rect.loc)
                    .unwrap_or(geometry.loc)
            };
            debug!(
                event = "xwayland.configure_notify",
                window_id,
                override_redirect = is_override_redirect,
                geometry = ?geometry,
                mapped_loc = ?loc,
                output_geometry = ?output_geometry,
                above = ?above,
                clamp_applied = !is_override_redirect && output_geometry.is_some(),
                "handling xwayland configure notify"
            );
            let previous_loc = self.workspaces.space_at(active).element_location(&win);
            self.workspaces
                .space_at_mut(active)
                .map_element(win.clone(), loc, false);
            let space_position_changed = previous_loc != Some(loc);
            if is_override_redirect {
                trace!(
                    event = "xwayland.or_diag.configure_notify",
                    window_id,
                    geometry = ?geometry,
                    above = ?above,
                    previous_loc = ?previous_loc,
                    new_loc = ?loc,
                    space_position_changed,
                    "xwayland.or_diag: OR configure notify"
                );
                update_or_diag_entry(self, &window, |entry| {
                    entry.last_geometry = geometry;
                    entry.last_map_location = Some(loc);
                    entry.last_configure_notify = Some(XwaylandOrDiagConfigureNotify {
                        geometry,
                        above_hint: above,
                        space_position_changed,
                    });
                });
                if let Some(above_id) = above {
                    restack_override_redirect_above_hint(
                        self,
                        active,
                        &win,
                        above_id,
                        "configure_notify",
                    );
                } else {
                    self.workspaces
                        .space_at_mut(active)
                        .raise_element(&win, false);
                    debug!(
                        event = "xwayland.override_redirect.restack_top",
                        source = "configure_notify",
                        reason = "no_above_hint",
                        "raised override-redirect window to topmost without above target"
                    );
                }
            }
            self.mark_all_outputs_dirty("xwayland-configure-notify");
        }
    }
    };
}
