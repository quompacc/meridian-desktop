use super::{embedded::CURSOR_HEIGHT, embedded::CURSOR_WIDTH, CursorImage, CURSOR_FORMAT};
use smithay::backend::allocator::Fourcc;
use std::sync::{Mutex, OnceLock};

#[cfg(feature = "xcursor-themes")]
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const CURSOR_PIXELS_LEN: usize = CURSOR_WIDTH as usize * CURSOR_HEIGHT as usize * 4;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn embedded_cursor_has_correct_dimensions() {
    let cursor = CursorImage::embedded();
    assert_eq!(cursor.width, CURSOR_WIDTH);
    assert_eq!(cursor.height, CURSOR_HEIGHT);
    assert_eq!(cursor.xhot, 0);
    assert_eq!(cursor.yhot, 0);
    assert_eq!(cursor.pixels_rgba.len(), CURSOR_PIXELS_LEN);
    assert_eq!(CURSOR_FORMAT, Fourcc::Argb8888);
}

#[test]
fn embedded_cursor_is_valid() {
    assert!(CursorImage::embedded().is_valid_visible_image());
}

#[test]
fn empty_theme_returns_embedded() {
    assert_eq!(CursorImage::load_theme("", 24).theme, "meridian-embedded");
}

#[test]
fn empty_theme_uses_requested_size() {
    let cursor = CursorImage::load_theme("", 24);
    assert_eq!(cursor.width, 24);
    assert_eq!(cursor.height, 24);
}

#[test]
fn embedded_cursor_icon_name_is_left_ptr() {
    assert_eq!(CursorImage::embedded().name, "left_ptr");
}

#[test]
fn embedded_cursor_hotspot_is_origin() {
    let c = CursorImage::embedded();
    assert_eq!(c.xhot, 0);
    assert_eq!(c.yhot, 0);
    assert!(c.xhot < c.width);
    assert!(c.yhot < c.height);
}

#[test]
fn embedded_cursor_pixel_spot_check() {
    let px = &CursorImage::embedded().pixels_rgba;
    assert_eq!(
        &px[0..4],
        &[0, 0, 0, 255],
        "tip pixel (0,0) must be opaque black border"
    );

    let has_opaque_fill = px
        .chunks_exact(4)
        .any(|rgba| rgba[3] == 255 && rgba[0] == 255 && rgba[1] == 255 && rgba[2] == 255);
    assert!(
        has_opaque_fill,
        "expected at least one opaque white fill pixel"
    );

    let off2 = 31 * 4;
    assert_eq!(
        &px[off2..off2 + 4],
        &[0, 0, 0, 0],
        "outside pixel (31,0) must be transparent"
    );
}

#[test]
fn embedded_cursor_is_not_all_transparent() {
    let px = &CursorImage::embedded().pixels_rgba;
    let visible = px.chunks_exact(4).filter(|rgba| rgba[3] > 0).count();
    let total = CURSOR_WIDTH as usize * CURSOR_HEIGHT as usize;
    assert!(
        visible * 5 >= total,
        "expected at least 20% non-transparent pixels, got {visible}/{total}"
    );
}

#[test]
fn embedded_cursor_tip_is_opaque() {
    let px = &CursorImage::embedded().pixels_rgba;
    assert_eq!(px[3], 255, "tip pixel alpha at (0,0) must be fully opaque");
}

#[test]
fn embedded_cursor_uses_premultiplied_alpha() {
    let px = &CursorImage::embedded().pixels_rgba;
    for rgba in px.chunks_exact(4) {
        let [r, g, b, a] = [rgba[0], rgba[1], rgba[2], rgba[3]];
        assert!(
            r <= a && g <= a && b <= a,
            "expected premultiplied RGBA, got r={} g={} b={} a={}",
            r,
            g,
            b,
            a
        );
    }
}

#[test]
fn embedded_cursor_to_memory_buffer_succeeds() {
    let _buffer = CursorImage::embedded().to_memory_buffer();
}

#[test]
fn cursor_has_valid_image() {
    let _guard = env_lock().lock().unwrap();
    let cursor = CursorImage::load_default();
    assert!(cursor.is_valid_visible_image());
}

#[cfg(feature = "xcursor-themes")]
#[test]
fn cursor_theme_loads_successfully() {
    let _guard = env_lock().lock().unwrap();
    let fixture = create_cursor_theme_fixture();
    let old_xcursor_path = std::env::var("XCURSOR_PATH").ok();
    std::env::set_var("XCURSOR_PATH", fixture.to_string_lossy().as_ref());

    let cursor =
        super::xcursor::load_xcursor("default", 24).expect("fixture cursor theme should load");

    if let Some(value) = old_xcursor_path {
        std::env::set_var("XCURSOR_PATH", value);
    } else {
        std::env::remove_var("XCURSOR_PATH");
    }

    assert!(cursor.is_valid_visible_image());
    assert_eq!(CURSOR_FORMAT, Fourcc::Argb8888);
    assert!(cursor.width > 0);
    assert!(cursor.height > 0);
    assert_eq!(cursor.xhot, 0);
    assert_eq!(cursor.yhot, 0);
}

#[cfg(feature = "xcursor-themes")]
fn create_cursor_theme_fixture() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "meridian-xcursor-test-{}-{}",
        std::process::id(),
        unique
    ));
    let cursor_dir = root.join("default").join("cursors");
    fs::create_dir_all(&cursor_dir).unwrap();
    fs::write(cursor_dir.join("left_ptr"), sample_xcursor_file()).unwrap();
    root
}

#[cfg(feature = "xcursor-themes")]
fn sample_xcursor_file() -> Vec<u8> {
    let width = 4u32;
    let height = 4u32;
    let image_pos = 28u32;
    let mut data = Vec::new();

    push_u32(&mut data, 0x7275_6358);
    push_u32(&mut data, 16);
    push_u32(&mut data, 0x0001_0000);
    push_u32(&mut data, 1);
    push_u32(&mut data, 0xfffd_0002);
    push_u32(&mut data, 24);
    push_u32(&mut data, image_pos);

    push_u32(&mut data, 36);
    push_u32(&mut data, 0xfffd_0002);
    push_u32(&mut data, 24);
    push_u32(&mut data, 1);
    push_u32(&mut data, width);
    push_u32(&mut data, height);
    push_u32(&mut data, 0);
    push_u32(&mut data, 0);
    push_u32(&mut data, 0);

    for y in 0..height {
        for x in 0..width {
            let pixel = if x == 0 || y == 0 {
                [255u8, 255, 255, 255]
            } else if x == y {
                [32, 32, 32, 255]
            } else {
                [0, 0, 0, 0]
            };
            data.extend_from_slice(&pixel);
        }
    }

    data
}

#[cfg(feature = "xcursor-themes")]
fn push_u32(data: &mut Vec<u8>, value: u32) {
    data.extend_from_slice(&value.to_le_bytes());
}
