macro_rules! xwm_input_selection_methods {
    () => {
    fn property_notify(&mut self, _xwm: XwmId, _window: X11Surface, property: WmWindowProperty) {
        if matches!(property, WmWindowProperty::Title | WmWindowProperty::Class) {
            self.broadcast_window_snapshot();
        }
    }

    fn resize_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        button: u32,
        edges: X11ResizeEdge,
    ) {
        let window_id = window.window_id();
        if window.is_override_redirect() {
            debug!(
                event = "xwayland.resize_request.ignored",
                window_id,
                override_redirect = true,
                button,
                edges = ?edges,
                reason = "override_redirect",
                "ignoring xwayland resize request"
            );
            return;
        }
        let Some(mapped_window) = find_active_x11_window(self, &window) else {
            debug!(
                event = "xwayland.resize_request.ignored",
                window_id,
                override_redirect = false,
                button,
                edges = ?edges,
                reason = "window_not_mapped",
                "ignoring xwayland resize request"
            );
            return;
        };
        let fullscreen_shaped = window_is_output_fullscreen_shape(self, &mapped_window);
        if fullscreen_shaped {
            debug!(
                event = "xwayland.resize_request.ignored",
                window_id,
                button,
                edges = ?edges,
                fullscreen_shaped,
                reason = "fullscreen_shaped",
                "ignoring xwayland resize request"
            );
            return;
        }
        debug!(
            event = "xwayland.resize_request.start",
            window_id,
            button,
            edges = ?edges,
            fullscreen_shaped,
            geometry = ?window.geometry(),
            "handling xwayland resize request"
        );
        let Some(pointer) = self.seat.get_pointer() else {
            tracing::debug!("ignoring xwayland resize request: seat has no pointer");
            return;
        };
        let Some(start_data) = pointer.grab_start_data() else {
            tracing::debug!(
                "ignoring xwayland resize request: pointer grab start data unavailable"
            );
            return;
        };
        let resize_edges = x11_resize_edge_to_resize_edge(edges);
        if resize_edges.is_empty() {
            tracing::debug!("ignoring xwayland resize request: empty resize edges");
            return;
        }
        let Some(initial_window_location) = self
            .workspaces
            .space_at(self.workspaces.active)
            .element_location(&mapped_window)
        else {
            tracing::debug!("ignoring xwayland resize request: window location unavailable");
            return;
        };
        let initial_window_size = mapped_window.geometry().size;
        clear_managed_xwayland_maximized_state(self, &window);
        debug!(
            event = "xwayland.resize_request.grab",
            window_id,
            initial_window_location = ?initial_window_location,
            initial_window_size = ?initial_window_size,
            "starting xwayland resize grab"
        );
        let grab = ResizeSurfaceGrab::start(
            configure_interval_at(self, start_data.location),
            start_data,
            mapped_window,
            resize_edges,
            Rectangle::new(initial_window_location, initial_window_size),
        );
        let serial = SERIAL_COUNTER.next_serial();
        pointer.set_grab(self, grab, serial, Focus::Clear);
    }

    fn move_request(&mut self, _xwm: XwmId, window: X11Surface, button: u32) {
        let window_id = window.window_id();
        if window.is_override_redirect() {
            debug!(
                event = "xwayland.move_request.ignored",
                window_id,
                override_redirect = true,
                button,
                reason = "override_redirect",
                "ignoring xwayland move request"
            );
            return;
        }
        let Some(mapped_window) = find_active_x11_window(self, &window) else {
            debug!(
                event = "xwayland.move_request.ignored",
                window_id,
                override_redirect = false,
                button,
                reason = "window_not_mapped",
                "ignoring xwayland move request"
            );
            return;
        };
        let fullscreen_shaped = window_is_output_fullscreen_shape(self, &mapped_window);
        if fullscreen_shaped {
            debug!(
                event = "xwayland.move_request.ignored",
                window_id,
                button,
                fullscreen_shaped,
                reason = "fullscreen_shaped",
                "ignoring xwayland move request"
            );
            return;
        }
        debug!(
            event = "xwayland.move_request.start",
            window_id,
            button,
            fullscreen_shaped,
            geometry = ?window.geometry(),
            "handling xwayland move request"
        );
        let Some(pointer) = self.seat.get_pointer() else {
            tracing::debug!("ignoring xwayland move request: seat has no pointer");
            return;
        };
        let Some(start_data) = pointer.grab_start_data() else {
            tracing::debug!("ignoring xwayland move request: pointer grab start data unavailable");
            return;
        };
        let Some(initial_window_location) = self
            .workspaces
            .space_at(self.workspaces.active)
            .element_location(&mapped_window)
        else {
            tracing::debug!("ignoring xwayland move request: window location unavailable");
            return;
        };
        debug!(
            event = "xwayland.move_request.grab",
            window_id,
            initial_window_location = ?initial_window_location,
            "starting xwayland move grab"
        );
        let grab = MoveSurfaceGrab {
            start_data,
            window: mapped_window,
            initial_window_location,
            latest_pointer_location: None,
            started_maximized: false,
            started_fullscreen: false,
            drag_restore_done: false,
            workspace: self.workspaces.active,
        };
        let serial = SERIAL_COUNTER.next_serial();
        pointer.set_grab(self, grab, serial, Focus::Clear);
    }

    fn allow_selection_access(&mut self, xwm: XwmId, _selection: SelectionTarget) -> bool {
        let Some(keyboard) = self.seat.get_keyboard() else {
            return false;
        };
        let Some(focused_surface) = keyboard.current_focus() else {
            return false;
        };

        (0..self.workspaces.count()).any(|workspace| {
            self.workspaces
                .space_at(workspace)
                .elements()
                .find(|window| {
                    window
                        .wl_surface()
                        .map(|surface| surface.into_owned())
                        .as_ref()
                        == Some(&focused_surface)
                })
                .and_then(|window| window.x11_surface())
                .is_some_and(|surface| surface.xwm_id().is_some_and(|id| id == xwm))
        })
    }

    fn new_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_types: Vec<String>) {
        match selection {
            SelectionTarget::Clipboard => {
                set_data_device_selection(&self.display_handle, &self.seat, mime_types, ())
            }
            SelectionTarget::Primary => {
                set_primary_selection(&self.display_handle, &self.seat, mime_types, ())
            }
        }
    }

    fn cleared_selection(&mut self, _xwm: XwmId, selection: SelectionTarget) {
        match selection {
            SelectionTarget::Clipboard => {
                if current_data_device_selection_userdata(&self.seat).is_some() {
                    clear_data_device_selection(&self.display_handle, &self.seat);
                }
            }
            SelectionTarget::Primary => {
                if current_primary_selection_userdata(&self.seat).is_some() {
                    clear_primary_selection(&self.display_handle, &self.seat);
                }
            }
        }
    }

    fn send_selection(
        &mut self,
        _xwm: XwmId,
        selection: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
    ) {
        match selection {
            SelectionTarget::Clipboard => {
                if let Err(err) = request_data_device_client_selection(&self.seat, mime_type, fd) {
                    warn!(
                        ?err,
                        "failed to request current wayland clipboard selection for x11 transfer"
                    );
                }
            }
            SelectionTarget::Primary => {
                if let Err(err) = request_primary_client_selection(&self.seat, mime_type, fd) {
                    warn!(
                        ?err,
                        "failed to request current wayland primary selection for x11 transfer"
                    );
                }
            }
        }
    }

    fn disconnected(&mut self, _xwm: XwmId) {
        debug!(event = "xwayland.disconnected", "xwayland wm disconnected");
        self.xwm = None;
    }
    };
}
