use std::{
    os::fd::{AsFd, BorrowedFd},
    os::unix::io::OwnedFd,
    path::{Path, PathBuf},
};

#[cfg(not(target_os = "openbsd"))]
use smithay::backend::udev::{all_gpus, primary_gpu};
use smithay::{
    backend::{
        drm::DrmDevice,
        session::{libseat::LibSeatSession, Session},
    },
    reexports::drm::control::{
        connector::{self, State as ConnState},
        crtc, Device as _, ResourceHandles,
    },
};
use tracing::{info, warn};

pub(super) fn select_gpu(
    session: &mut LibSeatSession,
    seat_name: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Ok(path) = std::env::var("MERIDIAN_DRM_DEVICE") {
        info!("Using GPU from MERIDIAN_DRM_DEVICE: {}", path);
        return Ok(PathBuf::from(path));
    }

    let gpus = gpu_candidates(seat_name);
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

    if let Some(path) = primary_gpu_candidate(seat_name) {
        warn!(
            "No GPU with connected outputs found, falling back to primary: {:?}",
            path
        );
        return Ok(path);
    }

    gpus.into_iter().next().ok_or_else(|| "no GPU found".into())
}

#[cfg(not(target_os = "openbsd"))]
fn gpu_candidates(seat_name: &str) -> Vec<PathBuf> {
    all_gpus(seat_name).unwrap_or_default()
}

#[cfg(not(target_os = "openbsd"))]
fn primary_gpu_candidate(seat_name: &str) -> Option<PathBuf> {
    primary_gpu(seat_name).ok().flatten()
}

#[cfg(target_os = "openbsd")]
fn gpu_candidates(_seat_name: &str) -> Vec<PathBuf> {
    let mut candidates = std::fs::read_dir("/dev/dri")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.strip_prefix("card").is_some_and(|suffix| {
                        !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit())
                    })
                })
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates
}

#[cfg(target_os = "openbsd")]
fn primary_gpu_candidate(_seat_name: &str) -> Option<PathBuf> {
    let card0 = PathBuf::from("/dev/dri/card0");
    card0.exists().then_some(card0)
}

fn probe_gpu_connectors(
    session: &mut LibSeatSession,
    path: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    use smithay::reexports::rustix::fs::OFlags;

    let fd: OwnedFd = session.open(
        path,
        OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
    )?;

    // Run the probe on a borrowed fd so we can hand `fd` back to
    // session.close() afterwards. Without that explicit close, libseat
    // keeps the device registered in its internal map and the next
    // session.open(same path) in init.rs returns FailedToOpenDevice
    // (EAGAIN) — dropping the OwnedFd only closes the kernel fd, not
    // the libseat-side tracking entry.
    let result = probe_connected(fd.as_fd());

    if let Err(e) = session.close(fd) {
        warn!("session.close after probe failed: {:?}", e);
    }

    result
}

fn probe_connected(fd: BorrowedFd<'_>) -> Result<bool, Box<dyn std::error::Error>> {
    struct ProbeDrmDevice<'a>(BorrowedFd<'a>);
    impl AsFd for ProbeDrmDevice<'_> {
        fn as_fd(&self) -> BorrowedFd<'_> {
            self.0
        }
    }
    impl smithay::reexports::drm::Device for ProbeDrmDevice<'_> {}
    impl smithay::reexports::drm::control::Device for ProbeDrmDevice<'_> {}

    let probe = ProbeDrmDevice(fd);
    let resources = probe.resource_handles()?;
    for conn in resources.connectors() {
        if let Ok(info) = probe.get_connector(*conn, false) {
            if info.state() == ConnState::Connected {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub(super) fn pick_crtc(
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
