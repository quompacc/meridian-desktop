use tracing::{info, warn};

use crate::state::MeridianState;

pub(super) fn resume_after_sleep(state: &mut MeridianState) {
    let Some(drm) = state.drm_backend.as_mut() else {
        return;
    };
    for output in &mut drm.outputs {
        if let Err(error) = output.compositor.reset_state() {
            warn!(%error, output = %output.output.name(), "failed to reset DRM state after resume");
        }
        output.frame_in_flight = false;
        output.needs_repaint = true;
    }
    info!("OpenBSD DRM outputs resumed");
}
