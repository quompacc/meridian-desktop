use std::{
    collections::{HashMap, HashSet},
    env,
    os::unix::io::OwnedFd,
    time::Duration,
};

use smithay::{
    backend::{
        allocator::{
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
            Format, Modifier,
        },
        drm::{
            compositor::DrmCompositor,
            exporter::gbm::{GbmFramebufferExporter, NodeFilter},
            DrmDevice, DrmDeviceFd, DrmEvent,
        },
        egl::{EGLContext, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::ImportDma,
        session::{libseat::LibSeatSession, Event as SessionEvent, Session},
    },
    desktop::layer_map_for_output,
    output::{Mode as OutputMode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::{
            timer::{TimeoutAction, Timer},
            EventLoop,
        },
        drm::control::Device as _,
        input::Libinput,
    },
    utils::{DeviceFd, Transform},
    wayland::dmabuf::DmabufFeedbackBuilder,
};
use tracing::{info, warn};

use crate::{
    cursor::CursorImage,
    state::{
        parse_output_transform, ConnectedOutput, MeridianState, OutputReconfigure,
        OutputRegistration, ResolvedOutput,
    },
};

use super::{
    gpu::select_gpu,
    init_diagnostics::{
        check_drm_master_lock, log_connector_modes, log_drm_startup_diagnostics, log_mode_details,
    },
    init_env::{
        disable_drm_modifiers_requested, duration_from_millihz, env_flag_enabled,
        force_drm_legacy_requested, forced_scanout_format_from_env, maybe_disable_modifiers,
        select_repaint_interval, selected_scanout_formats,
    },
    mode_selection::{
        forced_mode_index_from_env, forced_mode_size_from_env, mode_refresh_millihz_with_fallback,
        select_add_mode, select_mode_with_override,
    },
    render::render_outputs,
    DisabledDrmOutput, DrmBackend, DrmOutput,
};

#[derive(Debug, Clone, Copy)]
struct DrmConnectorReconfigureCandidate {
    output_id: crate::state::OutputId,
    geometry_x: i32,
    geometry_y: i32,
    width: i32,
    height: i32,
    refresh_millihz: i32,
}

#[derive(Debug, Default)]
struct DrmConnectorChangeSet {
    reconfigure: Vec<DrmConnectorReconfigureCandidate>,
    add: Vec<smithay::reexports::drm::control::connector::Handle>,
    remove: Vec<DrmConnectorRemoveCandidate>,
}

#[derive(Debug, Clone, Copy)]
struct DrmConnectorRemoveCandidate {
    connector: smithay::reexports::drm::control::connector::Handle,
    output_id: Option<crate::state::OutputId>,
}

#[derive(Debug, Clone)]
struct PendingInitOutput {
    output_name: String,
    connector: smithay::reexports::drm::control::connector::Handle,
    crtc: smithay::reexports::drm::control::crtc::Handle,
    mode: smithay::reexports::drm::control::Mode,
    width: i32,
    height: i32,
    refresh_millihz: i32,
    phys_size: (i32, i32),
    transform: Transform,
    scale: f64,
}

pub(crate) struct DrmCompositorBuildParams<'a> {
    pub state_display_handle: &'a smithay::reexports::wayland_server::DisplayHandle,
    pub device_fd: DrmDeviceFd,
    pub drm: &'a mut DrmDevice,
    pub crtc: smithay::reexports::drm::control::crtc::Handle,
    pub connector: smithay::reexports::drm::control::connector::Handle,
    pub mode: smithay::reexports::drm::control::Mode,
    pub renderer_formats: &'a HashSet<Format>,
    pub output: &'a Output,
    pub gbm: Option<GbmDevice<DrmDeviceFd>>,
}

pub(crate) fn build_drm_compositor(
    params: DrmCompositorBuildParams<'_>,
) -> Result<(super::GbmDrmCompositor, GbmDevice<DrmDeviceFd>), Box<dyn std::error::Error>> {
    let DrmCompositorBuildParams {
        device_fd,
        drm,
        crtc,
        connector,
        mode,
        renderer_formats,
        output,
        gbm,
        state_display_handle: _state_display_handle,
    } = params;

    let surface = drm.create_surface(crtc, mode, &[connector])?;
    let (mode_w, mode_h) = mode.size();
    info!(
        "drm kms surface created: connector={:?} crtc={:?} mode={}x{}@{}Hz calc_refresh_millihz={}",
        connector,
        crtc,
        mode_w,
        mode_h,
        mode.vrefresh(),
        mode_refresh_millihz_with_fallback(mode)
    );

    let gbm = match gbm {
        Some(existing) => existing,
        None => GbmDevice::new(device_fd.clone())?,
    };
    let allocator = GbmAllocator::new(
        gbm.clone(),
        GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
    );
    let exporter = GbmFramebufferExporter::new(gbm.clone(), NodeFilter::All);
    let force_format = forced_scanout_format_from_env();
    let color_formats = selected_scanout_formats(force_format);
    info!(
        "drm compositor format assumptions: forced_format={:?} color_formats={:?}",
        force_format, color_formats
    );

    let compositor = DrmCompositor::new(
        output,
        surface,
        None,
        allocator,
        exporter,
        color_formats,
        renderer_formats.iter().cloned(),
        drm.cursor_size(),
        Some(gbm.clone()),
    )?;

    Ok((compositor, gbm))
}

fn sync_primary_flags_from_resolved_layout(state: &mut MeridianState, resolved: &[ResolvedOutput]) {
    let current_by_name: HashMap<_, _> = state
        .output_registry
        .list()
        .iter()
        .map(|info| {
            (
                info.name.clone(),
                (
                    info.geometry,
                    info.scale,
                    info.transform,
                    info.refresh_millihz,
                ),
            )
        })
        .collect();

    for output in resolved {
        let Some((geometry, scale, transform, refresh_millihz)) = current_by_name.get(&output.name)
        else {
            continue;
        };
        let _ = state.output_registry.reconfigure_by_name(
            &output.name,
            OutputReconfigure {
                geometry: *geometry,
                scale: *scale,
                transform: *transform,
                refresh_millihz: *refresh_millihz,
                primary: Some(output.primary),
            },
        );
    }
}

fn parse_output_scale(scale_value: f64, output_name: &str) -> f64 {
    if !scale_value.is_finite() || scale_value <= 0.0 {
        tracing::warn!(
            "output {} scale {:?} invalid — using 1.0",
            output_name,
            scale_value
        );
        return 1.0;
    }

    scale_value
}

fn output_scale_from_value(scale_value: f64) -> Scale {
    if (scale_value - scale_value.round()).abs() < f64::EPSILON {
        return Scale::Integer(scale_value as i32);
    }

    Scale::Fractional(scale_value)
}

fn classify_drm_connector_changes(
    known_connectors: &[(smithay::reexports::drm::control::connector::Handle, String)],
    connected_modes: &HashMap<smithay::reexports::drm::control::connector::Handle, (i32, i32, i32)>,
    registry_by_name: &HashMap<String, (crate::state::OutputId, i32, i32, i32, i32, i32)>,
) -> DrmConnectorChangeSet {
    let mut changes = DrmConnectorChangeSet::default();
    let known_set: HashSet<_> = known_connectors.iter().map(|(conn, _)| *conn).collect();

    for (connector, name) in known_connectors {
        let Some((new_w, new_h, new_refresh)) = connected_modes.get(connector).copied() else {
            changes.remove.push(DrmConnectorRemoveCandidate {
                connector: *connector,
                output_id: registry_by_name.get(name).map(|(id, _, _, _, _, _)| *id),
            });
            continue;
        };
        let Some((id, x, y, old_w, old_h, old_refresh)) = registry_by_name.get(name).copied()
        else {
            continue;
        };
        if old_w != new_w || old_h != new_h || old_refresh != new_refresh {
            changes.reconfigure.push(DrmConnectorReconfigureCandidate {
                output_id: id,
                geometry_x: x,
                geometry_y: y,
                width: new_w,
                height: new_h,
                refresh_millihz: new_refresh,
            });
        }
    }

    for connector in connected_modes.keys() {
        if !known_set.contains(connector) {
            changes.add.push(*connector);
        }
    }

    changes
}

fn scan_drm_connectors_for_h5b(state: &mut MeridianState, source: &str) {
    tracing::trace!("drm connector scan triggered: source={}", source);
    let (should_scan, device_fd, known_connectors, known_output_names) =
        if let Some(drm) = state.drm_backend.as_mut() {
            let should_scan = drm.last_connector_scan.elapsed() >= Duration::from_millis(750);
            if should_scan {
                drm.last_connector_scan = std::time::Instant::now();
            } else {
                tracing::trace!(
                    "drm connector scan skipped: source={} reason=throttled",
                    source
                );
            }
            let known_connectors = drm
                .outputs
                .iter()
                .map(|out| (out.connector, out.output.name()))
                .chain(
                    drm.disabled_outputs
                        .iter()
                        .map(|out| (out.connector, out.name.clone())),
                )
                .collect::<Vec<_>>();
            let known_output_names = drm
                .outputs
                .iter()
                .map(|out| out.output.name())
                .chain(drm.disabled_outputs.iter().map(|out| out.name.clone()))
                .collect::<HashSet<_>>();
            (
                should_scan,
                drm.device_fd.clone(),
                known_connectors,
                known_output_names,
            )
        } else {
            return;
        };

    if !should_scan {
        return;
    }

    let resources = match device_fd.resource_handles() {
        Ok(resources) => resources,
        Err(err) => {
            tracing::warn!("drm hotplug scan failed to read resource handles: {}", err);
            return;
        }
    };

    let mut connected_modes = HashMap::new();
    for conn_handle in resources.connectors() {
        let Ok(conn) = device_fd.get_connector(*conn_handle, false) else {
            continue;
        };
        if conn.state() != smithay::reexports::drm::control::connector::State::Connected {
            continue;
        }
        let modes = conn.modes();
        if modes.is_empty() {
            continue;
        }
        let Some((mode, _mode_reason)) = select_add_mode(modes) else {
            tracing::trace!(
                "drm hotplug scan skipped connector without selectable mode: connector={:?}",
                conn_handle
            );
            continue;
        };
        let (w, h) = mode.size();
        connected_modes.insert(
            *conn_handle,
            (w as i32, h as i32, mode_refresh_millihz_with_fallback(mode)),
        );
    }

    let registry_by_name = state
        .output_registry
        .list()
        .iter()
        .filter(|info| known_output_names.contains(&info.name))
        .map(|info| {
            (
                info.name.clone(),
                (
                    info.id,
                    info.geometry.x,
                    info.geometry.y,
                    info.geometry.width,
                    info.geometry.height,
                    info.refresh_millihz.unwrap_or_default(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();

    let changes =
        classify_drm_connector_changes(&known_connectors, &connected_modes, &registry_by_name);

    for candidate in changes.reconfigure {
        tracing::debug!(
            "drm connector reconfigure detected: output_id={} width={} height={} refresh={}",
            candidate.output_id.0,
            candidate.width,
            candidate.height,
            candidate.refresh_millihz
        );
        if state.handle_output_reconfigured(
            candidate.output_id,
            OutputReconfigure {
                geometry: MeridianState::output_geometry_for_registry(
                    candidate.geometry_x,
                    candidate.geometry_y,
                    candidate.width,
                    candidate.height,
                ),
                scale: 1.0,
                transform: Transform::Normal,
                refresh_millihz: Some(candidate.refresh_millihz),
                primary: None,
            },
        ) {
            tracing::debug!(
                "drm output reconfigured via hotplug pipeline: output_id={}",
                candidate.output_id.0
            );
        }
    }

    for connector in changes.add {
        tracing::info!("drm output add detected: connector={:?}", connector);
        if add_drm_output_via_hotplug_pipeline(state, connector) {
            tracing::info!(
                "drm output added via hotplug pipeline: connector={:?}",
                connector
            );
        }
    }

    for candidate in changes.remove {
        tracing::info!(
            "drm output remove detected: connector={:?} output_id={:?}",
            candidate.connector,
            candidate.output_id.map(|id| id.0)
        );
        if remove_drm_output_via_hotplug_pipeline(state, candidate) {
            tracing::info!(
                "drm output removed via hotplug pipeline: connector={:?} output_id={:?}",
                candidate.connector,
                candidate.output_id.map(|id| id.0)
            );
        }
    }
}

fn add_drm_output_via_hotplug_pipeline(
    state: &mut MeridianState,
    connector: smithay::reexports::drm::control::connector::Handle,
) -> bool {
    let (device_fd, occupied_crtcs, renderer_formats) = {
        let Some(drm) = state.drm_backend.as_ref() else {
            tracing::warn!("drm output add skipped reason=drm-backend-missing");
            return false;
        };
        let renderer_formats: HashSet<Format> = drm
            .renderer
            .egl_context()
            .dmabuf_render_formats()
            .iter()
            .cloned()
            .collect();
        let renderer_formats =
            maybe_disable_modifiers(renderer_formats, disable_drm_modifiers_requested());
        (
            drm.device_fd.clone(),
            drm.outputs
                .iter()
                .map(|output| output.crtc)
                .collect::<Vec<_>>(),
            renderer_formats,
        )
    };

    let (mut drm, _notifier) = match DrmDevice::new(device_fd.clone(), false) {
        Ok(pair) => pair,
        Err(err) => {
            tracing::warn!(
                "drm output add skipped reason=device-open-failed connector={:?} err={}",
                connector,
                err
            );
            return false;
        }
    };

    let resources = match drm.resource_handles() {
        Ok(resources) => resources,
        Err(err) => {
            tracing::warn!(
                "drm output add skipped reason=resource-handles-failed connector={:?} err={}",
                connector,
                err
            );
            return false;
        }
    };

    let conn = match drm.get_connector(connector, false) {
        Ok(conn) => conn,
        Err(err) => {
            tracing::warn!(
                "drm output add skipped reason=connector-query-failed connector={:?} err={}",
                connector,
                err
            );
            return false;
        }
    };
    if conn.state() != smithay::reexports::drm::control::connector::State::Connected {
        tracing::warn!(
            "drm output add skipped reason=connector-not-connected connector={:?}",
            connector
        );
        return false;
    }

    let output_name = format!("drm-{}", state.outputs.len());
    let output_mode_override = state
        .output_config_entries
        .iter()
        .find(|entry| entry.name == output_name)
        .and_then(|entry| entry.mode.clone());
    let output_transform_override = state
        .output_config_entries
        .iter()
        .find(|entry| entry.name == output_name)
        .and_then(|entry| entry.transform.clone());
    let output_scale_override = state
        .output_config_entries
        .iter()
        .find(|entry| entry.name == output_name)
        .map(|entry| entry.scale)
        .unwrap_or(1.0);

    let modes = conn.modes();
    log_connector_modes("drm output add connector mode", connector, modes);
    let Some((mode, mode_reason)) =
        select_mode_with_override(modes, output_mode_override.as_ref(), &output_name)
    else {
        tracing::warn!(
            "drm output add skipped reason=no-mode connector={:?}",
            connector
        );
        return false;
    };
    let (width, height) = mode.size();
    let refresh_millihz = mode_refresh_millihz_with_fallback(mode);
    log_mode_details("drm output add selected mode details", connector, mode);
    tracing::debug!(
        "drm output add selected mode: connector={:?} mode={}x{} refresh={} reason={}",
        connector,
        width,
        height,
        refresh_millihz,
        mode_reason
    );

    let Some(crtc_handle) = super::gpu::pick_crtc(&drm, &resources, &conn, &occupied_crtcs) else {
        tracing::warn!(
            "drm output add skipped reason=no-free-crtc connector={:?}",
            connector
        );
        return false;
    };

    let mut pending: Vec<ConnectedOutput> = state
        .output_registry
        .list()
        .iter()
        .map(|info| ConnectedOutput {
            name: info.name.clone(),
            width: info.geometry.width,
            height: info.geometry.height,
        })
        .collect();
    pending.push(ConnectedOutput {
        name: output_name.clone(),
        width: width as i32,
        height: height as i32,
    });
    let resolved = state.resolve_output_layout(&pending);
    let new_resolved = resolved
        .iter()
        .find(|entry| entry.name == output_name)
        .expect("new output in resolver result");
    let (x, y) = (new_resolved.x, new_resolved.y);
    tracing::debug!(
        "resolved layout for hotplug output {}: x={} y={}",
        output_name,
        x,
        y
    );
    if !new_resolved.enabled {
        let disabled = DisabledDrmOutput {
            name: new_resolved.name.clone(),
            connector,
            crtc: crtc_handle,
            reserved_geometry_hint: MeridianState::output_geometry_for_registry(
                new_resolved.x,
                new_resolved.y,
                width as i32,
                height as i32,
            ),
        };
        if let Some(drm) = state.drm_backend.as_mut() {
            drm.disabled_outputs
                .retain(|existing| existing.name != new_resolved.name);
            drm.disabled_outputs.push(disabled);
        }
        tracing::info!(
            "hotplugged connector {:?} is disabled per TOML config; stored as disabled_output",
            connector
        );
        return true;
    }

    let transform = output_transform_override
        .as_deref()
        .map(parse_output_transform)
        .unwrap_or(Transform::Normal);
    if transform != Transform::Normal {
        tracing::debug!(
            "output {} uses non-normal transform {:?}; geometry tracking remains mode-size based",
            output_name,
            transform
        );
    }
    let scale_value = parse_output_scale(output_scale_override, &output_name);
    let output_scale = output_scale_from_value(scale_value);

    let phys_size = conn.size().map_or((0, 0), |s| (s.0 as i32, s.1 as i32));
    let output = Output::new(
        output_name.clone(),
        PhysicalProperties {
            size: phys_size.into(),
            subpixel: Subpixel::Unknown,
            make: "Unknown".into(),
            model: "Unknown".into(),
            serial_number: "Unknown".into(),
        },
    );
    let _global = output.create_global::<MeridianState>(&state.display_handle);
    let out_mode = OutputMode {
        size: (width as i32, height as i32).into(),
        refresh: refresh_millihz,
    };
    output.change_current_state(
        Some(out_mode),
        Some(transform),
        Some(output_scale),
        Some((x, y).into()),
    );
    output.set_preferred(out_mode);

    let compositor = match build_drm_compositor(DrmCompositorBuildParams {
        state_display_handle: &state.display_handle,
        device_fd: device_fd.clone(),
        drm: &mut drm,
        crtc: crtc_handle,
        connector,
        mode,
        renderer_formats: &renderer_formats,
        output: &output,
        gbm: None,
    }) {
        Ok((compositor, _gbm)) => compositor,
        Err(err) => {
            tracing::warn!(
                "drm output add skipped reason=compositor-create-failed connector={:?} err={}",
                connector,
                err
            );
            return false;
        }
    };

    state
        .workspaces
        .active_space_mut()
        .map_output(&output, (x, y));
    state.outputs.push(output.clone());

    let output_id = state.handle_output_added_or_updated(OutputRegistration {
        name: output_name.clone(),
        geometry: MeridianState::output_geometry_for_registry(x, y, width as i32, height as i32),
        scale: scale_value,
        transform,
        refresh_millihz: Some(refresh_millihz),
    });
    sync_primary_flags_from_resolved_layout(state, &resolved);

    let Some(drm_backend) = state.drm_backend.as_mut() else {
        tracing::warn!(
            "drm output add skipped reason=drm-backend-lost-after-state-update connector={:?}",
            connector
        );
        return false;
    };
    let output_name = output.name();
    drm_backend.outputs.push(DrmOutput {
        output_id,
        output,
        compositor,
        crtc: crtc_handle,
        connector,
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
        .register_output(output_id, output_name);

    tracing::debug!(
        "drm output added via hotplug pipeline details: connector={:?} output_id={}",
        connector,
        output_id.0
    );
    true
}

fn remove_drm_output_via_hotplug_pipeline(
    state: &mut MeridianState,
    candidate: DrmConnectorRemoveCandidate,
) -> bool {
    let Some((removed_output, removed_output_name)) = detach_drm_output(state, candidate.connector)
    else {
        if let Some(drm) = state.drm_backend.as_mut() {
            if let Some(disabled_idx) = drm
                .disabled_outputs
                .iter()
                .position(|output| output.connector == candidate.connector)
            {
                let removed_disabled = drm.disabled_outputs.remove(disabled_idx);
                tracing::info!(
                    "drm disabled output removed via hotplug pipeline: connector={:?} output={}",
                    candidate.connector,
                    removed_disabled.name
                );
                return true;
            }
        }
        tracing::warn!(
            "drm output remove skipped reason=connector-not-found connector={:?}",
            candidate.connector
        );
        return false;
    };

    for workspace_idx in 0..state.workspaces.count() {
        state
            .workspaces
            .space_at_mut(workspace_idx)
            .unmap_output(&removed_output);
    }
    layer_map_for_output(&removed_output).cleanup();

    if let Some(idx) = state
        .outputs
        .iter()
        .position(|output| output.name() == removed_output_name)
    {
        state.outputs.remove(idx);
    } else {
        tracing::warn!(
            "drm output remove skipped reason=state-output-not-found output={}",
            removed_output_name
        );
    }

    let output_id = candidate.output_id.or_else(|| {
        state
            .output_registry
            .list()
            .iter()
            .find(|info| info.name == removed_output_name)
            .map(|info| info.id)
    });

    let Some(output_id) = output_id else {
        tracing::warn!(
            "drm output remove skipped reason=registry-output-id-missing output={}",
            removed_output_name
        );
        return false;
    };

    if !state.handle_output_removed(output_id) {
        tracing::warn!(
            "drm output remove skipped reason=handle-output-removed-failed output_id={}",
            output_id.0
        );
        return false;
    }

    true
}

fn detach_drm_output(
    state: &mut MeridianState,
    connector: smithay::reexports::drm::control::connector::Handle,
) -> Option<(Output, String)> {
    let drm = state.drm_backend.as_mut()?;
    let idx = drm
        .outputs
        .iter()
        .position(|output| output.connector == connector)?;
    let removed = drm.outputs.remove(idx);
    drm.dirty_stats.unregister_output(removed.output_id);
    let name = removed.output.name();
    Some((removed.output, name))
}

fn configure_repaint_interval(
    drm_outputs: &[DrmOutput],
    first_selected_mode_refresh_millihz: Option<i32>,
) -> Duration {
    let mode_refresh_hint_millihz = first_selected_mode_refresh_millihz.or_else(|| {
        drm_outputs
            .first()
            .and_then(|output| output.output.current_mode().map(|mode| mode.refresh))
    });
    let mode_interval_hint = mode_refresh_hint_millihz.and_then(duration_from_millihz);
    let default_repaint_interval = mode_interval_hint.unwrap_or_else(|| Duration::from_millis(16));
    let default_repaint_source = mode_refresh_hint_millihz
        .map(|millihz| format!("default:calculated-mode-refresh({millihz}mHz)"))
        .unwrap_or_else(|| "default:hardcoded-16ms".to_string());
    let (repaint_interval, repaint_source) =
        select_repaint_interval(default_repaint_interval, default_repaint_source);
    tracing::info!(
        "drm repaint scheduler interval configured (timer-only, not KMS mode forcing): interval_ms={} interval_ns={} source={} mode_refresh_hint_millihz={:?} mode_interval_hint_ms={:?}",
        repaint_interval.as_millis(),
        repaint_interval.as_nanos(),
        repaint_source,
        mode_refresh_hint_millihz,
        mode_interval_hint.map(|duration| duration.as_millis())
    );
    repaint_interval
}

fn register_drm_event_source<Source>(
    event_loop: &mut EventLoop<MeridianState>,
    drm_notifier: Source,
) -> Result<(), Box<dyn std::error::Error>>
where
    Source: smithay::reexports::calloop::EventSource<Event = DrmEvent, Ret = ()> + 'static,
    Source::Metadata: 'static,
    Source::Error: std::error::Error + 'static,
{
    event_loop
        .handle()
        .insert_source(drm_notifier, |event, _metadata, state| match event {
            DrmEvent::VBlank(crtc) => {
                let vblank_event_at = std::time::Instant::now();
                if let Some(drm) = &mut state.drm_backend {
                    let handler_started = std::time::Instant::now();
                    let mut frame_submitted_duration = Duration::ZERO;
                    let mut matched_output = false;
                    if let Some(out) = drm.outputs.iter_mut().find(|o| o.crtc == crtc) {
                        matched_output = true;
                        let frame_submitted_started = std::time::Instant::now();
                        if let Err(err) = out.compositor.frame_submitted() {
                            tracing::warn!(
                                "drm frame_submitted failed on output {}: {}",
                                out.output.name(),
                                err
                            );
                            out.frame_in_flight = false;
                        } else {
                            out.frame_in_flight = false;
                        }
                        frame_submitted_duration = frame_submitted_started.elapsed();
                    }
                    drm.timing_stats.record_vblank(
                        vblank_event_at,
                        handler_started.elapsed(),
                        frame_submitted_duration,
                        matched_output,
                    );
                }
                tracing::trace!("drm vblank event received: crtc={:?}", crtc);
                scan_drm_connectors_for_h5b(state, "vblank");
            }
            DrmEvent::Error(err) => {
                tracing::warn!("drm device error event received: err={}", err);
                scan_drm_connectors_for_h5b(state, "error");
            }
        })?;
    Ok(())
}

fn register_repaint_timer_source(
    event_loop: &mut EventLoop<MeridianState>,
    repaint_interval: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    event_loop.handle().insert_source(
        Timer::from_duration(repaint_interval),
        move |timer_fired_at, _metadata, state| {
            let tick_started = std::time::Instant::now();
            let metrics = render_outputs(state);
            if let Some(drm) = state.drm_backend.as_mut() {
                drm.timing_stats.record_render_tick(
                    timer_fired_at,
                    tick_started,
                    tick_started.elapsed(),
                    metrics,
                );
                drm.dirty_stats.report_if_due(tick_started);
            }
            TimeoutAction::ToDuration(repaint_interval)
        },
    )?;
    Ok(())
}

fn register_libinput_event_source(
    event_loop: &mut EventLoop<MeridianState>,
    libinput: Libinput,
) -> Result<(), Box<dyn std::error::Error>> {
    event_loop
        .handle()
        .insert_source(LibinputInputBackend::new(libinput), |event, _, state| {
            state.process_input_event(event);
        })?;
    Ok(())
}

pub fn init_drm(
    event_loop: &mut EventLoop<MeridianState>,
    state: &mut MeridianState,
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut session, session_notifier) = LibSeatSession::new()?;
    let seat_name = session.seat();
    log_drm_startup_diagnostics(&seat_name);
    info!(
        "drm session initialized: backend=libseat seat={}",
        seat_name
    );
    if let Ok(backend_override) = env::var("LIBSEAT_BACKEND") {
        info!("libseat backend override: {}", backend_override);
    }
    if let Ok(session_type) = env::var("XDG_SESSION_TYPE") {
        info!("xdg session type: {}", session_type);
    }
    if env_flag_enabled("SMITHAY_USE_LEGACY") {
        warn!("SMITHAY_USE_LEGACY is enabled; atomic drm path is forced off");
    }
    if force_drm_legacy_requested() {
        // Smithay reads this env var inside DrmDevice::new() to force legacy KMS path.
        // SAFETY: this process-scoped env mutation is intentional during one-time backend initialization.
        unsafe {
            env::set_var("SMITHAY_USE_LEGACY", "1");
        }
        info!("MERIDIAN_DRM_FORCE_LEGACY requested: SMITHAY_USE_LEGACY=1 applied");
    }
    if let Some((w, h)) = forced_mode_size_from_env() {
        info!("drm mode override requested: {}x{}", w, h);
    }
    if let Some(index) = forced_mode_index_from_env() {
        info!("drm mode index override requested: {}", index);
    }

    event_loop
        .handle()
        .insert_source(session_notifier, |event, _, state| {
            if let SessionEvent::ActivateSession = event {
                if let Some(drm) = &mut state.drm_backend {
                    for out in &mut drm.outputs {
                        out.compositor.reset_state().ok();
                    }
                }
            }
        })?;

    let gpu_path = select_gpu(&mut session, &seat_name)?;
    let is_primary_node = gpu_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("card"));
    info!(
        "selected drm node: path={} session_managed_open=true primary_node={}",
        gpu_path.display(),
        is_primary_node
    );

    use smithay::reexports::rustix::fs::OFlags;
    let fd: OwnedFd = session.open(
        &gpu_path,
        OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
    )?;
    info!("drm session-opened fd path: {}", gpu_path.display());
    let device_fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let master_lock_ok = check_drm_master_lock(&device_fd, &gpu_path, &seat_name);
    let (mut drm, drm_notifier) = DrmDevice::new(device_fd.clone(), false)?;

    let gbm: GbmDevice<DrmDeviceFd> = GbmDevice::new(device_fd.clone())?;
    // SAFETY: `gbm` is a live GBM device tied to the opened DRM fd.
    let egl_display = unsafe { EGLDisplay::new(gbm.clone())? };
    let context = EGLContext::new(&egl_display)?;
    // SAFETY: `context` is freshly created from the current EGL display and valid for renderer creation.
    let renderer = unsafe { smithay::backend::renderer::gles::GlesRenderer::new(context)? };

    let disable_modifiers = disable_drm_modifiers_requested();
    let renderer_formats: HashSet<Format> = renderer
        .egl_context()
        .dmabuf_render_formats()
        .iter()
        .cloned()
        .collect();
    let renderer_formats = maybe_disable_modifiers(renderer_formats, disable_modifiers);
    let mut renderer_format_list: Vec<String> = renderer_formats
        .iter()
        .map(|format| format!("{:?}", format))
        .collect();
    renderer_format_list.sort();
    let renderer_format_preview = renderer_format_list
        .iter()
        .take(6)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    info!(
        "drm renderer dmabuf formats: count={} preview=[{}] disable_modifiers={} forced_modifier={:?}",
        renderer_formats.len(),
        renderer_format_preview,
        disable_modifiers,
        if disable_modifiers {
            Some(Modifier::Invalid)
        } else {
            None
        }
    );

    let resources = drm.resource_handles()?;
    let mut drm_outputs: Vec<DrmOutput> = Vec::new();
    let mut disabled_outputs: Vec<DisabledDrmOutput> = Vec::new();
    let mut pending_outputs: Vec<PendingInitOutput> = Vec::new();
    let mut occupied_crtcs: Vec<smithay::reexports::drm::control::crtc::Handle> = Vec::new();
    let mut first_selected_mode_refresh_millihz: Option<i32> = None;

    for conn_handle in resources.connectors() {
        let conn = match drm.get_connector(*conn_handle, false) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if conn.state() != smithay::reexports::drm::control::connector::State::Connected {
            continue;
        }
        let modes = conn.modes();
        if modes.is_empty() {
            continue;
        }
        log_connector_modes("drm connector mode", *conn_handle, modes);
        let output_name = format!("drm-{}", pending_outputs.len());
        let config_entry = state
            .output_config_entries
            .iter()
            .find(|entry| entry.name == output_name);
        let mode_override = config_entry.and_then(|entry| entry.mode.as_ref());
        let Some((mode, mode_reason)) =
            select_mode_with_override(modes, mode_override, &output_name)
        else {
            continue;
        };
        log_mode_details("drm mode selected", *conn_handle, mode);
        let selected_mode_refresh_millihz = mode_refresh_millihz_with_fallback(mode);
        if first_selected_mode_refresh_millihz.is_none() {
            first_selected_mode_refresh_millihz = Some(selected_mode_refresh_millihz);
        }
        tracing::info!(
            "drm mode selection reason: connector={:?} reason={}",
            conn_handle,
            mode_reason
        );

        let crtc_handle = match super::gpu::pick_crtc(&drm, &resources, &conn, &occupied_crtcs) {
            Some(c) => c,
            None => {
                warn!("No free CRTC for connector {:?}", conn_handle);
                continue;
            }
        };
        occupied_crtcs.push(crtc_handle);

        let (w, h) = mode.size();
        let phys_size = conn.size().map_or((0, 0), |s| (s.0 as i32, s.1 as i32));
        let transform = config_entry
            .and_then(|entry| entry.transform.as_deref())
            .map(parse_output_transform)
            .unwrap_or(Transform::Normal);
        let scale = parse_output_scale(
            config_entry.map(|entry| entry.scale).unwrap_or(1.0),
            &output_name,
        );
        pending_outputs.push(PendingInitOutput {
            output_name,
            connector: *conn_handle,
            crtc: crtc_handle,
            mode,
            width: w as i32,
            height: h as i32,
            refresh_millihz: selected_mode_refresh_millihz,
            phys_size,
            transform,
            scale,
        });
    }

    if pending_outputs.is_empty() {
        return Err("no connected displays found".into());
    }

    let pending_connected = pending_outputs
        .iter()
        .map(|pending| ConnectedOutput {
            name: pending.output_name.clone(),
            width: pending.width,
            height: pending.height,
        })
        .collect::<Vec<_>>();
    let resolved_layout = state.resolve_output_layout(&pending_connected);

    for (pending, resolved) in pending_outputs.iter().zip(resolved_layout.iter()) {
        tracing::debug!(
            "resolved layout for init output {}: x={} y={}",
            pending.output_name,
            resolved.x,
            resolved.y
        );
        if !resolved.enabled {
            tracing::info!(
                "output {} skipped at init: enabled=false in TOML",
                pending.output_name
            );
            disabled_outputs.push(DisabledDrmOutput {
                name: pending.output_name.clone(),
                connector: pending.connector,
                crtc: pending.crtc,
                reserved_geometry_hint: MeridianState::output_geometry_for_registry(
                    resolved.x,
                    resolved.y,
                    resolved.width,
                    resolved.height,
                ),
            });
            continue;
        }
        if pending.transform != Transform::Normal {
            tracing::debug!(
                "output {} uses non-normal transform {:?}; geometry tracking remains mode-size based",
                pending.output_name,
                pending.transform
            );
        }
        let output = Output::new(
            pending.output_name.clone(),
            PhysicalProperties {
                size: pending.phys_size.into(),
                subpixel: Subpixel::Unknown,
                make: "Unknown".into(),
                model: "Unknown".into(),
                serial_number: "Unknown".into(),
            },
        );
        let _global = output.create_global::<MeridianState>(&state.display_handle);
        let out_mode = OutputMode {
            size: (pending.width, pending.height).into(),
            refresh: pending.refresh_millihz,
        };
        output.change_current_state(
            Some(out_mode),
            Some(pending.transform),
            Some(output_scale_from_value(pending.scale)),
            Some((resolved.x, resolved.y).into()),
        );
        output.set_preferred(out_mode);

        state
            .workspaces
            .active_space_mut()
            .map_output(&output, (resolved.x, resolved.y));
        state.outputs.push(output.clone());
        let output_id = state.register_output_info(OutputRegistration {
            name: output.name(),
            geometry: MeridianState::output_geometry_for_registry(
                resolved.x,
                resolved.y,
                pending.width,
                pending.height,
            ),
            scale: pending.scale,
            transform: pending.transform,
            refresh_millihz: Some(pending.refresh_millihz),
        });

        let (compositor, _gbm) = build_drm_compositor(DrmCompositorBuildParams {
            state_display_handle: &state.display_handle,
            device_fd: device_fd.clone(),
            drm: &mut drm,
            crtc: pending.crtc,
            connector: pending.connector,
            mode: pending.mode,
            renderer_formats: &renderer_formats,
            output: &output,
            gbm: Some(gbm.clone()),
        })?;

        drm_outputs.push(DrmOutput {
            output_id,
            output,
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
        info!(
            "Initialized output {}x{} @ {}Hz (calc_refresh_millihz={})",
            pending.width,
            pending.height,
            pending.mode.vrefresh(),
            pending.refresh_millihz
        );
    }

    sync_primary_flags_from_resolved_layout(state, &resolved_layout);
    let repaint_interval =
        configure_repaint_interval(&drm_outputs, first_selected_mode_refresh_millihz);

    let force_legacy = force_drm_legacy_requested();
    info!(
        "drm api selected: path={} (atomic={})",
        if drm.is_atomic() {
            "atomic"
        } else if force_legacy {
            "legacy-forced"
        } else {
            "legacy"
        },
        drm.is_atomic()
    );

    let cursor_config = &state.theme_manager.current().config.cursor;
    if env::var_os("XCURSOR_THEME").is_none() && !cursor_config.theme.is_empty() {
        env::set_var("XCURSOR_THEME", &cursor_config.theme);
    }
    if env::var_os("XCURSOR_SIZE").is_none() {
        env::set_var("XCURSOR_SIZE", cursor_config.size.to_string());
    }

    let cursor_theme = env::var("XCURSOR_THEME").unwrap_or_else(|_| cursor_config.theme.clone());
    let cursor_size = env::var("XCURSOR_SIZE")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(cursor_config.size);
    tracing::debug!(
        "cursor theme loaded: theme={} size={}",
        cursor_theme,
        cursor_size
    );
    let cursor_image = CursorImage::load_theme(&cursor_theme, cursor_size);
    info!(
        "Setting cursor: width={} height={} hotspot={},{}",
        cursor_image.width, cursor_image.height, cursor_image.xhot, cursor_image.yhot
    );
    let cursor_buffer = cursor_image.to_memory_buffer();
    let timing_enabled = env_flag_enabled("MERIDIAN_DRM_TIMING");
    let dirty_stats_enabled = env_flag_enabled("MERIDIAN_DIRTY_STATS");

    if state.dmabuf_global.is_none() {
        let dmabuf_formats: Vec<_> = renderer.dmabuf_formats().into_iter().collect();
        let main_device = std::fs::metadata(&gpu_path).ok().map(|meta| {
            use std::os::unix::fs::MetadataExt;
            meta.rdev()
        });

        if let Some(main_device) = main_device {
            match DmabufFeedbackBuilder::new(main_device, dmabuf_formats.clone()).build() {
                Ok(feedback) => {
                    let global = state
                        .dmabuf_state
                        .create_global_with_default_feedback::<MeridianState>(
                            &state.display_handle,
                            &feedback,
                        );
                    state.dmabuf_global = Some(global);
                    state.dmabuf_default_feedback = Some(feedback);
                    tracing::info!(
                        "linux-dmabuf-v1 global registered (v4 + feedback, main_device=0x{:x})",
                        main_device
                    );
                }
                Err(err) => {
                    tracing::warn!(
                        "dmabuf feedback build failed: {} - falling back to v3 global",
                        err
                    );
                    let global = state
                        .dmabuf_state
                        .create_global::<MeridianState>(&state.display_handle, dmabuf_formats);
                    state.dmabuf_global = Some(global);
                    state.dmabuf_default_feedback = None;
                }
            }
        } else {
            tracing::warn!(
                "failed to stat kms_node_path={:?} - registering v3 dmabuf global without feedback",
                gpu_path
            );
            let global = state
                .dmabuf_state
                .create_global::<MeridianState>(&state.display_handle, dmabuf_formats);
            state.dmabuf_global = Some(global);
            state.dmabuf_default_feedback = None;
        }
    }

    state.drm_backend = Some(DrmBackend {
        device_fd: device_fd.clone(),
        kms_node_path: gpu_path.display().to_string(),
        kms_is_primary_node: is_primary_node,
        kms_master_lock_ok: master_lock_ok,
        kms_first_commit_verified: false,
        renderer,
        outputs: drm_outputs,
        disabled_outputs,
        cursor_image,
        cursor_buffer,
        named_cursor_cache: std::collections::HashMap::new(),
        cursor_icon: super::DrmCursorIcon::Default,
        dirty_stats: super::DrmDirtyStats::new(dirty_stats_enabled),
        last_pointer_location: None,
        last_connector_scan: std::time::Instant::now(),
        timing_stats: super::DrmTimingStats::new(timing_enabled),
    });
    if let Some(drm) = state.drm_backend.as_mut() {
        for output in &drm.outputs {
            drm.dirty_stats
                .register_output(output.output_id, output.output.name());
        }
    }

    register_drm_event_source(event_loop, drm_notifier)?;
    register_repaint_timer_source(event_loop, repaint_interval)?;

    let mut libinput = Libinput::new_with_udev(LibinputSessionInterface::from(session));
    libinput
        .udev_assign_seat(&seat_name)
        .map_err(|_| "libinput seat assignment failed")?;

    register_libinput_event_source(event_loop, libinput)?;

    Ok(())
}
