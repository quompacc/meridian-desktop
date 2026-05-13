use std::{env, path::Path};

use smithay::backend::drm::DrmDeviceFd;
use smithay::reexports::drm::Device as _;
use tracing::{info, warn};

use super::{
    init_env::env_value_or_unset,
    mode_selection::{calculate_mode_refresh_millihz, mode_conservative_key},
};

pub(super) fn log_mode_details(
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

pub(super) fn log_connector_modes(
    label: &str,
    connector: smithay::reexports::drm::control::connector::Handle,
    modes: &[smithay::reexports::drm::control::Mode],
) {
    let preferred_count = modes
        .iter()
        .filter(|mode| {
            mode.mode_type()
                .contains(smithay::reexports::drm::control::ModeTypeFlags::PREFERRED)
        })
        .count();
    tracing::info!(
        "{} summary: connector={:?} mode_count={} preferred_count={}",
        label,
        connector,
        modes.len(),
        preferred_count
    );

    for (index, mode) in modes.iter().copied().enumerate() {
        let (hdisplay, vdisplay) = mode.size();
        let preferred = mode
            .mode_type()
            .contains(smithay::reexports::drm::control::ModeTypeFlags::PREFERRED);
        tracing::debug!(
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

pub(super) fn log_drm_startup_diagnostics(seat_name: &str) {
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

pub(super) fn check_drm_master_lock(
    device_fd: &DrmDeviceFd,
    gpu_path: &Path,
    seat_name: &str,
) -> bool {
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
