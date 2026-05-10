use smithay::{
    backend::{allocator::Fourcc, renderer::element::memory::MemoryRenderBuffer},
    utils::Transform,
};
use tracing::info;

#[cfg(feature = "xcursor-themes")]
use tracing::warn;

use super::embedded::{make_cursor_pixels, CURSOR_WIDTH};

pub const CURSOR_FORMAT: Fourcc = Fourcc::Argb8888;
const CURSOR_XHOT: u32 = 0;
const CURSOR_YHOT: u32 = 0;

#[derive(Debug, Clone)]
pub struct CursorImage {
    pub theme: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub xhot: u32,
    pub yhot: u32,
    pub pixels_rgba: Vec<u8>,
}

impl CursorImage {
    pub fn load_default() -> Self {
        #[cfg(feature = "xcursor-themes")]
        {
            let theme_name = std::env::var("XCURSOR_THEME").unwrap_or_default();
            let requested_size = std::env::var("XCURSOR_SIZE")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(24);
            return Self::load_theme(&theme_name, requested_size);
        }
        #[cfg(not(feature = "xcursor-themes"))]
        Self::embedded()
    }

    pub fn load_theme(theme_name: &str, requested_size: u32) -> Self {
        if theme_name.is_empty() {
            info!(
                "cursor fallback used: empty theme name, using embedded cursor size={}",
                requested_size
            );
            return Self::embedded_sized(requested_size);
        }

        #[cfg(feature = "xcursor-themes")]
        {
            super::xcursor::load_xcursor(theme_name, requested_size)
                .or_else(|err| {
                    warn!(
                        "Cannot load xcursor theme={} size={}: {}; trying \"default\"",
                        theme_name, requested_size, err
                    );
                    super::xcursor::load_xcursor("default", requested_size)
                })
                .unwrap_or_else(|err| {
                    warn!(
                        "cursor fallback used: xcursor load failed for theme={} size={}: {}",
                        theme_name, requested_size, err
                    );
                    Self::embedded_sized(requested_size)
                })
        }

        #[cfg(not(feature = "xcursor-themes"))]
        {
            info!(
                "cursor fallback used: xcursor feature disabled, using embedded cursor size={}",
                requested_size
            );
            Self::embedded_sized(requested_size)
        }
    }

    pub fn embedded() -> Self {
        Self::embedded_sized(CURSOR_WIDTH)
    }

    pub fn embedded_sized(size: u32) -> Self {
        let size = size.clamp(16, 64);
        info!(
            "Loading embedded Meridian cursor ({}x{} ARGB8888)",
            size, size
        );
        Self {
            theme: "meridian-embedded".to_string(),
            name: "left_ptr".to_string(),
            width: size,
            height: size,
            xhot: CURSOR_XHOT,
            yhot: CURSOR_YHOT,
            pixels_rgba: make_cursor_pixels(size, size),
        }
    }

    pub fn is_valid_visible_image(&self) -> bool {
        if self.width == 0 || self.height == 0 {
            return false;
        }
        if self.pixels_rgba.len() != self.width as usize * self.height as usize * 4 {
            return false;
        }
        let mut has_visible = false;
        let mut has_non_black_visible = false;
        for px in self.pixels_rgba.chunks_exact(4) {
            let [r, g, b, a] = [px[0], px[1], px[2], px[3]];
            if a != 0 {
                has_visible = true;
                if r != 0 || g != 0 || b != 0 {
                    has_non_black_visible = true;
                }
            }
        }
        has_visible && has_non_black_visible
    }

    pub fn to_memory_buffer(&self) -> MemoryRenderBuffer {
        info!(
            "cursor buffer rebuilt: name={} theme={} size={}x{} hotspot={},{}",
            self.name, self.theme, self.width, self.height, self.xhot, self.yhot
        );
        MemoryRenderBuffer::from_slice(
            &self.pixels_rgba,
            CURSOR_FORMAT,
            (self.width as i32, self.height as i32),
            1,
            Transform::Normal,
            None,
        )
    }
}
