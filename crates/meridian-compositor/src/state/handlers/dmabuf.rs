use smithay::{
    backend::{allocator::dmabuf::Dmabuf, renderer::ImportDma},
    wayland::dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier},
};

use crate::state::MeridianState;

impl DmabufHandler for MeridianState {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        let Some(drm) = self.drm_backend.as_mut() else {
            tracing::warn!("dmabuf_imported but no DRM backend active - failing import");
            notifier.failed();
            return;
        };

        match drm.renderer.import_dmabuf(&dmabuf, None) {
            Ok(_texture) => {
                if notifier.successful::<MeridianState>().is_err() {
                    tracing::warn!("dmabuf import succeeded but notifying client failed");
                }
            }
            Err(err) => {
                tracing::warn!("dmabuf import via GlesRenderer failed: {}", err);
                notifier.failed();
            }
        }
    }
}
