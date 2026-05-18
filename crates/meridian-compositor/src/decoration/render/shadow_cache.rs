use smithay::{
    backend::{allocator::Fourcc, renderer::element::memory::MemoryRenderBuffer},
    utils::Transform,
};

use crate::decoration::shadow_bitmap::{
    flip_horizontal, flip_vertical, rasterize_corner_rect, rasterize_edge_left, rasterize_edge_top,
};

pub(crate) struct ShadowBuffers<'a> {
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
    last_radius_side: u32,
    last_radius_top: u32,
    last_alpha: f32,
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
            last_radius_side: 0,
            last_radius_top: 0,
            last_alpha: 0.0,
            initialized: false,
            #[cfg(test)]
            rebuild_count: 0,
        }
    }

    pub(crate) fn get_for(
        &mut self,
        radius_side_px: u32,
        radius_top_px: u32,
        base_alpha: f32,
    ) -> ShadowBuffers<'_> {
        let radius_side_px = radius_side_px.max(1);
        let radius_top_px = radius_top_px.max(1);
        if !self.initialized
            || radius_side_px != self.last_radius_side
            || radius_top_px != self.last_radius_top
            || (base_alpha - self.last_alpha).abs() > 0.001
        {
            self.rebuild(radius_side_px, radius_top_px, base_alpha);
        }

        ShadowBuffers {
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

    fn rebuild(&mut self, radius_side_px: u32, radius_top_px: u32, base_alpha: f32) {
        let corner_top_pixels = rasterize_corner_rect(radius_side_px, radius_top_px, base_alpha);
        let corner_tr_pixels = flip_horizontal(&corner_top_pixels, radius_side_px, radius_top_px);
        let corner_bottom_pixels =
            rasterize_corner_rect(radius_side_px, radius_side_px, base_alpha);
        let corner_br_pixels =
            flip_horizontal(&corner_bottom_pixels, radius_side_px, radius_side_px);

        let edge_top_pixels = rasterize_edge_top(radius_top_px, base_alpha);
        let edge_bottom_unflipped = rasterize_edge_top(radius_side_px, base_alpha);
        let edge_bottom_pixels = flip_vertical(&edge_bottom_unflipped, 1, radius_side_px);
        let edge_left_pixels = rasterize_edge_left(radius_side_px, base_alpha);
        let edge_right_pixels = flip_horizontal(&edge_left_pixels, radius_side_px, 1);

        let top_corner_size = (radius_side_px as i32, radius_top_px as i32);
        let bottom_corner_size = (radius_side_px as i32, radius_side_px as i32);
        self.corner_tl = Some(MemoryRenderBuffer::from_slice(
            &corner_top_pixels,
            Fourcc::Abgr8888,
            top_corner_size,
            1,
            Transform::Normal,
            None,
        ));
        self.corner_tr = Some(MemoryRenderBuffer::from_slice(
            &corner_tr_pixels,
            Fourcc::Abgr8888,
            top_corner_size,
            1,
            Transform::Normal,
            None,
        ));
        self.corner_bl = Some(MemoryRenderBuffer::from_slice(
            &corner_bottom_pixels,
            Fourcc::Abgr8888,
            bottom_corner_size,
            1,
            Transform::Normal,
            None,
        ));
        self.corner_br = Some(MemoryRenderBuffer::from_slice(
            &corner_br_pixels,
            Fourcc::Abgr8888,
            bottom_corner_size,
            1,
            Transform::Normal,
            None,
        ));

        self.edge_top = Some(MemoryRenderBuffer::from_slice(
            &edge_top_pixels,
            Fourcc::Abgr8888,
            (1, radius_top_px as i32),
            1,
            Transform::Normal,
            None,
        ));
        self.edge_bottom = Some(MemoryRenderBuffer::from_slice(
            &edge_bottom_pixels,
            Fourcc::Abgr8888,
            (1, radius_side_px as i32),
            1,
            Transform::Normal,
            None,
        ));
        self.edge_left = Some(MemoryRenderBuffer::from_slice(
            &edge_left_pixels,
            Fourcc::Abgr8888,
            (radius_side_px as i32, 1),
            1,
            Transform::Normal,
            None,
        ));
        self.edge_right = Some(MemoryRenderBuffer::from_slice(
            &edge_right_pixels,
            Fourcc::Abgr8888,
            (radius_side_px as i32, 1),
            1,
            Transform::Normal,
            None,
        ));

        self.last_radius_side = radius_side_px;
        self.last_radius_top = radius_top_px;
        self.last_alpha = base_alpha;
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
        let _ = cache.get_for(24, 24, 0.22);
        assert_eq!(cache.rebuild_count(), 1);
    }

    #[test]
    fn test_shadow_cache_rebuilds_on_radius_change() {
        let mut cache = ShadowCache::new();
        let _ = cache.get_for(24, 24, 0.22);
        let _ = cache.get_for(40, 40, 0.22);
        assert_eq!(cache.rebuild_count(), 2);
    }

    #[test]
    fn test_shadow_cache_does_not_rebuild_on_identical_request() {
        let mut cache = ShadowCache::new();
        let first_ptr = cache.get_for(40, 40, 0.22).corner_tl as *const _ as usize;
        let second_ptr = cache.get_for(40, 40, 0.22).corner_tl as *const _ as usize;
        assert_eq!(first_ptr, second_ptr);
        assert_eq!(cache.rebuild_count(), 1);
    }

    #[test]
    fn test_shadow_cache_rebuilds_on_top_radius_change() {
        let mut cache = ShadowCache::new();
        let _ = cache.get_for(40, 12, 0.18);
        let _ = cache.get_for(40, 12, 0.18);
        let _ = cache.get_for(40, 20, 0.18);
        assert_eq!(cache.rebuild_count(), 2);
    }
}
