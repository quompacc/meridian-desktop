use std::collections::HashMap;

use meridian_config::ThemeColors;
use smithay::{
    backend::{allocator::Fourcc, renderer::element::memory::MemoryRenderBuffer},
    utils::Transform,
};

use crate::decoration::corner_mask_bitmap::rasterize_rounded_rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ButtonBgTint {
    SurfaceHover,
    RedHover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ButtonBgKey {
    tint: ButtonBgTint,
    color: [u8; 4],
}

pub(crate) struct ButtonBgCache {
    buffers: HashMap<ButtonBgKey, MemoryRenderBuffer>,
    width: u32,
    height: u32,
    radius: u32,
}

#[cfg(test)]
impl ButtonBgCache {
    fn len(&self) -> usize {
        self.buffers.len()
    }
}

#[cfg(test)]
mod tests {
    use meridian_config::ThemeColors;

    use super::{ButtonBgCache, ButtonBgTint};

    #[test]
    fn button_bg_cache_builds_on_first_request() {
        let mut cache = ButtonBgCache::new(28, 24, 6);
        let colors = ThemeColors::default();
        assert_eq!(cache.len(), 0);
        let _ = cache.get_for(ButtonBgTint::SurfaceHover, &colors);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn button_bg_cache_reuses_buffer_for_identical_request() {
        let mut cache = ButtonBgCache::new(28, 24, 6);
        let colors = ThemeColors::default();
        let first = cache.get_for(ButtonBgTint::RedHover, &colors) as *const _ as usize;
        let second = cache.get_for(ButtonBgTint::RedHover, &colors) as *const _ as usize;
        assert_eq!(first, second);
        assert_eq!(cache.len(), 1);
    }
}

impl ButtonBgCache {
    pub(crate) fn new(width: u32, height: u32, radius: u32) -> Self {
        Self {
            buffers: HashMap::new(),
            width,
            height,
            radius,
        }
    }

    pub(crate) fn get_for(
        &mut self,
        tint: ButtonBgTint,
        colors: &ThemeColors,
    ) -> &MemoryRenderBuffer {
        let color = match tint {
            ButtonBgTint::SurfaceHover => {
                [colors.surface.r, colors.surface.g, colors.surface.b, 255]
            }
            ButtonBgTint::RedHover => [colors.error.r, colors.error.g, colors.error.b, 255],
        };

        self.buffers
            .entry(ButtonBgKey { tint, color })
            .or_insert_with(|| {
                let pixels = rasterize_rounded_rect(self.width, self.height, self.radius, color);
                MemoryRenderBuffer::from_slice(
                    &pixels,
                    Fourcc::Abgr8888,
                    (self.width as i32, self.height as i32),
                    1,
                    Transform::Normal,
                    None,
                )
            })
    }
}
