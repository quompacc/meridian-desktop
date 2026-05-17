use smithay::{
    backend::{allocator::Fourcc, renderer::element::memory::MemoryRenderBuffer},
    utils::Transform,
};

use crate::decoration::{
    corner_mask_bitmap::rasterize_corner_mask,
    shadow_bitmap::{flip_horizontal, flip_vertical},
};

pub(crate) struct FrameCornerBuffers<'a> {
    pub(crate) tl: &'a MemoryRenderBuffer,
    pub(crate) tr: &'a MemoryRenderBuffer,
    pub(crate) bl: &'a MemoryRenderBuffer,
    pub(crate) br: &'a MemoryRenderBuffer,
}

pub(crate) struct CornerCache {
    tl: Option<MemoryRenderBuffer>,
    tr: Option<MemoryRenderBuffer>,
    bl: Option<MemoryRenderBuffer>,
    br: Option<MemoryRenderBuffer>,
    last_radius: u32,
    last_color: [u8; 4],
    initialized: bool,
    #[cfg(test)]
    rebuild_count: usize,
}

impl CornerCache {
    pub(crate) fn new() -> Self {
        Self {
            tl: None,
            tr: None,
            bl: None,
            br: None,
            last_radius: 0,
            last_color: [0; 4],
            initialized: false,
            #[cfg(test)]
            rebuild_count: 0,
        }
    }

    pub(crate) fn get_for(&mut self, radius_px: u32, color: [u8; 4]) -> FrameCornerBuffers<'_> {
        let radius_px = radius_px.max(1);
        if !self.initialized || radius_px != self.last_radius || color != self.last_color {
            self.rebuild(radius_px, color);
        }

        FrameCornerBuffers {
            tl: self.tl.as_ref().expect("corner tl should exist"),
            tr: self.tr.as_ref().expect("corner tr should exist"),
            bl: self.bl.as_ref().expect("corner bl should exist"),
            br: self.br.as_ref().expect("corner br should exist"),
        }
    }

    fn rebuild(&mut self, radius_px: u32, color: [u8; 4]) {
        let (tl_pixels, internal_size) = rasterize_corner_mask(radius_px, color);
        let tr_pixels = flip_horizontal(&tl_pixels, internal_size, internal_size);
        let bl_pixels = flip_vertical(&tl_pixels, internal_size, internal_size);
        let br_pixels = flip_vertical(&tr_pixels, internal_size, internal_size);

        let size = (internal_size as i32, internal_size as i32);
        self.tl = Some(MemoryRenderBuffer::from_slice(
            &tl_pixels,
            Fourcc::Abgr8888,
            size,
            1,
            Transform::Normal,
            None,
        ));
        self.tr = Some(MemoryRenderBuffer::from_slice(
            &tr_pixels,
            Fourcc::Abgr8888,
            size,
            1,
            Transform::Normal,
            None,
        ));
        self.bl = Some(MemoryRenderBuffer::from_slice(
            &bl_pixels,
            Fourcc::Abgr8888,
            size,
            1,
            Transform::Normal,
            None,
        ));
        self.br = Some(MemoryRenderBuffer::from_slice(
            &br_pixels,
            Fourcc::Abgr8888,
            size,
            1,
            Transform::Normal,
            None,
        ));

        self.last_radius = radius_px;
        self.last_color = color;
        self.initialized = true;
        #[cfg(test)]
        {
            self.rebuild_count += 1;
        }
    }

    #[cfg(test)]
    fn rebuild_count(&self) -> usize {
        self.rebuild_count
    }
}

#[cfg(test)]
mod tests {
    use super::CornerCache;

    #[test]
    fn corner_cache_lazy_builds_on_first_get() {
        let mut cache = CornerCache::new();
        assert_eq!(cache.rebuild_count(), 0);
        let _ = cache.get_for(12, [10, 20, 30, 255]);
        assert_eq!(cache.rebuild_count(), 1);
    }

    #[test]
    fn corner_cache_rebuilds_on_color_change() {
        let mut cache = CornerCache::new();
        let _ = cache.get_for(12, [10, 20, 30, 255]);
        let _ = cache.get_for(12, [12, 20, 30, 255]);
        assert_eq!(cache.rebuild_count(), 2);
    }

    #[test]
    fn corner_cache_no_rebuild_on_identical_request() {
        let mut cache = CornerCache::new();
        let first_ptr = cache.get_for(12, [10, 20, 30, 255]).tl as *const _ as usize;
        let second_ptr = cache.get_for(12, [10, 20, 30, 255]).tl as *const _ as usize;
        assert_eq!(first_ptr, second_ptr);
        assert_eq!(cache.rebuild_count(), 1);
    }
}
