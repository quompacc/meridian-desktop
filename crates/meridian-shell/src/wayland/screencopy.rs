use std::{
    os::unix::io::{AsFd, AsRawFd, FromRawFd, OwnedFd},
    path::PathBuf,
};

use wayland_client::{
    protocol::{wl_buffer::WlBuffer, wl_shm, wl_shm_pool::WlShmPool},
    Connection, Dispatch, QueueHandle, WEnum,
};
use wayland_protocols::ext::{
    image_capture_source::v1::client::{
        ext_image_capture_source_v1::{self, ExtImageCaptureSourceV1},
        ext_output_image_capture_source_manager_v1::{self, ExtOutputImageCaptureSourceManagerV1},
    },
    image_copy_capture::v1::client::{
        ext_image_copy_capture_frame_v1::{self, ExtImageCopyCaptureFrameV1},
        ext_image_copy_capture_manager_v1::{self, ExtImageCopyCaptureManagerV1},
        ext_image_copy_capture_session_v1::{self, ExtImageCopyCaptureSessionV1},
    },
};

use super::shell::MeridianShell;

/// State for a single in-flight screenshot capture.
pub(crate) struct ScreenshotCapture {
    pub session: ExtImageCopyCaptureSessionV1,
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub format: Option<wl_shm::Format>,
    pub constraints_done: bool,
    pub pool: Option<WlShmPool>,
    pub buffer: Option<WlBuffer>,
    pub frame: Option<ExtImageCopyCaptureFrameV1>,
    pub fd: Option<OwnedFd>,
    pub mapped_ptr: *mut libc::c_void,
    pub mapped_len: usize,
}

// SAFETY: the mmap pointer is only accessed from the single-threaded Wayland event loop.
unsafe impl Send for ScreenshotCapture {}
unsafe impl Sync for ScreenshotCapture {}

impl Drop for ScreenshotCapture {
    fn drop(&mut self) {
        if !self.mapped_ptr.is_null() && self.mapped_len > 0 {
            unsafe { libc::munmap(self.mapped_ptr, self.mapped_len) };
            self.mapped_ptr = std::ptr::null_mut();
        }
        if let Some(frame) = self.frame.take() {
            frame.destroy();
        }
        if let Some(buffer) = self.buffer.take() {
            buffer.destroy();
        }
        if let Some(pool) = self.pool.take() {
            pool.destroy();
        }
        self.session.destroy();
    }
}

// ──── Dispatch: trivial globals (no incoming events) ────────────────────────

impl Dispatch<ExtImageCopyCaptureManagerV1, ()> for MeridianShell {
    fn event(
        _state: &mut Self,
        _proxy: &ExtImageCopyCaptureManagerV1,
        _event: ext_image_copy_capture_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtOutputImageCaptureSourceManagerV1, ()> for MeridianShell {
    fn event(
        _state: &mut Self,
        _proxy: &ExtOutputImageCaptureSourceManagerV1,
        _event: ext_output_image_capture_source_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtImageCaptureSourceV1, ()> for MeridianShell {
    fn event(
        _state: &mut Self,
        _proxy: &ExtImageCaptureSourceV1,
        _event: ext_image_capture_source_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlShmPool, ()> for MeridianShell {
    fn event(
        _state: &mut Self,
        _proxy: &WlShmPool,
        _event: wayland_client::protocol::wl_shm_pool::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlBuffer, ()> for MeridianShell {
    fn event(
        _state: &mut Self,
        _proxy: &WlBuffer,
        _event: wayland_client::protocol::wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

// ──── Dispatch: session events ───────────────────────────────────────────────

impl Dispatch<ExtImageCopyCaptureSessionV1, ()> for MeridianShell {
    fn event(
        state: &mut Self,
        _proxy: &ExtImageCopyCaptureSessionV1,
        event: ext_image_copy_capture_session_v1::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use ext_image_copy_capture_session_v1::Event;
        let Some(cap) = state.screenshot_capture.as_mut() else {
            return;
        };
        match event {
            Event::BufferSize { width, height } => {
                cap.width = width;
                cap.height = height;
            }
            Event::ShmFormat { format } => {
                if let (WEnum::Value(f), None) = (format, cap.format) {
                    cap.format = Some(f);
                }
            }
            Event::Done => {
                cap.constraints_done = true;
                issue_frame_capture(state, qh);
            }
            Event::Stopped => {
                state.screenshot_capture = None;
            }
            _ => {}
        }
    }
}

fn issue_frame_capture(state: &mut MeridianShell, qh: &QueueHandle<MeridianShell>) {
    let Some(cap) = state.screenshot_capture.as_mut() else {
        return;
    };
    let width = cap.width;
    let height = cap.height;
    let format = cap.format.unwrap_or(wl_shm::Format::Xrgb8888);
    if !is_supported_screenshot_format(format) {
        tracing::warn!("screenshot: unsupported shm format: {:?}", format);
        state.screenshot_capture = None;
        return;
    }
    let Some((stride, size)) = screenshot_buffer_layout(width, height) else {
        tracing::warn!(
            width,
            height,
            "screenshot: invalid or oversized buffer constraints"
        );
        state.screenshot_capture = None;
        return;
    };

    // Create anonymous file backed by memfd.
    let c_name = std::ffi::CString::new("meridian-screenshot").unwrap();
    let raw_fd = unsafe { libc::memfd_create(c_name.as_ptr(), libc::MFD_CLOEXEC) };
    if raw_fd < 0 {
        tracing::warn!("screenshot: memfd_create failed");
        state.screenshot_capture = None;
        return;
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
    if unsafe { libc::ftruncate(fd.as_raw_fd(), size as libc::off_t) } < 0 {
        tracing::warn!("screenshot: ftruncate failed");
        state.screenshot_capture = None;
        return;
    }

    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd.as_raw_fd(),
            0,
        )
    };
    if ptr == libc::MAP_FAILED {
        tracing::warn!("screenshot: mmap failed");
        state.screenshot_capture = None;
        return;
    }

    let wl_shm = state.shm.wl_shm();
    let pool = wl_shm.create_pool(fd.as_fd(), size as i32, qh, ());
    let buffer = pool.create_buffer(0, width as i32, height as i32, stride, format, qh, ());

    let cap = state.screenshot_capture.as_mut().unwrap();
    cap.fd = Some(fd);
    cap.mapped_ptr = ptr;
    cap.mapped_len = size;
    cap.pool = Some(pool);
    cap.buffer = Some(buffer.clone());

    let frame = cap.session.create_frame(qh, ());
    frame.attach_buffer(&buffer);
    frame.damage_buffer(0, 0, width as i32, height as i32);
    frame.capture();
    cap.frame = Some(frame);
}

fn is_supported_screenshot_format(format: wl_shm::Format) -> bool {
    matches!(format, wl_shm::Format::Xrgb8888 | wl_shm::Format::Argb8888)
}

fn screenshot_buffer_layout(width: u32, height: u32) -> Option<(i32, usize)> {
    if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
        return None;
    }
    let stride = width.checked_mul(4)?;
    let size = u64::from(stride).checked_mul(u64::from(height))?;
    if stride > i32::MAX as u32 || size > i32::MAX as u64 {
        return None;
    }
    Some((stride as i32, size as usize))
}

// ──── Dispatch: frame events ─────────────────────────────────────────────────

impl Dispatch<ExtImageCopyCaptureFrameV1, ()> for MeridianShell {
    fn event(
        state: &mut Self,
        _proxy: &ExtImageCopyCaptureFrameV1,
        event: ext_image_copy_capture_frame_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use ext_image_copy_capture_frame_v1::Event;
        match event {
            Event::Ready => {
                if let Some(cap) = state.screenshot_capture.take() {
                    let path = cap.path.clone();
                    let width = cap.width;
                    let height = cap.height;
                    let ptr = cap.mapped_ptr;
                    let len = cap.mapped_len;

                    if !ptr.is_null() && len > 0 {
                        let raw = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) };
                        match encode_screenshot_png(raw, width, height) {
                            Ok(png_data) => match std::fs::write(&path, &png_data) {
                                Ok(()) => tracing::info!("screenshot saved: {}", path.display()),
                                Err(e) => tracing::warn!("screenshot: write failed: {}", e),
                            },
                            Err(e) => {
                                tracing::warn!("screenshot: PNG encode failed: {}", e)
                            }
                        }
                    }
                    // cap drops here → munmap + Wayland object cleanup
                }
            }
            Event::Failed { reason } => {
                tracing::warn!("screenshot: capture failed: {:?}", reason);
                state.screenshot_capture = None;
            }
            _ => {}
        }
    }
}

/// Encode XRGB8888 pixels as an RGB PNG.
///
/// `wl_shm::Format::Xrgb8888` on little-endian: byte[0]=B, byte[1]=G, byte[2]=R, byte[3]=X.
pub(crate) fn encode_screenshot_png(
    xrgb: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    let rgb: Vec<u8> = xrgb
        .chunks_exact(4)
        .flat_map(|c| [c[2], c[1], c[0]])
        .collect();
    writer.write_image_data(&rgb)?;
    drop(writer);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{encode_screenshot_png, screenshot_buffer_layout};

    #[test]
    fn xrgb_to_rgb_channel_swap() {
        // XRGB8888 LE: byte[0]=B=0x11, byte[1]=G=0x22, byte[2]=R=0x33, byte[3]=X=0xFF
        let xrgb = [0x11u8, 0x22, 0x33, 0xFF];
        let png_data = encode_screenshot_png(&xrgb, 1, 1).unwrap();
        assert!(png_data.starts_with(b"\x89PNG"));

        let decoder = png::Decoder::new(png_data.as_slice());
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgb);
        // Expect R=0x33, G=0x22, B=0x11
        assert_eq!(&buf[..3], &[0x33, 0x22, 0x11]);
    }

    #[test]
    fn screenshot_buffer_layout_rejects_empty_or_oversized_constraints() {
        assert_eq!(screenshot_buffer_layout(0, 1), None);
        assert_eq!(screenshot_buffer_layout(1, 0), None);
        assert_eq!(screenshot_buffer_layout(u32::MAX, 1), None);
        assert_eq!(
            screenshot_buffer_layout(1920, 1080),
            Some((7680, 8_294_400))
        );
    }
}
