macro_rules! xwm_state_request_methods {
    () => {
        fn maximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
            apply_x11_maximize(self, &window);
        }

        fn unmaximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
            apply_x11_unmaximize(self, &window);
        }

        fn fullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
            if window.is_override_redirect() {
                return;
            }

            let Some(mapped_window) = find_active_x11_window(self, &window) else {
                return;
            };
            let Some(current_loc) = self
                .workspaces
                .space_at(self.workspaces.active)
                .element_location(&mapped_window)
            else {
                return;
            };
            let current_size = mapped_window.geometry().size;
            let requested_rect = Rectangle::new(current_loc, current_size);
            let Some(output_geometry) = select_output_geometry_for_rect(self, requested_rect)
            else {
                return;
            };
            let target_rect = Rectangle::new(
                (output_geometry.x, output_geometry.y).into(),
                (output_geometry.width.max(1), output_geometry.height.max(1)).into(),
            );

            remember_maximize_restore_geometry(
                &mut self.maximize_restore_locations,
                x11_fullscreen_restore_key(&window),
                MaximizeRestoreGeometry::new(current_loc, Some(current_size)),
            );
            if let Err(err) = window.set_fullscreen(true) {
                error!(
                    "xwayland fullscreen request: set_fullscreen failed: {}",
                    err
                );
            }
            if let Some(wl_surface) = window.wl_surface() {
                self.decoration_manager.set_fullscreen(&wl_surface, true);
            }
            if let Err(err) = window.configure(target_rect) {
                error!("xwayland fullscreen request: configure failed: {}", err);
                return;
            }
            self.workspaces
                .space_at_mut(self.workspaces.active)
                .map_element(mapped_window, target_rect.loc, true);
            self.mark_all_outputs_dirty("xwayland-fullscreen-request");
        }

        fn unfullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
            if window.is_override_redirect() {
                return;
            }

            let Some(mapped_window) = find_active_x11_window(self, &window) else {
                return;
            };
            if let Err(err) = window.set_fullscreen(false) {
                error!(
                    "xwayland unfullscreen request: set_fullscreen failed: {}",
                    err
                );
            }
            if let Some(wl_surface) = window.wl_surface() {
                self.decoration_manager.set_fullscreen(&wl_surface, false);
            }

            let restore = self
                .maximize_restore_locations
                .remove(&x11_fullscreen_restore_key(&window));
            let Some(restore) = restore else {
                return;
            };
            let restore_size = restore
                .client_size
                .unwrap_or_else(|| mapped_window.geometry().size);
            let restore_rect = Rectangle::new(restore.client_loc, restore_size);
            if let Err(err) = window.configure(restore_rect) {
                error!("xwayland unfullscreen request: configure failed: {}", err);
                return;
            }
            self.workspaces
                .space_at_mut(self.workspaces.active)
                .map_element(mapped_window, restore.client_loc, true);
            self.mark_all_outputs_dirty("xwayland-unfullscreen-request");
        }

        fn minimize_request(&mut self, _xwm: XwmId, window: X11Surface) {
            if window.is_override_redirect() {
                return;
            }

            let Some((workspace, mapped_window)) = find_x11_window_with_workspace(self, &window)
            else {
                return;
            };
            let restore_loc = self
                .workspaces
                .space_at(workspace)
                .element_location(&mapped_window)
                .unwrap_or_default();
            let window_key = x11_window_key(&window);

            if let Err(err) = window.set_hidden(true) {
                error!(
                    "xwayland minimize request: set_hidden(true) failed: {}",
                    err
                );
            }

            self.minimized_windows.insert(
                window_key,
                crate::state::MinimizedWindowEntry {
                    window: mapped_window.clone(),
                    workspace,
                    restore_loc,
                },
            );
            self.workspaces
                .space_at_mut(workspace)
                .unmap_elem(&mapped_window);

            let serial = SERIAL_COUNTER.next_serial();
            let window_surface = mapped_window
                .wl_surface()
                .map(|surface| surface.into_owned());
            let was_focused = window_surface.as_ref().is_some_and(|window_surface| {
                self.seat
                    .get_keyboard()
                    .and_then(|keyboard| keyboard.current_focus())
                    .as_ref()
                    == Some(window_surface)
            });
            if was_focused {
                let fallback_surface = self
                    .workspaces
                    .space_at(workspace)
                    .elements()
                    .filter_map(|candidate| {
                        candidate.wl_surface().map(|surface| surface.into_owned())
                    })
                    .next_back();
                if let Some(surface) = fallback_surface {
                    self.set_keyboard_focus_with_decorations(Some(surface.clone()), serial);
                    self.update_focused_output_from_surface(
                        &surface,
                        "keyboard-focus-x11-minimize-fallback",
                    );
                    self.broadcast_toplevel_focused(&surface);
                } else {
                    self.set_keyboard_focus_with_decorations(
                            Option::<
                                smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
                            >::None,
                            serial,
                        );
                    self.broadcast_toplevel_focus_cleared();
                }
            }

            self.workspaces
                .space_at(workspace)
                .elements()
                .for_each(|window| {
                    if let Some(toplevel) = window.toplevel() {
                        toplevel.send_pending_configure();
                    }
                });
            self.mark_all_outputs_dirty("xwayland-minimize-request");
            self.broadcast_window_snapshot();
        }

        fn unminimize_request(&mut self, _xwm: XwmId, window: X11Surface) {
            if window.is_override_redirect() {
                return;
            }

            if let Err(err) = window.set_hidden(false) {
                error!(
                    "xwayland unminimize request: set_hidden(false) failed: {}",
                    err
                );
            }

            let window_key = x11_window_key(&window);
            let Some(minimized) = self.minimized_windows.remove(&window_key) else {
                return;
            };
            let workspace = minimized.workspace;
            if workspace >= self.workspaces.count() {
                return;
            }

            self.workspaces.space_at_mut(workspace).map_element(
                minimized.window.clone(),
                minimized.restore_loc,
                true,
            );

            if workspace == self.workspaces.active {
                self.workspaces
                    .space_at_mut(workspace)
                    .raise_element(&minimized.window, true);

                if let Some(surface) = minimized
                    .window
                    .wl_surface()
                    .map(|surface| surface.into_owned())
                {
                    let serial = SERIAL_COUNTER.next_serial();
                    self.set_keyboard_focus_with_decorations(Some(surface.clone()), serial);
                    self.update_focused_output_from_surface(
                        &surface,
                        "keyboard-focus-x11-unminimize-request",
                    );
                    self.broadcast_toplevel_focused(&surface);
                }
            }

            self.workspaces
                .space_at(workspace)
                .elements()
                .for_each(|window| {
                    if let Some(toplevel) = window.toplevel() {
                        toplevel.send_pending_configure();
                    }
                });
            self.mark_all_outputs_dirty("xwayland-unminimize-request");
            self.broadcast_window_snapshot();
        }
    };
}
