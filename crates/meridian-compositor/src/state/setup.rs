use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    io::Error,
    sync::Arc,
    time::Instant,
};

use meridian_config::{MeridianConfig, OutputEntry, ThemeManager};
use meridian_wm::WmWorkspace;
use smithay::{
    backend::{allocator::Format, drm::DrmDevice},
    desktop::{layer_map_for_output, PopupManager},
    input::SeatState,
    output::{Mode as OutputMode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, Mode, PostAction},
        drm::control::Device as _,
        wayland_server::Display,
    },
    utils::Transform,
    wayland::{
        compositor::CompositorState,
        idle_inhibit::IdleInhibitManagerState,
        idle_notify::IdleNotifierState,
        output::OutputManagerState,
        selection::{data_device::DataDeviceState, primary_selection::PrimarySelectionState},
        session_lock::SessionLockManagerState,
        shell::{wlr_layer::WlrLayerShellState, xdg::XdgShellState},
        shm::ShmState,
        socket::ListeningSocketSource,
        xwayland_shell::XWaylandShellState,
    },
};

use crate::{
    backend::drm::{DisabledDrmOutput, DrmOutput},
    decoration::DecorationManager,
    wallpaper::WallpaperManager,
    workspace::WorkspaceManager,
};

use super::{
    detect_output_reload_diff, parse_output_transform, ClientState, ConnectedOutput,
    IdleInhibitorSet, IpcServer, LockManager, MeridianState, OutputGeometry, OutputId,
    OutputReconfigure, OutputRegistration, OutputRegistry, WorkspaceOutputState,
};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ThemeOverrideChanges {
    pub theme_changed: bool,
    pub cursor_changed: bool,
    pub wallpaper_changed: bool,
}

pub(crate) fn apply_config_overrides(
    theme_manager: &mut ThemeManager,
    meridian_config: &MeridianConfig,
) -> ThemeOverrideChanges {
    let prev_theme_name = theme_manager.current().name.clone();
    let prev_cursor = (
        theme_manager.current().config.cursor.theme.clone(),
        theme_manager.current().config.cursor.size,
    );
    let prev_wallpaper = theme_manager
        .current()
        .config
        .wallpaper
        .as_ref()
        .map(|w| (w.path.clone(), w.mode));

    let requested_theme = if meridian_config.general.theme.trim().is_empty() {
        "default"
    } else {
        meridian_config.general.theme.trim()
    };
    if let Err(err) = theme_manager.set_theme(requested_theme) {
        tracing::warn!(
            "Failed to load theme {:?} from config: {} — keeping current theme {:?}",
            requested_theme,
            err,
            theme_manager.current().name
        );
    }

    if let Some(cursor) = &meridian_config.cursor {
        theme_manager.current_mut().config.cursor.theme = cursor.theme.clone();
        theme_manager.current_mut().config.cursor.size = cursor.size;
    }
    if meridian_config.wallpaper.is_some() {
        theme_manager.current_mut().config.wallpaper = meridian_config.wallpaper_override();
    }

    ThemeOverrideChanges {
        theme_changed: prev_theme_name != theme_manager.current().name,
        cursor_changed: prev_cursor
            != (
                theme_manager.current().config.cursor.theme.clone(),
                theme_manager.current().config.cursor.size,
            ),
        wallpaper_changed: prev_wallpaper
            != theme_manager
                .current()
                .config
                .wallpaper
                .as_ref()
                .map(|w| (w.path.clone(), w.mode)),
    }
}

impl MeridianState {
    /// Re-resolves all currently registered outputs against the current
    /// `self.output_layout` and applies position/scale/transform/primary
    /// changes to Smithay's Output state, Space mapping, and OutputRegistry.
    ///
    /// Applies mode changes via DrmCompositor rebuild when DRM is active.
    /// Enabled toggles still require output add/remove lifecycle handling
    /// and are logged as restart-required.
    pub fn reapply_output_layout(&mut self, previous_entries: &[OutputEntry]) {
        let mut connected: Vec<ConnectedOutput> = self
            .output_registry
            .list()
            .iter()
            .map(|info| ConnectedOutput {
                name: info.name.clone(),
                width: info.geometry.width,
                height: info.geometry.height,
            })
            .collect();
        if let Some(drm) = self.drm_backend.as_ref() {
            for disabled in &drm.disabled_outputs {
                if connected
                    .iter()
                    .any(|existing| existing.name == disabled.name)
                {
                    continue;
                }
                connected.push(ConnectedOutput {
                    name: disabled.name.clone(),
                    width: disabled.reserved_geometry_hint.width.max(1),
                    height: disabled.reserved_geometry_hint.height.max(1),
                });
            }
        }
        if connected.is_empty() {
            return;
        }

        let resolved = self.resolve_output_layout(&connected);
        let refresh_by_name: HashMap<String, Option<i32>> = self
            .output_registry
            .list()
            .iter()
            .map(|info| (info.name.clone(), info.refresh_millihz))
            .collect();

        for output in &resolved {
            let previous_entry = previous_entries
                .iter()
                .find(|entry| entry.name == output.name);
            let entry = self
                .output_config_entries
                .iter()
                .find(|candidate| candidate.name == output.name);
            let diff = detect_output_reload_diff(previous_entry, entry);

            if diff.mode_changed {
                if let Some(next_mode) = entry.and_then(|candidate| candidate.mode.as_ref()) {
                    let rebuild_ok = if let Some(drm) = self.drm_backend.as_mut() {
                        drm.rebuild_compositor_for_mode(
                            &self.display_handle,
                            &output.name,
                            next_mode,
                        )
                    } else {
                        tracing::debug!(
                            "output {} mode change skipped: drm backend not active",
                            output.name
                        );
                        false
                    };
                    if !rebuild_ok && self.drm_backend.is_some() {
                        tracing::warn!(
                            "output {} mode rebuild failed; keeping previous mode",
                            output.name
                        );
                    }
                } else {
                    tracing::warn!(
                        "output {} mode override was removed in reload; current mode kept until restart",
                        output.name
                    );
                }
            }
            if diff.enabled_changed {
                let now_enabled = entry.map(|candidate| candidate.enabled).unwrap_or(true);
                if now_enabled {
                    let pending = if let Some(drm) = self.drm_backend.as_mut() {
                        drm.enable_output_pull_pending(&output.name)
                    } else {
                        None
                    };
                    if let Some(pending) = pending {
                        if let Err(err) = self.build_and_register_disabled_output(&pending, output)
                        {
                            tracing::warn!(
                                "output {} live re-enable failed: {} — putting back in disabled list",
                                output.name,
                                err
                            );
                            if let Some(drm) = self.drm_backend.as_mut() {
                                drm.disabled_outputs.push(pending);
                            }
                        } else {
                            tracing::info!(
                                "output {} re-enabled live (was disabled in previous config)",
                                output.name
                            );
                        }
                    } else if self.drm_backend.is_none() {
                        tracing::warn!(
                            "output {} enable skipped: drm backend not active (winit?) — cannot apply live",
                            output.name
                        );
                    } else {
                        tracing::warn!(
                            "output {} flagged for enable but not in disabled_outputs (was likely never connected) — needs hotplug",
                            output.name
                        );
                    }
                } else if self.drm_backend.is_some() {
                    let disabled = self
                        .drm_backend
                        .as_mut()
                        .and_then(|drm| drm.disable_output(&output.name))
                        .is_some();
                    if disabled {
                        if let Some(removed_output) = self
                            .outputs
                            .iter()
                            .find(|candidate| candidate.name() == output.name)
                            .cloned()
                        {
                            for workspace_idx in 0..self.workspaces.count() {
                                self.workspaces
                                    .space_at_mut(workspace_idx)
                                    .unmap_output(&removed_output);
                            }
                            layer_map_for_output(&removed_output).cleanup();
                        }
                        self.outputs
                            .retain(|candidate| candidate.name() != output.name);
                        if let Some(removed_info) =
                            self.output_registry.remove_by_name(&output.name)
                        {
                            self.post_output_state_change(
                                "output-disabled-live",
                                Some(removed_info.id),
                                Some(&output.name),
                            );
                        } else {
                            self.sync_outputs_with_workspace_state();
                            self.mark_all_outputs_dirty("output-disabled-live");
                        }
                        tracing::info!("output {} disabled live", output.name);
                    }
                    continue;
                }
                if self.drm_backend.is_none() {
                    tracing::warn!(
                        "output {} enabled toggle skipped: drm backend not active (winit?)",
                        output.name
                    );
                } else {
                    tracing::debug!(
                        "output {} enabled toggle no-op: drm backend present but disable/enable path did not match",
                        output.name
                    );
                }
                continue;
            }

            let requested_scale = entry.map(|candidate| candidate.scale).unwrap_or(1.0);
            let scale_value = if !requested_scale.is_finite() || requested_scale <= 0.0 {
                tracing::warn!(
                    "output {} scale {:?} invalid during reload — using 1.0",
                    output.name,
                    requested_scale
                );
                1.0
            } else {
                requested_scale
            };
            let transform = entry
                .and_then(|candidate| candidate.transform.as_deref())
                .map(parse_output_transform)
                .unwrap_or(Transform::Normal);

            let _ = self.output_registry.reconfigure_by_name(
                &output.name,
                OutputReconfigure {
                    geometry: OutputGeometry {
                        x: output.x,
                        y: output.y,
                        width: output.width,
                        height: output.height,
                    },
                    scale: scale_value,
                    transform,
                    refresh_millihz: refresh_by_name.get(&output.name).copied().flatten(),
                    primary: Some(output.primary),
                },
            );

            let Some(smithay_output) = self
                .outputs
                .iter()
                .find(|candidate| candidate.name() == output.name)
                .cloned()
            else {
                tracing::warn!(
                    "output {} missing from smithay output list during live reload; registry updated only",
                    output.name
                );
                continue;
            };

            let scale = if (scale_value - scale_value.round()).abs() < f64::EPSILON {
                Scale::Integer(scale_value as i32)
            } else {
                Scale::Fractional(scale_value)
            };

            smithay_output.change_current_state(
                None,
                Some(transform),
                Some(scale),
                Some((output.x, output.y).into()),
            );
            self.workspaces
                .active_space_mut()
                .map_output(&smithay_output, (output.x, output.y));
        }

        self.sync_outputs_with_workspace_state();
        self.refresh_lock_focus();
        self.mark_all_outputs_dirty("output-layout-reapplied");
    }

    fn build_and_register_disabled_output(
        &mut self,
        pending: &DisabledDrmOutput,
        resolved: &super::ResolvedOutput,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let missing_backend_err = || Error::other("drm backend not active for live re-enable");
        let (device_fd, renderer_formats) = {
            let drm = self.drm_backend.as_ref().ok_or_else(missing_backend_err)?;
            let formats: HashSet<Format> = drm
                .renderer
                .egl_context()
                .dmabuf_render_formats()
                .iter()
                .cloned()
                .collect();
            let formats = crate::backend::drm::init_env::maybe_disable_modifiers(
                formats,
                crate::backend::drm::init_env::disable_drm_modifiers_requested(),
            );
            (drm.device_fd.clone(), formats)
        };

        let entry = self
            .output_config_entries
            .iter()
            .find(|candidate| candidate.name == pending.name);
        let mut drm = DrmDevice::new(device_fd.clone(), false)?.0;
        let connector_info = drm.get_connector(pending.connector, false)?;
        if connector_info.state() != smithay::reexports::drm::control::connector::State::Connected {
            return Err(Error::other(format!(
                "connector for output {} is not connected",
                pending.name
            ))
            .into());
        }
        let modes = connector_info.modes();
        let (mode, reason) = crate::backend::drm::mode_selection::select_mode_with_override(
            modes,
            entry.and_then(|candidate| candidate.mode.as_ref()),
            &pending.name,
        )
        .ok_or_else(|| Error::other(format!("no mode available for output {}", pending.name)))?;
        tracing::info!(
            "output {} live re-enable selected mode {}x{} reason={}",
            pending.name,
            mode.size().0,
            mode.size().1,
            reason
        );

        let transform = entry
            .and_then(|candidate| candidate.transform.as_deref())
            .map(parse_output_transform)
            .unwrap_or(Transform::Normal);
        let requested_scale = entry.map(|candidate| candidate.scale).unwrap_or(1.0);
        let scale_value = if !requested_scale.is_finite() || requested_scale <= 0.0 {
            tracing::warn!(
                "output {} scale {:?} invalid during live re-enable — using 1.0",
                pending.name,
                requested_scale
            );
            1.0
        } else {
            requested_scale
        };

        let phys_size = connector_info
            .size()
            .map_or((0, 0), |size| (size.0 as i32, size.1 as i32));
        let output = Output::new(
            pending.name.clone(),
            PhysicalProperties {
                size: phys_size.into(),
                subpixel: Subpixel::Unknown,
                make: "Unknown".into(),
                model: "Unknown".into(),
                serial_number: "Unknown".into(),
            },
        );
        let _global = output.create_global::<MeridianState>(&self.display_handle);
        let output_mode = OutputMode {
            size: (mode.size().0 as i32, mode.size().1 as i32).into(),
            refresh: crate::backend::drm::mode_selection::mode_refresh_millihz_with_fallback(mode),
        };
        let scale = if (scale_value - scale_value.round()).abs() < f64::EPSILON {
            Scale::Integer(scale_value as i32)
        } else {
            Scale::Fractional(scale_value)
        };
        output.change_current_state(
            Some(output_mode),
            Some(transform),
            Some(scale),
            Some((resolved.x, resolved.y).into()),
        );
        output.set_preferred(output_mode);

        let (compositor, _gbm) = crate::backend::drm::init::build_drm_compositor(
            crate::backend::drm::init::DrmCompositorBuildParams {
                state_display_handle: &self.display_handle,
                device_fd: device_fd.clone(),
                drm: &mut drm,
                crtc: pending.crtc,
                connector: pending.connector,
                mode,
                renderer_formats: &renderer_formats,
                output: &output,
                gbm: None,
            },
        )?;

        self.workspaces
            .active_space_mut()
            .map_output(&output, (resolved.x, resolved.y));
        self.outputs.push(output.clone());
        let output_id = self.handle_output_added_or_updated(OutputRegistration {
            name: output.name(),
            geometry: MeridianState::output_geometry_for_registry(
                resolved.x,
                resolved.y,
                mode.size().0 as i32,
                mode.size().1 as i32,
            ),
            scale: scale_value,
            transform,
            refresh_millihz: Some(
                crate::backend::drm::mode_selection::mode_refresh_millihz_with_fallback(mode),
            ),
        });

        let drm_backend = self
            .drm_backend
            .as_mut()
            .ok_or_else(|| Error::other("drm backend lost while finalizing live re-enable"))?;
        drm_backend.outputs.push(DrmOutput {
            output_id,
            output: output.clone(),
            compositor,
            crtc: pending.crtc,
            connector: pending.connector,
            wallpaper: None,
            frame_in_flight: false,
            needs_repaint: true,
            scratch_normal: Vec::new(),
            scratch_cursor: Vec::new(),
            scratch_final: Vec::new(),
            scratch_windows: Vec::new(),
            scratch_lower_layer_data: Vec::new(),
            scratch_upper_layer_data: Vec::new(),
            scratch_lower_layer_elements: Vec::new(),
            scratch_upper_layer_elements: Vec::new(),
        });
        drm_backend
            .dirty_stats
            .register_output(output_id, output.name());

        Ok(())
    }

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

        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(&display_handle, "seat-0");
        seat.add_keyboard(Default::default(), 200, 25).unwrap();
        seat.add_pointer();

        let loop_handle = event_loop.handle();
        let idle_notifier = IdleNotifierState::<Self>::new(&display_handle, loop_handle.clone());
        let idle_inhibit_state = IdleInhibitManagerState::new::<Self>(&display_handle);
        let session_lock_state =
            SessionLockManagerState::new::<Self, _>(&display_handle, |_client| true);
        let socket_name = Self::init_wayland_listener(display, event_loop)?;
        let loop_signal = event_loop.get_signal();

        let meridian_config = MeridianConfig::load();
        let output_config_entries = meridian_config.outputs.clone();
        let output_layout = super::OutputLayout::from_config_entries(&output_config_entries);
        let mut theme_manager = ThemeManager::new();
        let _ = apply_config_overrides(&mut theme_manager, &meridian_config);

        let mut wallpaper_manager = WallpaperManager::new();
        wallpaper_manager.apply_theme(theme_manager.current());

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
            idle_notifier,
            idle_inhibit_state,
            idle_inhibitors: IdleInhibitorSet::new(),
            session_lock_state,
            lock_manager: LockManager::new(),
            xwm: None,
            drm_backend: None,
            maximize_restore_locations: std::collections::HashMap::new(),
            half_snap_restore_locations: std::collections::HashMap::new(),
            active_window_snap_states: std::collections::HashMap::new(),
            minimized_windows: std::collections::HashMap::new(),
            xwayland_or_diag: std::collections::HashMap::new(),
            cursor_status: smithay::input::pointer::CursorImageStatus::default_named(),
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

    pub fn handle_output_removed(&mut self, id: OutputId) -> bool {
        let Some(removed) = self.output_registry.remove_by_id(id) else {
            return false;
        };
        tracing::info!(
            "output removed: id={} name={} geometry=({},{} {}x{})",
            removed.id.0,
            removed.name,
            removed.geometry.x,
            removed.geometry.y,
            removed.geometry.width,
            removed.geometry.height
        );
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

#[cfg(test)]
mod tests {
    use meridian_config::{
        CursorConfig, MeridianConfig, ThemeManager, WallpaperConfig, WallpaperMode,
    };

    use super::apply_config_overrides;

    #[test]
    fn apply_config_overrides_marks_cursor_change_when_cursor_override_differs() {
        let mut theme_manager = ThemeManager::new();
        let mut config = MeridianConfig::default();
        config.cursor = Some(CursorConfig {
            theme: "Breeze".to_string(),
            size: 32,
        });

        let changes = apply_config_overrides(&mut theme_manager, &config);

        assert!(!changes.theme_changed);
        assert!(changes.cursor_changed);
        assert!(!changes.wallpaper_changed);
        assert_eq!(theme_manager.current().config.cursor.theme, "Breeze");
        assert_eq!(theme_manager.current().config.cursor.size, 32);
    }

    #[test]
    fn apply_config_overrides_marks_wallpaper_change_and_updates_override() {
        let mut theme_manager = ThemeManager::new();
        let mut config = MeridianConfig::default();
        config.wallpaper = Some(WallpaperConfig {
            path: "/tmp/wallpaper.png".to_string(),
            mode: WallpaperMode::Tile,
        });

        let changes = apply_config_overrides(&mut theme_manager, &config);

        assert!(!changes.theme_changed);
        assert!(!changes.cursor_changed);
        assert!(changes.wallpaper_changed);
        let wallpaper = theme_manager
            .current()
            .config
            .wallpaper
            .as_ref()
            .expect("wallpaper override applied");
        assert_eq!(wallpaper.path, "/tmp/wallpaper.png");
        assert_eq!(wallpaper.mode, WallpaperMode::Tile);
    }

    #[test]
    fn apply_config_overrides_with_unknown_theme_keeps_current_theme_and_flags_unchanged() {
        let mut theme_manager = ThemeManager::new();
        let mut config = MeridianConfig::default();
        config.general.theme = "theme-that-does-not-exist".to_string();

        let before = theme_manager.current().name.clone();
        let changes = apply_config_overrides(&mut theme_manager, &config);

        assert_eq!(theme_manager.current().name, before);
        assert!(!changes.theme_changed);
        assert!(!changes.cursor_changed);
        assert!(!changes.wallpaper_changed);
    }
}
