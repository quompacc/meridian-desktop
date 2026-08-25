use std::{
    collections::{HashMap, HashSet},
    env,
    os::unix::io::OwnedFd,
    time::Duration,
};

#[cfg(not(target_os = "openbsd"))]
use smithay::wayland::drm_syncobj::{supports_syncobj_eventfd, DrmSyncobjState};
#[cfg(not(target_os = "openbsd"))]
use smithay::{
    backend::libinput::{LibinputInputBackend, LibinputSessionInterface},
    reexports::input::Libinput,
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
        output_mode_infos, select_add_mode, select_mode_with_override,
    },
    render::{render_output_after_vblank, render_outputs},
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
    modes: Vec<smithay::reexports::drm::control::Mode>,
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
include!("init/build.rs");
include!("init/layout.rs");
include!("init/hotplug.rs");
include!("init/event_sources.rs");
#[allow(clippy::items_after_test_module)] // Split test submodule precedes this entry point.
pub fn init_drm(
    event_loop: &mut EventLoop<MeridianState>,
    state: &mut MeridianState,
) -> Result<(), Box<dyn std::error::Error>> {
    // Phase 8: ask meridian-login to release DRM master before we start the
    // libseat acquire chain. Best-effort: if we were not launched by
    // meridian-login the socket simply doesn't exist and we proceed.
    super::login_ipc::send_handover();

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

    #[cfg(target_os = "openbsd")]
    super::openbsd_privsep::install(session.clone());
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
            modes: modes.to_vec(),
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
        state
            .output_registry
            .set_modes_by_id(output_id, output_mode_infos(&pending.modes, pending.mode));

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
            modes: pending.modes.clone(),
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
            glass_pass_cache: Default::default(),
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
        let main_device = std::fs::metadata(&gpu_path).ok().and_then(|meta| {
            use std::os::unix::fs::MetadataExt;
            libc::dev_t::try_from(meta.rdev()).ok()
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

    #[cfg(not(target_os = "openbsd"))]
    if state.syncobj_state.is_none() {
        let device_fd_for_syncobj = device_fd.clone();
        if supports_syncobj_eventfd(&device_fd_for_syncobj) {
            state.syncobj_state = Some(DrmSyncobjState::new::<MeridianState>(
                &state.display_handle,
                device_fd_for_syncobj,
            ));
            tracing::info!("linux-drm-syncobj-v1 global registered (explicit-sync enabled)");
        } else {
            tracing::info!(
                "linux-drm-syncobj-v1 not enabled: kernel/driver does not support syncobj-eventfd"
            );
        }
    }

    #[cfg(target_os = "openbsd")]
    tracing::info!(
        "linux-drm-syncobj-v1 unavailable on OpenBSD: using native DRM without eventfd explicit sync"
    );

    state.drm_backend = Some(DrmBackend {
        device_fd: device_fd.clone(),
        kms_node_path: std::sync::Arc::from(gpu_path.display().to_string()),
        kms_is_primary_node: is_primary_node,
        kms_master_lock_ok: master_lock_ok,
        kms_first_commit_verified: false,
        renderer,
        outputs: drm_outputs,
        disabled_outputs,
        cursor_image,
        cursor_buffer,
        compositor_cursor_cache: std::collections::HashMap::new(),
        named_cursor_cache: std::collections::HashMap::new(),
        cursor_icon: super::DrmCursorIcon::Default,
        dirty_stats: super::DrmDirtyStats::new(dirty_stats_enabled),
        last_pointer_location: None,
        last_connector_scan: std::time::Instant::now(),
        timing_stats: super::DrmTimingStats::new(timing_enabled),
        repaint_idle_scheduled: false,
    });
    if let Some(drm) = state.drm_backend.as_mut() {
        for output in &drm.outputs {
            drm.dirty_stats
                .register_output(output.output_id, output.output.name());
        }
    }

    register_drm_event_source(event_loop, drm_notifier)?;
    register_repaint_timer_source(event_loop, repaint_interval)?;

    #[cfg(not(target_os = "openbsd"))]
    {
        let mut libinput = Libinput::new_with_udev(LibinputSessionInterface::from(session));
        libinput
            .udev_assign_seat(&seat_name)
            .map_err(|_| "libinput seat assignment failed")?;
        register_libinput_event_source(event_loop, libinput)?;
    }

    #[cfg(target_os = "openbsd")]
    super::wscons::register_wscons_event_sources(event_loop, &mut session)?;

    Ok(())
}
