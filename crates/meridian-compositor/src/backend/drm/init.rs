use std::{
    collections::{HashMap, HashSet},
    env,
    os::unix::io::OwnedFd,
    path::Path,
    time::Duration,
};

use smithay::{
    backend::{
        allocator::{
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
            Format, Fourcc, Modifier,
        },
        drm::{
            compositor::DrmCompositor,
            exporter::gbm::{GbmFramebufferExporter, NodeFilter},
            DrmDevice, DrmDeviceFd, DrmEvent,
        },
        egl::{EGLContext, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        session::{libseat::LibSeatSession, Event as SessionEvent, Session},
    },
    desktop::layer_map_for_output,
    output::{Mode as OutputMode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{
            timer::{TimeoutAction, Timer},
            EventLoop,
        },
        drm::control::Device as _,
        drm::Device as _,
        input::Libinput,
    },
    utils::{DeviceFd, Transform},
};
use tracing::{info, warn};

use crate::{
    cursor::CursorImage,
    state::{MeridianState, OutputReconfigure, OutputRegistration},
};

use super::{gpu::select_gpu, render::render_outputs, DrmBackend, DrmOutput};

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
                .collect::<Vec<_>>();
            let known_output_names = drm
                .outputs
                .iter()
                .map(|out| out.output.name())
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
        let mode = modes[0];
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

fn select_add_mode(
    modes: &[smithay::reexports::drm::control::Mode],
) -> Option<(smithay::reexports::drm::control::Mode, String)> {
    if env_flag_enabled("MERIDIAN_DRM_SAFE_MODE") {
        if let Some(mode) = select_safe_mode(modes) {
            return Some((mode, "safe-mode".to_string()));
        }
    }

    if let Some(index) = forced_mode_index_from_env() {
        if let Some(mode) = modes.get(index).copied() {
            return Some((mode, format!("forced-index({})", index)));
        }
        tracing::warn!(
            "MERIDIAN_DRM_MODE_INDEX={} out of range (available modes={})",
            index,
            modes.len()
        );
    }

    if let Some((force_w, force_h)) = forced_mode_size_from_env() {
        let same_size: Vec<_> = modes
            .iter()
            .copied()
            .filter(|mode| mode.size() == (force_w, force_h))
            .collect();
        if same_size.len() > 1 {
            tracing::info!(
                "drm mode candidates for forced-size {}x{}: count={} candidates={:?}",
                force_w,
                force_h,
                same_size.len(),
                same_size
                    .iter()
                    .map(|mode| mode_brief(*mode))
                    .collect::<Vec<_>>()
            );
        }
        if let Some(forced_mode) = select_best_mode_for_size(modes, force_w, force_h) {
            return Some((forced_mode, format!("forced-size({}x{})", force_w, force_h)));
        }
        tracing::warn!(
            "requested drm mode {}x{} not found in connector mode list",
            force_w,
            force_h
        );
    }
    if let Some(preferred) = modes.iter().copied().find(|mode| {
        mode.mode_type()
            .contains(smithay::reexports::drm::control::ModeTypeFlags::PREFERRED)
    }) {
        let (pref_w, pref_h) = preferred.size();
        let same_size: Vec<_> = modes
            .iter()
            .copied()
            .filter(|mode| mode.size() == (pref_w, pref_h))
            .collect();
        if same_size.len() > 1 {
            tracing::info!(
                "drm preferred-size candidates {}x{}: count={} candidates={:?}",
                pref_w,
                pref_h,
                same_size.len(),
                same_size
                    .iter()
                    .map(|mode| mode_brief(*mode))
                    .collect::<Vec<_>>()
            );
        }
        if let Some(selected_mode) = select_best_mode_for_size(modes, pref_w, pref_h) {
            if !same_mode(preferred, selected_mode) {
                tracing::info!(
                    "drm preferred mode adjusted to best same-size refresh candidate: preferred={} selected={}",
                    mode_brief(preferred),
                    mode_brief(selected_mode)
                );
            }
            return Some((selected_mode, "preferred-size-best-refresh".to_string()));
        }
        return Some((preferred, "preferred".to_string()));
    }
    modes
        .first()
        .copied()
        .map(|mode| (mode, "safe-fallback-first".to_string()))
}

fn mode_flags_weird_penalty(mode: smithay::reexports::drm::control::Mode) -> u8 {
    let flags = format!("{:?}", mode.flags()).to_ascii_uppercase();
    let mut penalty = 0_u8;
    for token in ["INTERLACE", "DBLSCAN", "DOUBLESCAN", "3D"] {
        if flags.contains(token) {
            penalty = penalty.saturating_add(1);
        }
    }
    penalty
}

fn calculate_mode_refresh_millihz(mode: smithay::reexports::drm::control::Mode) -> Option<i32> {
    let clock_khz = mode.clock();
    let htotal = mode.hsync().2 as i64;
    let vtotal = mode.vsync().2 as i64;
    if clock_khz == 0 || htotal <= 0 || vtotal <= 0 {
        return None;
    }

    let mut refresh_millihz = u128::from(clock_khz)
        .checked_mul(1_000_000)?
        .checked_div(htotal as u128)?
        .checked_div(vtotal as u128)?;

    let flags = mode.flags();
    if flags.contains(smithay::reexports::drm::control::ModeFlags::INTERLACE) {
        refresh_millihz = refresh_millihz.checked_mul(2)?;
    }
    if flags.contains(smithay::reexports::drm::control::ModeFlags::DBLSCAN) {
        refresh_millihz /= 2;
    }

    let vscan = mode.vscan() as u128;
    if vscan > 1 {
        refresh_millihz /= vscan;
    }

    i32::try_from(refresh_millihz).ok()
}

fn mode_refresh_millihz_with_fallback(mode: smithay::reexports::drm::control::Mode) -> i32 {
    calculate_mode_refresh_millihz(mode).unwrap_or_else(|| mode.vrefresh().max(0) as i32 * 1000)
}

fn mode_conservative_key(mode: smithay::reexports::drm::control::Mode) -> (u8, u32, u32, u8) {
    let refresh = mode.vrefresh().max(0) as u32;
    let exact_60_penalty = if refresh == 60 { 0 } else { 1 };
    let refresh_distance = refresh.abs_diff(60);
    (
        exact_60_penalty,
        refresh_distance,
        mode.clock(),
        mode_flags_weird_penalty(mode),
    )
}

fn same_mode(
    a: smithay::reexports::drm::control::Mode,
    b: smithay::reexports::drm::control::Mode,
) -> bool {
    a.size() == b.size()
        && a.clock() == b.clock()
        && a.vrefresh() == b.vrefresh()
        && a.flags() == b.flags()
        && a.mode_type() == b.mode_type()
}

fn mode_brief(mode: smithay::reexports::drm::control::Mode) -> String {
    let (w, h) = mode.size();
    let calc_refresh_millihz = calculate_mode_refresh_millihz(mode);
    format!(
        "{}x{}@{}Hz mclock_khz={} calc_refresh_millihz={:?} flags={:?} mode_type={:?}",
        w,
        h,
        mode.vrefresh(),
        mode.clock(),
        calc_refresh_millihz,
        mode.flags(),
        mode.mode_type()
    )
}

fn select_best_mode_for_size(
    modes: &[smithay::reexports::drm::control::Mode],
    width: u16,
    height: u16,
) -> Option<smithay::reexports::drm::control::Mode> {
    modes
        .iter()
        .copied()
        .filter(|mode| mode.size() == (width, height))
        .max_by_key(|mode| {
            (
                mode_refresh_millihz_with_fallback(*mode),
                u8::MAX - mode_flags_weird_penalty(*mode),
                mode.clock(),
            )
        })
}

fn select_safe_mode(
    modes: &[smithay::reexports::drm::control::Mode],
) -> Option<smithay::reexports::drm::control::Mode> {
    let mut candidates: Vec<_> = modes
        .iter()
        .copied()
        .filter(|mode| {
            let (w, h) = mode.size();
            w >= 1920 && h >= 1080
        })
        .collect();
    if candidates.is_empty() {
        candidates.extend(modes.iter().copied());
    }
    candidates.into_iter().min_by_key(|mode| {
        let (w, h) = mode.size();
        (
            w as u32 * h as u32,
            mode_conservative_key(*mode),
            mode.vrefresh().max(0) as u32,
            mode.clock(),
        )
    })
}

fn parse_mode_size(value: &str) -> Option<(u16, u16)> {
    let trimmed = value.trim().to_ascii_lowercase();
    let (w, h) = trimmed.split_once('x')?;
    let width = w.parse::<u16>().ok()?;
    let height = h.parse::<u16>().ok()?;
    Some((width, height))
}

fn forced_mode_size_from_env() -> Option<(u16, u16)> {
    if let Ok(value) = env::var("MERIDIAN_DRM_MODE") {
        return parse_mode_size(&value);
    }
    env::var("MERIDIAN_DRM_FORCE_MODE")
        .ok()
        .and_then(|value| parse_mode_size(&value))
}

fn forced_mode_index_from_env() -> Option<usize> {
    env::var("MERIDIAN_DRM_MODE_INDEX")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
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

    let modes = conn.modes();
    log_connector_modes("drm output add connector mode", connector, modes);
    let Some((mode, mode_reason)) = select_add_mode(modes) else {
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

    let surface = match drm.create_surface(crtc_handle, mode, &[connector]) {
        Ok(surface) => surface,
        Err(err) => {
            tracing::warn!(
                "drm output add skipped reason=create-surface-failed connector={:?} err={}",
                connector,
                err
            );
            return false;
        }
    };

    let gbm = match GbmDevice::new(device_fd.clone()) {
        Ok(gbm) => gbm,
        Err(err) => {
            tracing::warn!(
                "drm output add skipped reason=gbm-device-failed connector={:?} err={}",
                connector,
                err
            );
            return false;
        }
    };
    let allocator = GbmAllocator::new(
        gbm.clone(),
        GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
    );
    let exporter = GbmFramebufferExporter::new(gbm.clone(), NodeFilter::All);
    let force_format = forced_scanout_format_from_env();
    let color_formats = selected_scanout_formats(force_format);
    tracing::info!(
        "drm scanout format selection: source=hotplug-add forced_format={:?} selected={:?}",
        force_format,
        color_formats
    );

    let x_offset: i32 = state
        .outputs
        .iter()
        .map(|o| o.current_mode().map_or(0, |m| m.size.w))
        .sum();
    let output_name = format!("drm-{}", state.outputs.len());
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
        Some(Transform::Normal),
        None,
        Some((0, 0).into()),
    );
    output.set_preferred(out_mode);

    let compositor = match DrmCompositor::new(
        &output,
        surface,
        None,
        allocator,
        exporter,
        color_formats.clone(),
        renderer_formats.iter().cloned(),
        drm.cursor_size(),
        Some(gbm),
    ) {
        Ok(compositor) => compositor,
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
        .map_output(&output, (x_offset, 0));
    state.outputs.push(output.clone());

    let output_id = state.handle_output_added_or_updated(OutputRegistration {
        name: output_name,
        geometry: MeridianState::output_geometry_for_registry(
            x_offset,
            0,
            width as i32,
            height as i32,
        ),
        scale: 1.0,
        transform: Transform::Normal,
        refresh_millihz: Some(refresh_millihz),
    });

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

fn env_flag_enabled(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn force_drm_legacy_requested() -> bool {
    env_flag_enabled("MERIDIAN_DRM_FORCE_LEGACY")
}

fn disable_drm_modifiers_requested() -> bool {
    env_flag_enabled("MERIDIAN_DRM_DISABLE_MODIFIERS")
}

fn forced_scanout_format_from_env() -> Option<Fourcc> {
    let value = env::var("MERIDIAN_DRM_FORCE_FORMAT").ok()?;
    let normalized = value.trim().to_ascii_uppercase();
    match normalized.as_str() {
        "XRGB8888" => Some(Fourcc::Xrgb8888),
        "ARGB8888" => Some(Fourcc::Argb8888),
        _ => None,
    }
}

fn maybe_disable_modifiers(formats: HashSet<Format>, disable_modifiers: bool) -> HashSet<Format> {
    if !disable_modifiers {
        return formats;
    }
    formats
        .into_iter()
        .map(|format| Format {
            code: format.code,
            modifier: Modifier::Invalid,
        })
        .collect()
}

fn selected_scanout_formats(force_format: Option<Fourcc>) -> Vec<Fourcc> {
    match force_format {
        Some(format) => vec![format],
        None => vec![Fourcc::Argb8888, Fourcc::Xrgb8888],
    }
}

fn duration_from_hz(hz: u32) -> Option<Duration> {
    if hz == 0 {
        return None;
    }
    Some(Duration::from_nanos((1_000_000_000u64 / hz as u64).max(1)))
}

fn duration_from_millihz(millihz: i32) -> Option<Duration> {
    if millihz <= 0 {
        return None;
    }
    Some(Duration::from_nanos(
        ((1_000_000_000u128 * 1000) / millihz as u128) as u64,
    ))
}

fn select_repaint_interval(default: Duration, default_source: String) -> (Duration, String) {
    if let Ok(value) = env::var("MERIDIAN_DRM_FRAME_INTERVAL_MS") {
        match value.trim().parse::<u64>() {
            Ok(ms) if ms > 0 => {
                return (
                    Duration::from_millis(ms),
                    format!("env:MERIDIAN_DRM_FRAME_INTERVAL_MS({})", ms),
                );
            }
            _ => {
                tracing::warn!(
                    "invalid MERIDIAN_DRM_FRAME_INTERVAL_MS value {:?}; using default {:?}",
                    value,
                    default
                );
            }
        }
    }

    if let Ok(value) = env::var("MERIDIAN_DRM_FORCE_REFRESH_HZ") {
        match value.trim().parse::<u32>() {
            Ok(hz) if hz > 0 => {
                if let Some(interval) = duration_from_hz(hz) {
                    return (
                        interval,
                        format!("env:MERIDIAN_DRM_FORCE_REFRESH_HZ({})", hz),
                    );
                }
            }
            _ => {
                tracing::warn!(
                    "invalid MERIDIAN_DRM_FORCE_REFRESH_HZ value {:?}; using default {:?}",
                    value,
                    default
                );
            }
        }
    }

    (default, default_source)
}

fn log_mode_details(
    label: &str,
    connector: smithay::reexports::drm::control::connector::Handle,
    mode: smithay::reexports::drm::control::Mode,
) {
    let (hdisplay, vdisplay) = mode.size();
    let (hsync_start, hsync_end, htotal) = mode.hsync();
    let (vsync_start, vsync_end, vtotal) = mode.vsync();
    tracing::info!(
        "{}: connector={:?} name={} mclock_khz={} vrefresh_hz={} calc_refresh_millihz={:?} hdisplay={} hsync_start={} hsync_end={} htotal={} vdisplay={} vsync_start={} vsync_end={} vtotal={} flags={:?} mode_type={:?}",
        label,
        connector,
        mode.name().to_string_lossy(),
        mode.clock(),
        mode.vrefresh(),
        calculate_mode_refresh_millihz(mode),
        hdisplay,
        hsync_start,
        hsync_end,
        htotal,
        vdisplay,
        vsync_start,
        vsync_end,
        vtotal,
        mode.flags(),
        mode.mode_type(),
    );
}

fn log_connector_modes(
    label: &str,
    connector: smithay::reexports::drm::control::connector::Handle,
    modes: &[smithay::reexports::drm::control::Mode],
) {
    for (index, mode) in modes.iter().copied().enumerate() {
        let (hdisplay, vdisplay) = mode.size();
        let preferred = mode
            .mode_type()
            .contains(smithay::reexports::drm::control::ModeTypeFlags::PREFERRED);
        tracing::info!(
            "{}: connector={:?} index={} name={} hdisplay={} vdisplay={} vrefresh_hz={} calc_refresh_millihz={:?} mclock_khz={} flags={:?} mode_type={:?} preferred={} safe_mode_key={:?}",
            label,
            connector,
            index,
            mode.name().to_string_lossy(),
            hdisplay,
            vdisplay,
            mode.vrefresh(),
            calculate_mode_refresh_millihz(mode),
            mode.clock(),
            mode.flags(),
            mode.mode_type(),
            preferred,
            mode_conservative_key(mode),
        );
    }
}

fn env_value_or_unset(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| "<unset>".to_string())
}

fn log_drm_startup_diagnostics(seat_name: &str) {
    let session_id = env_value_or_unset("XDG_SESSION_ID");
    let session_type = env_value_or_unset("XDG_SESSION_TYPE");
    let session_seat = env_value_or_unset("XDG_SEAT");
    let session_vtnr = env_value_or_unset("XDG_VTNR");
    let libseat_backend = env::var("LIBSEAT_BACKEND").unwrap_or_else(|_| "auto".to_string());

    info!(
        "drm startup session context: backend=libseat libseat_backend={} seat={} xdg_session_id={} xdg_seat={} xdg_vtnr={} xdg_session_type={}",
        libseat_backend, seat_name, session_id, session_seat, session_vtnr, session_type
    );
}

fn check_drm_master_lock(device_fd: &DrmDeviceFd, gpu_path: &Path, seat_name: &str) -> bool {
    match device_fd.acquire_master_lock() {
        Ok(()) => {
            info!(
                "drm master acquired: node={} seat={}",
                gpu_path.display(),
                seat_name
            );
            true
        }
        Err(err) => {
            let is_render_node = gpu_path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("renderD"));
            warn!(
                "diagnostic drm master lock check failed: node={} seat={} xdg_session_id={} xdg_seat={} xdg_vtnr={} libseat_backend={} primary_node={} render_node={} err={}. \
this check is diagnostic only; functional KMS gate (surface creation + first commit) decides startup success.",
                gpu_path.display(),
                seat_name,
                env_value_or_unset("XDG_SESSION_ID"),
                env_value_or_unset("XDG_SEAT"),
                env_value_or_unset("XDG_VTNR"),
                env::var("LIBSEAT_BACKEND").unwrap_or_else(|_| "auto".to_string()),
                !is_render_node,
                is_render_node,
                err
            );
            false
        }
    }
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
    let egl_display = unsafe { EGLDisplay::new(gbm.clone())? };
    let context = EGLContext::new(&egl_display)?;
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
        let Some((mode, mode_reason)) = select_add_mode(modes) else {
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

        let surface = drm.create_surface(crtc_handle, mode, &[*conn_handle])?;
        let (mode_w, mode_h) = mode.size();
        info!(
            "drm kms surface created: connector={:?} crtc={:?} mode={}x{}@{}Hz calc_refresh_millihz={}",
            conn_handle,
            crtc_handle,
            mode_w,
            mode_h,
            mode.vrefresh(),
            selected_mode_refresh_millihz
        );
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

        let (w, h) = mode.size();
        let phys_size = conn.size().map_or((0, 0), |s| (s.0 as i32, s.1 as i32));
        let output = Output::new(
            format!("drm-{}", drm_outputs.len()),
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
            size: (w as i32, h as i32).into(),
            refresh: selected_mode_refresh_millihz,
        };
        output.change_current_state(
            Some(out_mode),
            Some(Transform::Normal),
            None,
            Some((0, 0).into()),
        );
        output.set_preferred(out_mode);

        let x_offset: i32 = drm_outputs
            .iter()
            .map(|o| o.output.current_mode().map_or(0, |m| m.size.w))
            .sum();
        state
            .workspaces
            .active_space_mut()
            .map_output(&output, (x_offset, 0));
        state.outputs.push(output.clone());
        let output_id = state.register_output_info(OutputRegistration {
            name: output.name(),
            geometry: MeridianState::output_geometry_for_registry(x_offset, 0, w as i32, h as i32),
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(selected_mode_refresh_millihz),
        });

        let compositor = DrmCompositor::new(
            &output,
            surface,
            None,
            allocator,
            exporter,
            color_formats.clone(),
            renderer_formats.iter().cloned(),
            drm.cursor_size(),
            Some(gbm.clone()),
        )?;

        drm_outputs.push(DrmOutput {
            output_id,
            output,
            compositor,
            crtc: crtc_handle,
            connector: *conn_handle,
            wallpaper: None,
            frame_in_flight: false,
            needs_repaint: true,
        });
        info!(
            "Initialized output {}x{} @ {}Hz (calc_refresh_millihz={})",
            w,
            h,
            mode.vrefresh(),
            selected_mode_refresh_millihz
        );
    }

    if drm_outputs.is_empty() {
        return Err("no connected displays found".into());
    }
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
        "drm repaint interval configured: interval_ms={} interval_ns={} source={} mode_refresh_hint_millihz={:?} mode_interval_hint_ms={:?}",
        repaint_interval.as_millis(),
        repaint_interval.as_nanos(),
        repaint_source,
        mode_refresh_hint_millihz,
        mode_interval_hint.map(|duration| duration.as_millis())
    );

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

    state.drm_backend = Some(DrmBackend {
        device_fd: device_fd.clone(),
        kms_node_path: gpu_path.display().to_string(),
        kms_is_primary_node: is_primary_node,
        kms_master_lock_ok: master_lock_ok,
        kms_first_commit_verified: false,
        renderer,
        outputs: drm_outputs,
        cursor_image,
        cursor_buffer,
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

    let mut libinput = Libinput::new_with_udev(LibinputSessionInterface::from(session));
    libinput
        .udev_assign_seat(&seat_name)
        .map_err(|_| "libinput seat assignment failed")?;

    event_loop
        .handle()
        .insert_source(LibinputInputBackend::new(libinput), |event, _, state| {
            state.process_input_event(event);
        })?;

    Ok(())
}
