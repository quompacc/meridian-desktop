use smithay::{
    backend::{allocator::Fourcc, renderer::element::memory::MemoryRenderBuffer},
    utils::Transform,
};

use crate::decoration::shadow_bitmap::{
    flip_horizontal, flip_vertical, rasterize_edge_left, rasterize_edge_top,
    rasterize_shadow_corner_with_frame,
};

pub(crate) struct ShadowBuffers<'a> {
    pub(crate) internal_size: u32,
    pub(crate) corner_tl: &'a MemoryRenderBuffer,
    pub(crate) corner_tr: &'a MemoryRenderBuffer,
    pub(crate) corner_bl: &'a MemoryRenderBuffer,
    pub(crate) corner_br: &'a MemoryRenderBuffer,
    pub(crate) edge_top: &'a MemoryRenderBuffer,
    pub(crate) edge_bottom: &'a MemoryRenderBuffer,
    pub(crate) edge_left: &'a MemoryRenderBuffer,
    pub(crate) edge_right: &'a MemoryRenderBuffer,
}

pub(crate) struct ShadowCache {
    corner_tl: Option<MemoryRenderBuffer>,
    corner_tr: Option<MemoryRenderBuffer>,
    corner_bl: Option<MemoryRenderBuffer>,
    corner_br: Option<MemoryRenderBuffer>,
    edge_top: Option<MemoryRenderBuffer>,
    edge_bottom: Option<MemoryRenderBuffer>,
    edge_left: Option<MemoryRenderBuffer>,
    edge_right: Option<MemoryRenderBuffer>,
    last_radius: u32,
    last_alpha: f32,
    last_frame_radius: u32,
    last_internal_size: u32,
    initialized: bool,
    #[cfg(test)]
    rebuild_count: usize,
}

impl ShadowCache {
    pub(crate) fn new() -> Self {
        Self {
            corner_tl: None,
            corner_tr: None,
            corner_bl: None,
            corner_br: None,
            edge_top: None,
            edge_bottom: None,
            edge_left: None,
            edge_right: None,
            last_radius: 0,
            last_alpha: 0.0,
            last_frame_radius: 0,
            last_internal_size: 0,
            initialized: false,
            #[cfg(test)]
            rebuild_count: 0,
        }
    }

    pub(crate) fn get_for(
        &mut self,
        radius_px: u32,
        base_alpha: f32,
        frame_radius_px: u32,
    ) -> ShadowBuffers<'_> {
        let radius_px = radius_px.max(1);
        if !self.initialized
            || radius_px != self.last_radius
            || (base_alpha - self.last_alpha).abs() > 0.001
            || frame_radius_px != self.last_frame_radius
        {
            self.rebuild(radius_px, base_alpha, frame_radius_px);
        }

        ShadowBuffers {
            internal_size: self.last_internal_size,
            corner_tl: self
                .corner_tl
                .as_ref()
                .expect("shadow corner tl should exist"),
            corner_tr: self
                .corner_tr
                .as_ref()
                .expect("shadow corner tr should exist"),
            corner_bl: self
                .corner_bl
                .as_ref()
                .expect("shadow corner bl should exist"),
            corner_br: self
                .corner_br
                .as_ref()
                .expect("shadow corner br should exist"),
            edge_top: self
                .edge_top
                .as_ref()
                .expect("shadow edge top should exist"),
            edge_bottom: self
                .edge_bottom
                .as_ref()
                .expect("shadow edge bottom should exist"),
            edge_left: self
                .edge_left
                .as_ref()
                .expect("shadow edge left should exist"),
            edge_right: self
                .edge_right
                .as_ref()
                .expect("shadow edge right should exist"),
        }
    }

    fn rebuild(&mut self, radius_px: u32, base_alpha: f32, frame_radius_px: u32) {
        let (corner_tl_pixels, internal_size) =
            rasterize_shadow_corner_with_frame(radius_px, frame_radius_px, base_alpha);
        let corner_tr_pixels = flip_horizontal(&corner_tl_pixels, internal_size, internal_size);
        let corner_bl_pixels = flip_vertical(&corner_tl_pixels, internal_size, internal_size);
        let corner_br_pixels = flip_vertical(&corner_tr_pixels, internal_size, internal_size);
        let edge_top_pixels = rasterize_edge_top(radius_px, base_alpha);
        let edge_bottom_pixels = flip_vertical(&edge_top_pixels, 1, radius_px);
        let edge_left_pixels = rasterize_edge_left(radius_px, base_alpha);
        let edge_right_pixels = flip_horizontal(&edge_left_pixels, radius_px, 1);

        let corner_size = (internal_size as i32, internal_size as i32);
        self.corner_tl = Some(MemoryRenderBuffer::from_slice(
            &corner_tl_pixels,
            Fourcc::Abgr8888,
            corner_size,
            1,
            Transform::Normal,
            None,
        ));
        self.corner_tr = Some(MemoryRenderBuffer::from_slice(
            &corner_tr_pixels,
            Fourcc::Abgr8888,
            corner_size,
            1,
            Transform::Normal,
            None,
        ));
        self.corner_bl = Some(MemoryRenderBuffer::from_slice(
            &corner_bl_pixels,
            Fourcc::Abgr8888,
            corner_size,
            1,
            Transform::Normal,
            None,
        ));
        self.corner_br = Some(MemoryRenderBuffer::from_slice(
            &corner_br_pixels,
            Fourcc::Abgr8888,
            corner_size,
            1,
            Transform::Normal,
            None,
        ));

        self.edge_top = Some(MemoryRenderBuffer::from_slice(
            &edge_top_pixels,
            Fourcc::Abgr8888,
            (1, radius_px as i32),
            1,
            Transform::Normal,
            None,
        ));
        self.edge_bottom = Some(MemoryRenderBuffer::from_slice(
            &edge_bottom_pixels,
            Fourcc::Abgr8888,
            (1, radius_px as i32),
            1,
            Transform::Normal,
            None,
        ));
        self.edge_left = Some(MemoryRenderBuffer::from_slice(
            &edge_left_pixels,
            Fourcc::Abgr8888,
            (radius_px as i32, 1),
            1,
            Transform::Normal,
            None,
        ));
        self.edge_right = Some(MemoryRenderBuffer::from_slice(
            &edge_right_pixels,
            Fourcc::Abgr8888,
            (radius_px as i32, 1),
            1,
            Transform::Normal,
            None,
        ));

        self.last_radius = radius_px;
        self.last_alpha = base_alpha;
        self.last_frame_radius = frame_radius_px;
        self.last_internal_size = internal_size;
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
    use super::ShadowCache;

    #[test]
    fn test_shadow_cache_lazy_builds_on_first_get() {
        let mut cache = ShadowCache::new();
        assert_eq!(cache.rebuild_count(), 0);
        let _ = cache.get_for(24, 0.5, 12);
        assert_eq!(cache.rebuild_count(), 1);
    }

    #[test]
    fn test_shadow_cache_rebuilds_on_radius_change() {
        let mut cache = ShadowCache::new();
        let _ = cache.get_for(24, 0.5, 12);
        let _ = cache.get_for(32, 0.5, 12);
        assert_eq!(cache.rebuild_count(), 2);
    }

    #[test]
    fn test_shadow_cache_does_not_rebuild_on_identical_request() {
        let mut cache = ShadowCache::new();
        let first_ptr = cache.get_for(24, 0.5, 12).corner_tl as *const _ as usize;
        let second_ptr = cache.get_for(24, 0.5, 12).corner_tl as *const _ as usize;
        assert_eq!(first_ptr, second_ptr);
        assert_eq!(cache.rebuild_count(), 1);
    }
}
