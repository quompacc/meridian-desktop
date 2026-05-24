use std::collections::HashMap;

use meridian_config::ThemeColors;
use smithay::{
    backend::{allocator::Fourcc, renderer::element::memory::MemoryRenderBuffer},
    utils::Transform,
};

use crate::decoration::icons::{rasterize, IconTint, WindowIcon};

pub(crate) struct IconCache {
    buffers: HashMap<(WindowIcon, IconTint), MemoryRenderBuffer>,
    icon_size_px: u32,
    stroke_width: f32,
}

impl IconCache {
    pub(crate) fn new(icon_size_px: u32, stroke_width: f32) -> Self {
        Self {
            buffers: HashMap::new(),
            icon_size_px,
            stroke_width,
        }
    }

    pub(crate) fn get_or_build(
        &mut self,
        kind: WindowIcon,
        tint: IconTint,
        colors: &ThemeColors,
    ) -> &MemoryRenderBuffer {
        let stroke = match tint {
            IconTint::OnSurface => [colors.text.r, colors.text.g, colors.text.b, 255],
            IconTint::OnAccentRed => [
                colors.background.r,
                colors.background.g,
                colors.background.b,
                255,
            ],
        };
        self.buffers.entry((kind, tint)).or_insert_with(|| {
            let pixels = rasterize(kind, self.icon_size_px, stroke, self.stroke_width);
            MemoryRenderBuffer::from_slice(
                &pixels,
                Fourcc::Abgr8888,
                (self.icon_size_px as i32, self.icon_size_px as i32),
                1,
                Transform::Normal,
                None,
            )
        })
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.buffers.len()
    }
}

#[cfg(test)]
mod tests {
    use crate::decoration::icons::{IconTint, WindowIcon};

    use super::IconCache;

    #[test]
    fn test_icon_cache_lazy_builds_on_first_request() {
        let colors = meridian_config::ThemeColors::default();
        let mut cache = IconCache::new(16, 1.5);
        assert_eq!(cache.len(), 0);
        let _ = cache.get_or_build(WindowIcon::Close, IconTint::OnSurface, &colors);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_icon_cache_returns_same_buffer_on_second_request() {
        let colors = meridian_config::ThemeColors::default();
        let mut cache = IconCache::new(16, 1.5);
        let first_ptr = cache.get_or_build(WindowIcon::Close, IconTint::OnSurface, &colors)
            as *const _ as usize;
        let second_ptr = cache.get_or_build(WindowIcon::Close, IconTint::OnSurface, &colors)
            as *const _ as usize;
        assert_eq!(first_ptr, second_ptr);
    }
}
