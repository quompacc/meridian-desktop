use smithay_client_toolkit::shm::slot::{Buffer, SlotPool};
use tracing::info;
use wayland_client::protocol::wl_shm;

pub fn shm_buffer_format() -> wl_shm::Format {
    wl_shm::Format::Argb8888
}

pub fn shm_buffer_stride(width: u32) -> i32 {
    (width * 4) as i32
}

pub fn shm_buffer_size(width: u32, height: u32) -> usize {
    shm_buffer_stride(width) as usize * height as usize
}

pub fn buffer_for<'a>(
    pool: &mut SlotPool,
    current: &'a mut Option<Buffer>,
    width: u32,
    height: u32,
    stride: i32,
) -> &'a mut Buffer {
    let recreate = current
        .as_ref()
        .map(|buf| buf.height() != height as i32 || buf.stride() != stride)
        .unwrap_or(true);

    if recreate {
        info!(
            "Creating wl_shm buffer: {}x{} stride={} bytes={} format=Argb8888",
            width,
            height,
            stride,
            shm_buffer_size(width, height)
        );
        let (buffer, _) = pool
            .create_buffer(width as i32, height as i32, stride, shm_buffer_format())
            .expect("create shm buffer");
        *current = Some(buffer);
    }

    current.as_mut().expect("buffer exists")
}

#[cfg(test)]
mod tests {
    use super::{shm_buffer_format, shm_buffer_size, shm_buffer_stride};
    use wayland_client::protocol::wl_shm;

    #[test]
    fn buffer_format_is_argb8888() {
        assert_eq!(shm_buffer_format(), wl_shm::Format::Argb8888);
    }

    #[test]
    fn buffer_size_matches_dimensions() {
        assert_eq!(shm_buffer_size(1280, 36), 184_320);
    }

    #[test]
    fn buffer_stride_is_width_times_4() {
        assert_eq!(shm_buffer_stride(1280), 5120);
    }
}
