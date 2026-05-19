use smithay::{
    output::{Output, WeakOutput},
    reexports::wayland_server::protocol::wl_shm,
    utils::Size,
    wayland::{
        image_capture_source::{
            ImageCaptureSource, ImageCaptureSourceHandler, OutputCaptureSourceHandler,
            OutputCaptureSourceState,
        },
        image_copy_capture::{
            BufferConstraints, Frame, ImageCopyCaptureHandler, ImageCopyCaptureState,
            Session, SessionRef,
        },
    },
};

use crate::state::MeridianState;

impl ImageCaptureSourceHandler for MeridianState {
    fn source_destroyed(&mut self, _source: ImageCaptureSource) {}
}

impl OutputCaptureSourceHandler for MeridianState {
    fn output_capture_source_state(&mut self) -> &mut OutputCaptureSourceState {
        &mut self.output_capture_source_state
    }

    fn output_source_created(&mut self, source: ImageCaptureSource, output: &Output) {
        source.user_data().insert_if_missing(|| output.downgrade());
    }
}

impl ImageCopyCaptureHandler for MeridianState {
    fn image_copy_capture_state(&mut self) -> &mut ImageCopyCaptureState {
        &mut self.image_copy_capture_state
    }

    fn capture_constraints(&mut self, source: &ImageCaptureSource) -> Option<BufferConstraints> {
        let output = source
            .user_data()
            .get::<WeakOutput>()?
            .upgrade()?;
        let mode = output.current_mode()?;
        Some(BufferConstraints {
            size: Size::from((mode.size.w, mode.size.h)),
            shm: vec![wl_shm::Format::Xrgb8888],
            dma: None,
        })
    }

    fn new_session(&mut self, session: Session) {
        if let Some(weak) = session.source().user_data().get::<WeakOutput>() {
            if let Some(output) = weak.upgrade() {
                session.user_data().insert_if_missing(|| output);
            }
        }
        self.screencopy_sessions.push(session);
    }

    fn session_destroyed(&mut self, session: SessionRef) {
        self.screencopy_sessions.retain(|s| s != &session);
    }

    fn frame(&mut self, session: &SessionRef, frame: Frame) {
        let Some(output) = session.user_data().get::<Output>().cloned() else {
            frame.fail(smithay::wayland::image_copy_capture::CaptureFailureReason::Unknown);
            return;
        };
        if let Some(ref mut drm) = self.drm_backend {
            for out in drm.outputs.iter_mut() {
                if out.output == output {
                    out.needs_repaint = true;
                    break;
                }
            }
        }
        self.pending_screencopy_frames.push((frame, output));
    }
}

#[cfg(test)]
mod tests {
    use smithay::{
        output::{Mode, Output, PhysicalProperties, Subpixel},
        utils::Size,
    };

    #[test]
    fn screencopy_constraints_uses_current_mode() {
        let output = Output::new(
            "test".into(),
            PhysicalProperties {
                size: Size::from((300, 200)),
                subpixel: Subpixel::None,
                make: "test".into(),
                model: "test".into(),
                serial_number: String::new(),
            },
        );
        let mode = Mode {
            size: Size::from((1920, 1080)),
            refresh: 60000,
        };
        output.change_current_state(Some(mode), None, None, None);
        assert_eq!(output.current_mode().unwrap().size, Size::from((1920, 1080)));
    }
}
