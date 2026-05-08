use std::{collections::HashSet, os::unix::io::OwnedFd, path::{Path, PathBuf}, time::Duration};

use smithay::{
    backend::{
        allocator::{
            Format, Fourcc,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        },
        drm::{
            DrmDevice, DrmDeviceFd, DrmEvent,
            compositor::{DrmCompositor, FrameFlags},
            exporter::gbm::{GbmFramebufferExporter, NodeFilter},
        },
        egl::{EGLContext, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::gles::GlesRenderer,
        session::{Event as SessionEvent, Session, libseat::LibSeatSession},
        udev::{all_gpus, primary_gpu},
    },
    desktop::{
        Window,
        space::{SpaceRenderElements, space_render_elements},
    },
    output::{Mode as OutputMode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{EventLoop, timer::{Timer, TimeoutAction}},
        drm::control::{
            Device as _,
            ResourceHandles,
            connector::{self, State as ConnState},
            crtc,
        },
        input::Libinput,
    },
    utils::{DeviceFd, Transform},
};
use tracing::{error, info, warn};

use crate::state::MeridianState;

// ── Type aliases ─────────────────────────────────────────────────────────────

pub type GbmDrmCompositor = DrmCompositor<
    GbmAllocator<DrmDeviceFd>,
    GbmFramebufferExporter<DrmDeviceFd>,
    (),
    DrmDeviceFd,
>;

// ── State ─────────────────────────────────────────────────────────────────────

pub struct DrmOutput {
    pub output: Output,
    pub compositor: GbmDrmCompositor,
    pub crtc: crtc::Handle,
}

pub struct DrmBackend {
    pub renderer: GlesRenderer,
    pub outputs: Vec<DrmOutput>,
}

// ── Init ──────────────────────────────────────────────────────────────────────

pub fn init_drm(
    event_loop: &mut EventLoop<MeridianState>,
    state: &mut MeridianState,
) -> Result<(), Box<dyn std::error::Error>> {
    // ── Session ───────────────────────────────────────────────────────────────
    let (mut session, session_notifier) = LibSeatSession::new()?;
    let seat_name = session.seat();

    event_loop.handle().insert_source(session_notifier, |event, _, state| {
        if let SessionEvent::ActivateSession = event {
            if let Some(drm) = &mut state.drm_backend {
                for out in &mut drm.outputs {
                    out.compositor.reset_state().ok();
                }
            }
        }
    })?;

    // ── GPU selection ────────────────────────────────────────────────────────
    let gpu_path = select_gpu(&mut session, &seat_name)?;
    info!("Using GPU: {:?}", gpu_path);

    // ── Open DRM device ──────────────────────────────────────────────────────
    use smithay::reexports::rustix::fs::OFlags;
    let fd: OwnedFd = session.open(
        &gpu_path,
        OFlags::RDWR | OFlags::CLOEXEC | OFlags::NONBLOCK,
    )?;
    let device_fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (mut drm, drm_notifier) = DrmDevice::new(device_fd.clone(), false)?;

    // ── GBM + EGL + GLES ─────────────────────────────────────────────────────
    let gbm: GbmDevice<DrmDeviceFd> = GbmDevice::new(device_fd.clone())?;
    let egl_display = unsafe { EGLDisplay::new(gbm.clone())? };
    let context = EGLContext::new(&egl_display)?;
    let renderer = unsafe { GlesRenderer::new(context)? };

    let renderer_formats: HashSet<Format> = renderer
        .egl_context()
        .dmabuf_render_formats()
        .iter()
        .cloned()
        .collect();

    // ── Discover connected outputs ────────────────────────────────────────────
    let resources = drm.resource_handles()?;
    let mut drm_outputs: Vec<DrmOutput> = Vec::new();
    let mut occupied_crtcs: Vec<crtc::Handle> = Vec::new();

    for conn_handle in resources.connectors() {
        let conn = match drm.get_connector(*conn_handle, false) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if conn.state() != ConnState::Connected {
            continue;
        }
        let modes = conn.modes();
        if modes.is_empty() {
            continue;
        }
        let mode = modes[0];

        let crtc_handle = match pick_crtc(&drm, &resources, &conn, &occupied_crtcs) {
            Some(c) => c,
            None => {
                warn!("No free CRTC for connector {:?}", conn_handle);
                continue;
            }
        };
        occupied_crtcs.push(crtc_handle);

        let surface = drm.create_surface(crtc_handle, mode, &[*conn_handle])?;
        let allocator = GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
        let exporter = GbmFramebufferExporter::new(gbm.clone(), NodeFilter::All);
        let color_formats = [Fourcc::Argb8888, Fourcc::Xrgb8888];

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
            refresh: mode.vrefresh() as i32 * 1000,
        };
        output.change_current_state(Some(out_mode), Some(Transform::Normal), None, Some((0, 0).into()));
        output.set_preferred(out_mode);

        let x_offset: i32 = drm_outputs.iter()
            .map(|o| o.output.current_mode().map_or(0, |m| m.size.w))
            .sum();
        state.workspaces.active_space_mut().map_output(&output, (x_offset, 0));
        state.outputs.push(output.clone());

        let compositor = DrmCompositor::new(
            &output,
            surface,
            None,
            allocator,
            exporter,
            color_formats,
            renderer_formats.iter().cloned(),
            drm.cursor_size(),
            Some(gbm.clone()),
        )?;

        drm_outputs.push(DrmOutput { output, compositor, crtc: crtc_handle });
        info!("Initialized output {}x{} @ {}Hz", w, h, mode.vrefresh());
    }

    if drm_outputs.is_empty() {
        return Err("no connected displays found".into());
    }

    state.drm_backend = Some(DrmBackend { renderer, outputs: drm_outputs });

    // ── VBlank: only mark frame as submitted ──────────────────────────────────
    event_loop.handle().insert_source(drm_notifier, |event, _metadata, state| {
        let DrmEvent::VBlank(crtc) = event else { return; };
        if let Some(drm) = &mut state.drm_backend {
            if let Some(out) = drm.outputs.iter_mut().find(|o| o.crtc == crtc) {
                out.compositor.frame_submitted().ok();
            }
        }
    })?;

    // ── Timer: render all outputs at ~60fps ───────────────────────────────────
    event_loop.handle().insert_source(
        Timer::from_duration(Duration::from_millis(16)),
        |_instant, _metadata, state| {
            render_outputs(state);
            TimeoutAction::ToDuration(Duration::from_millis(16))
        },
    )?;

    // ── Libinput ──────────────────────────────────────────────────────────────
    let mut libinput = Libinput::new_with_udev(LibinputSessionInterface::from(session));
    libinput.udev_assign_seat(&seat_name).map_err(|_| "libinput seat assignment failed")?;

    event_loop.handle().insert_source(LibinputInputBackend::new(libinput), |event, _, state| {
        state.process_input_event(event);
    })?;

    Ok(())
}

fn render_outputs(state: &mut MeridianState) {
    let mut drm = match state.drm_backend.take() {
        Some(d) => d,
        None => return,
    };

    let DrmBackend { ref mut renderer, ref mut outputs } = drm;

    for out in outputs.iter_mut() {
        let elements = space_render_elements::<GlesRenderer, Window, _>(
            renderer,
            [state.workspaces.active_space()],
            &out.output,
            1.0,
        ).unwrap_or_default();

        let n_elements = elements.len();

        let bg = state.theme_manager.current().config.colors.background.as_f32_array();
        match out.compositor.render_frame::<GlesRenderer, SpaceRenderElements<GlesRenderer, _>>(
            renderer,
            &elements,
            bg,
            FrameFlags::DEFAULT,
        ) {
            Ok(frame) if !frame.is_empty => {
                let mode_str = out.output.current_mode()
                    .map_or_else(|| "?".to_string(), |m| {
                        format!("{}x{}@{}Hz", m.size.w, m.size.h, m.refresh / 1000)
                    });
                info!("Frame rendered: output={} mode={} elements={}", out.output.name(), mode_str, n_elements);
                out.compositor.queue_frame(()).ok();
            }
            Ok(_) => {}
            Err(err) => error!("DRM render error on {}: {}", out.output.name(), err),
        }

        let time = state.start_time.elapsed();
        let out_clone = out.output.clone();
        state.workspaces.active_space().elements().for_each(|w| {
            w.send_frame(&out_clone, time, Some(Duration::ZERO), |_, _| Some(out_clone.clone()));
        });
    }

    state.workspaces.active_space_mut().refresh();
    state.popups.cleanup();
    let _ = state.display_handle.flush_clients();
    state.drm_backend = Some(drm);
}

// ── GPU selection ─────────────────────────────────────────────────────────────

fn select_gpu(
    session: &mut LibSeatSession,
    seat_name: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Explicit override
    if let Ok(path) = std::env::var("MERIDIAN_DRM_DEVICE") {
        info!("Using GPU from MERIDIAN_DRM_DEVICE: {}", path);
        return Ok(PathBuf::from(path));
    }

    // Scan all GPUs and pick the first with connected outputs
    let gpus = all_gpus(seat_name).unwrap_or_default();
    info!("Detected {} GPU(s): {:?}", gpus.len(), gpus);

    for path in &gpus {
        match probe_gpu_connectors(session, path) {
            Ok(true) => {
                info!("Selected GPU with connected outputs: {:?}", path);
                return Ok(path.clone());
            }
            Ok(false) => info!("GPU {:?}: no connected outputs, skipping", path),
            Err(e) => warn!("GPU {:?}: probe failed ({}), skipping", path, e),
        }
    }

    // Fall back to primary GPU
    if let Ok(Some(path)) = primary_gpu(seat_name) {
        warn!("No GPU with connected outputs found, falling back to primary: {:?}", path);
        return Ok(path);
    }

    gpus.into_iter().next().ok_or_else(|| "no GPU found".into())
}

fn probe_gpu_connectors(
    session: &mut LibSeatSession,
    path: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    use smithay::reexports::rustix::fs::OFlags;
    let fd: OwnedFd = session.open(path, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NONBLOCK)?;
    let device_fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (drm, _notifier) = DrmDevice::new(device_fd, false)?;
    let resources = drm.resource_handles()?;
    for conn in resources.connectors() {
        if let Ok(info) = drm.get_connector(*conn, false) {
            if info.state() == ConnState::Connected {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn pick_crtc(
    drm: &DrmDevice,
    resources: &ResourceHandles,
    connector: &connector::Info,
    occupied: &[crtc::Handle],
) -> Option<crtc::Handle> {
    for encoder_handle in connector.encoders() {
        if let Ok(encoder) = drm.get_encoder(*encoder_handle) {
            for crtc_h in resources.filter_crtcs(encoder.possible_crtcs()) {
                if !occupied.contains(&crtc_h) {
                    return Some(crtc_h);
                }
            }
        }
    }
    None
}
