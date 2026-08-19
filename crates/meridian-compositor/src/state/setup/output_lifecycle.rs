impl MeridianState {
    pub fn handle_output_added_or_updated(&mut self, registration: OutputRegistration) -> OutputId {
        let existed = self.output_registry.contains_name(&registration.name);
        let id = self.output_registry.upsert(registration.clone());
        if let Some(info) = self.output_registry.by_id(id) {
            tracing::info!(
                "output {}: id={} name={} primary={} geometry=({},{} {}x{}) scale={} transform={:?} refresh={:?}",
                if existed { "reconfigured" } else { "registered" },
                info.id.0,
                info.name,
                info.primary,
                info.geometry.x,
                info.geometry.y,
                info.geometry.width,
                info.geometry.height,
                info.scale,
                info.transform,
                info.refresh_millihz
            );
        } else {
            tracing::info!(
                "output {} fallback: id={} name={} geometry=({},{} {}x{})",
                if existed {
                    "reconfigured"
                } else {
                    "registered"
                },
                id.0,
                registration.name,
                registration.geometry.x,
                registration.geometry.y,
                registration.geometry.width,
                registration.geometry.height
            );
        }
        if let Some(primary) = self.output_registry.primary() {
            tracing::debug!(
                "output primary/fallback: id={} name={}",
                primary.id.0,
                primary.name
            );
        }
        self.post_output_state_change(
            if existed {
                "output-updated"
            } else {
                "output-added"
            },
            Some(id),
            Some(&registration.name),
        );
        id
    }

    /// Move any window stranded off every live output back onto the primary
    /// output's origin. Called after the output set changes (removal or a
    /// resolution shrink) so a window is never left unreachable off-screen.
    /// The reclamp logic is unit-tested in workspace.rs via a mock element.
    /// Returns the number of windows relocated.
    fn reclamp_windows_to_live_outputs(&mut self) -> usize {
        let live_outputs: Vec<smithay::utils::Rectangle<i32, smithay::utils::Logical>> = self
            .output_registry
            .list()
            .iter()
            .map(|o| {
                smithay::utils::Rectangle::new(
                    (o.geometry.x, o.geometry.y).into(),
                    (o.geometry.width, o.geometry.height).into(),
                )
            })
            .collect();
        let fallback: smithay::utils::Point<i32, smithay::utils::Logical> = self
            .output_registry
            .primary()
            .map(|p| (p.geometry.x, p.geometry.y).into())
            .unwrap_or_default();
        self.workspaces
            .reclamp_offscreen_windows(&live_outputs, fallback)
    }

    pub fn handle_output_removed(&mut self, id: OutputId) -> bool {
        let Some(removed) = self.output_registry.remove_by_id(id) else {
            return false;
        };
        if let Some(resources) = self.output_power_resources.remove(&removed.name) {
            for resource in &resources {
                resource.failed();
            }
            self.output_power_manager.forget(&removed.name);
            tracing::debug!(
                "wlr-output-power: sent failed to {} resources for removed output {}",
                resources.len(),
                removed.name
            );
        } else {
            self.output_power_manager.forget(&removed.name);
        }
        tracing::info!(
            "output removed: id={} name={} geometry=({},{} {}x{})",
            removed.id.0,
            removed.name,
            removed.geometry.x,
            removed.geometry.y,
            removed.geometry.width,
            removed.geometry.height
        );
        let rescued = self.reclamp_windows_to_live_outputs();
        if rescued > 0 {
            tracing::info!(
                "output removed: reclamped {} off-screen window(s) onto a surviving output",
                rescued
            );
        }
        // A maximized window rescued onto a surviving output keeps its old size
        // (reclamp only moves position); re-measure it to the new workarea.
        crate::state::handlers::xdg::requests::remeasure_maximized_windows(self);
        crate::protocols::xwayland::remeasure_maximized_x11_windows(self);
        self.post_output_state_change("output-removed", Some(id), Some(&removed.name));
        true
    }

    pub fn handle_output_reconfigured(
        &mut self,
        id: OutputId,
        reconfigure: OutputReconfigure,
    ) -> bool {
        if !self.output_registry.reconfigure_by_id(id, reconfigure) {
            return false;
        }
        if let Some(info) = self.output_registry.by_id(id) {
            tracing::info!(
                "output reconfigured: id={} name={} primary={} geometry=({},{} {}x{}) scale={} transform={:?} refresh={:?}",
                info.id.0,
                info.name,
                info.primary,
                info.geometry.x,
                info.geometry.y,
                info.geometry.width,
                info.geometry.height,
                info.scale,
                info.transform,
                info.refresh_millihz
            );
        }
        // A resolution shrink can leave a window outside the new geometry; pull
        // it back on-screen (same reclamp as output removal).
        let rescued = self.reclamp_windows_to_live_outputs();
        if rescued > 0 {
            tracing::info!(
                "output reconfigured: reclamped {} off-screen window(s) onto a surviving output",
                rescued
            );
        }
        // Maximized windows keep their old size across a mode switch; re-measure
        // them to the new output geometry.
        crate::state::handlers::xdg::requests::remeasure_maximized_windows(self);
        crate::protocols::xwayland::remeasure_maximized_x11_windows(self);
        let output_name = self.output_registry.by_id(id).map(|info| info.name.clone());
        self.post_output_state_change("output-reconfigured", Some(id), output_name.as_deref());
        true
    }

    pub fn register_output_info(&mut self, registration: OutputRegistration) -> OutputId {
        self.handle_output_added_or_updated(registration)
    }

    pub fn output_geometry_for_registry(x: i32, y: i32, width: i32, height: i32) -> OutputGeometry {
        OutputGeometry {
            x,
            y,
            width,
            height,
        }
    }

    fn init_wayland_listener(
        display: Display<Self>,
        event_loop: &mut EventLoop<Self>,
    ) -> Result<OsString, Box<dyn std::error::Error>> {
        let listening_socket = ListeningSocketSource::new_auto().map_err(|err| {
            Error::other(format!("failed to create wayland listening socket: {err}"))
        })?;
        let socket_name = listening_socket.socket_name().to_os_string();
        let loop_handle = event_loop.handle();

        loop_handle
            .insert_source(listening_socket, move |client_stream, _, state| {
                if let Err(err) = state
                    .display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                {
                    tracing::warn!("failed to insert wayland client: {}", err);
                }
            })
            .map_err(|err| {
                Error::other(format!("failed to initialize wayland socket source: {err}"))
            })?;

        loop_handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    // SAFETY: the event loop owns `display`; calloop guarantees serialized access in this callback.
                    unsafe { display.get_mut().dispatch_clients(state) }.map_err(|err| {
                        Error::other(format!("failed to dispatch wayland clients: {err}"))
                    })?;
                    Ok(PostAction::Continue)
                },
            )
            .map_err(|err| {
                Error::other(format!("failed to insert display into event loop: {err}"))
            })?;

        Ok(socket_name)
    }
}
