use meridian_config::Decorations;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle, Size};

use super::super::{DecorationManager, BUTTON_MARGIN, BUTTON_SIZE, TITLE_BAR_HEIGHT};

pub(crate) const SSD_RESIZE_HANDLE_THICKNESS: i32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SsdFrameMetrics {
    pub(crate) border_width: i32,
    pub(crate) titlebar_height: i32,
    pub(crate) frame_origin: Point<i32, Logical>,
    pub(crate) frame_size: Size<i32, Logical>,
    pub(crate) client_origin: Point<i32, Logical>,
    pub(crate) client_size: Size<i32, Logical>,
    pub(crate) frame_rect: Rectangle<i32, Logical>,
    pub(crate) client_rect: Rectangle<i32, Logical>,
    pub(crate) titlebar_rect: Rectangle<i32, Logical>,
}

impl SsdFrameMetrics {
    pub(crate) fn from_frame_origin(
        frame_origin: Point<i32, Logical>,
        client_size: Size<i32, Logical>,
        border_width: i32,
        titlebar_height: i32,
    ) -> Self {
        let frame_w = client_size.w + border_width * 2;
        let frame_h = client_size.h + titlebar_height + border_width * 2;
        let client_origin = Point::from((
            frame_origin.x + border_width,
            frame_origin.y + titlebar_height + border_width,
        ));
        let frame_size = Size::from((frame_w, frame_h));
        let frame_rect = Rectangle::new(frame_origin, frame_size);
        let client_rect = Rectangle::new(client_origin, client_size);
        let titlebar_rect = Rectangle::new(
            frame_origin,
            Size::from((frame_w, titlebar_height + border_width)),
        );

        Self {
            border_width,
            titlebar_height,
            frame_origin,
            frame_size,
            client_origin,
            client_size,
            frame_rect,
            client_rect,
            titlebar_rect,
        }
    }

    pub(crate) fn from_client_origin(
        client_origin: Point<i32, Logical>,
        client_size: Size<i32, Logical>,
        border_width: i32,
        titlebar_height: i32,
    ) -> Self {
        let frame_origin = Point::from((
            client_origin.x - border_width,
            client_origin.y - titlebar_height - border_width,
        ));
        Self::from_frame_origin(frame_origin, client_size, border_width, titlebar_height)
    }

    pub(crate) fn decoration_offset(self) -> (i32, i32) {
        (
            self.client_origin.x - self.frame_origin.x,
            self.client_origin.y - self.frame_origin.y,
        )
    }

    pub(crate) fn decoration_inset(self) -> (i32, i32, i32, i32) {
        let left = self.client_origin.x - self.frame_origin.x;
        let top = self.client_origin.y - self.frame_origin.y;
        let right = self.frame_rect.loc.x + self.frame_rect.size.w
            - (self.client_rect.loc.x + self.client_rect.size.w);
        let bottom = self.frame_rect.loc.y + self.frame_rect.size.h
            - (self.client_rect.loc.y + self.client_rect.size.h);
        (left, top, right, bottom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SsdButtonMetrics {
    pub(crate) close_rect: Rectangle<i32, Logical>,
    pub(crate) maximize_rect: Rectangle<i32, Logical>,
    pub(crate) minimize_rect: Rectangle<i32, Logical>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SsdResizeBandMetrics {
    pub(crate) thickness: i32,
    pub(crate) top_band: Rectangle<i32, Logical>,
    pub(crate) left_band: Rectangle<i32, Logical>,
    pub(crate) right_band: Rectangle<i32, Logical>,
    pub(crate) bottom_band: Rectangle<i32, Logical>,
    pub(crate) top_left_corner: Rectangle<i32, Logical>,
    pub(crate) top_right_corner: Rectangle<i32, Logical>,
    pub(crate) bottom_left_corner: Rectangle<i32, Logical>,
    pub(crate) bottom_right_corner: Rectangle<i32, Logical>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SsdShadowMetrics {
    pub(crate) rect: Rectangle<i32, Logical>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SsdChromeMetrics {
    pub(crate) frame: SsdFrameMetrics,
}

#[allow(dead_code)]
impl SsdChromeMetrics {
    pub(crate) fn new(frame: SsdFrameMetrics) -> Self {
        Self { frame }
    }

    pub(crate) fn decoration_offset(self) -> (i32, i32) {
        self.frame.decoration_offset()
    }

    pub(crate) fn decoration_inset(self) -> (i32, i32, i32, i32) {
        self.frame.decoration_inset()
    }

    pub(crate) fn button_metrics(self) -> Option<SsdButtonMetrics> {
        if self.frame.titlebar_height <= 0 {
            return None;
        }

        let frame_right = self.frame.frame_origin.x + self.frame.frame_size.w;
        let close_x = frame_right - BUTTON_SIZE - BUTTON_MARGIN;
        let close_y = self.frame.frame_origin.y
            + (self.frame.titlebar_height - BUTTON_SIZE) / 2
            + self.frame.border_width;
        let max_x = close_x - BUTTON_SIZE - BUTTON_MARGIN / 2;
        let min_x = max_x - BUTTON_SIZE - BUTTON_MARGIN / 2;
        let close_rect =
            Rectangle::new((close_x, close_y).into(), (BUTTON_SIZE, BUTTON_SIZE).into());
        let maximize_rect =
            Rectangle::new((max_x, close_y).into(), (BUTTON_SIZE, BUTTON_SIZE).into());
        let minimize_rect =
            Rectangle::new((min_x, close_y).into(), (BUTTON_SIZE, BUTTON_SIZE).into());

        Some(SsdButtonMetrics {
            close_rect,
            maximize_rect,
            minimize_rect,
        })
    }

    pub(crate) fn resize_band_metrics(self) -> Option<SsdResizeBandMetrics> {
        let bw = self.frame.border_width;
        if bw <= 0 {
            return None;
        }

        let resize_w = bw.max(SSD_RESIZE_HANDLE_THICKNESS);
        let frame_left = self.frame.frame_origin.x;
        let frame_top = self.frame.frame_origin.y;
        let frame_right = frame_left + self.frame.frame_size.w;
        let frame_bottom = frame_top + self.frame.frame_size.h;
        let top_band = Rectangle::new(
            (frame_left, frame_top).into(),
            (self.frame.frame_size.w, resize_w).into(),
        );
        let left_band = Rectangle::new(
            (frame_left, frame_top).into(),
            (resize_w, self.frame.frame_size.h).into(),
        );
        let right_band = Rectangle::new(
            (frame_right - resize_w, frame_top).into(),
            (resize_w, self.frame.frame_size.h).into(),
        );
        let bottom_band = Rectangle::new(
            (frame_left, frame_bottom - resize_w).into(),
            (self.frame.frame_size.w, resize_w).into(),
        );
        let top_left_corner =
            Rectangle::new((frame_left, frame_top).into(), (resize_w, resize_w).into());
        let top_right_corner = Rectangle::new(
            (frame_right - resize_w, frame_top).into(),
            (resize_w, resize_w).into(),
        );
        let bottom_left_corner = Rectangle::new(
            (frame_left, frame_bottom - resize_w).into(),
            (resize_w, resize_w).into(),
        );
        let bottom_right_corner = Rectangle::new(
            (frame_right - resize_w, frame_bottom - resize_w).into(),
            (resize_w, resize_w).into(),
        );

        Some(SsdResizeBandMetrics {
            thickness: resize_w,
            top_band,
            left_band,
            right_band,
            bottom_band,
            top_left_corner,
            top_right_corner,
            bottom_left_corner,
            bottom_right_corner,
        })
    }

    pub(crate) fn shadow_metrics(self, shadow_radius: i32) -> Option<SsdShadowMetrics> {
        if self.frame.border_width <= 0 {
            return None;
        }

        let sw = (self.frame.frame_size.w + shadow_radius * 2).max(1);
        let sh = (self.frame.client_size.h
            + self.frame.titlebar_height
            + self.frame.border_width
            + shadow_radius * 2)
            .max(1);
        let shadow_rect = Rectangle::new(
            (
                self.frame.frame_origin.x - shadow_radius,
                self.frame.frame_origin.y - shadow_radius,
            )
                .into(),
            (sw, sh).into(),
        );
        Some(SsdShadowMetrics { rect: shadow_rect })
    }
}

impl DecorationManager {
    pub(crate) fn ssd_render_metrics(
        &self,
        surface: &WlSurface,
        window_loc: Point<i32, Logical>,
        content_size: Size<i32, Logical>,
        theme: &Decorations,
    ) -> SsdFrameMetrics {
        let (border_width, titlebar_height) = self
            .decorations
            .get(&Self::key(surface))
            .map(|deco| {
                let bw = deco.border_width(theme);
                let title_h = if deco.should_draw_title_bar() {
                    TITLE_BAR_HEIGHT
                } else {
                    0
                };
                (bw, title_h)
            })
            .unwrap_or((0, 0));

        SsdFrameMetrics::from_client_origin(window_loc, content_size, border_width, titlebar_height)
    }

    pub fn decoration_offset(&self, surface: &WlSurface, theme: &Decorations) -> (i32, i32) {
        let Some(deco) = self.decorations.get(&Self::key(surface)) else {
            return (0, 0);
        };
        if !deco.should_draw() {
            return (0, 0);
        }
        let bw = deco.border_width(theme);
        let title_h = if deco.should_draw_title_bar() {
            TITLE_BAR_HEIGHT
        } else {
            0
        };
        let metrics = SsdFrameMetrics::from_frame_origin((0, 0).into(), (0, 0).into(), bw, title_h);
        (
            metrics.client_origin.x - metrics.frame_origin.x,
            metrics.client_origin.y - metrics.frame_origin.y,
        )
    }

    pub fn decoration_inset(
        &self,
        surface: &WlSurface,
        theme: &Decorations,
    ) -> (i32, i32, i32, i32) {
        let Some(deco) = self.decorations.get(&Self::key(surface)) else {
            return (0, 0, 0, 0);
        };
        if !deco.should_draw() {
            return (0, 0, 0, 0);
        }
        let bw = deco.border_width(theme);
        let title_h = if deco.should_draw_title_bar() {
            TITLE_BAR_HEIGHT
        } else {
            0
        };
        let metrics = SsdFrameMetrics::from_frame_origin((0, 0).into(), (0, 0).into(), bw, title_h);
        let left = metrics.client_origin.x - metrics.frame_origin.x;
        let top = metrics.client_origin.y - metrics.frame_origin.y;
        let right = metrics.frame_rect.loc.x + metrics.frame_rect.size.w
            - (metrics.client_rect.loc.x + metrics.client_rect.size.w);
        let bottom = metrics.frame_rect.loc.y + metrics.frame_rect.size.h
            - (metrics.client_rect.loc.y + metrics.client_rect.size.h);
        (left, top, right, bottom)
    }
}

#[cfg(test)]
mod tests {
    use smithay::utils::{Point, Size};

    use super::{SsdChromeMetrics, SsdFrameMetrics, SSD_RESIZE_HANDLE_THICKNESS};

    #[test]
    fn metrics_from_frame_origin_match_expected_client_and_frame_geometry() {
        let metrics = SsdFrameMetrics::from_frame_origin((0, 0).into(), (640, 400).into(), 2, 32);

        assert_eq!(metrics.frame_origin, Point::from((0, 0)));
        assert_eq!(metrics.client_origin, Point::from((2, 34)));
        assert_eq!(metrics.client_size, Size::from((640, 400)));
        assert_eq!(metrics.frame_size, Size::from((644, 436)));
        assert_eq!(metrics.frame_rect.loc, Point::from((0, 0)));
        assert_eq!(metrics.frame_rect.size, Size::from((644, 436)));
        assert_eq!(metrics.client_rect.loc, Point::from((2, 34)));
        assert_eq!(metrics.client_rect.size, Size::from((640, 400)));
        assert_eq!(metrics.titlebar_rect.loc, Point::from((0, 0)));
        assert_eq!(metrics.titlebar_rect.size, Size::from((644, 34)));
    }

    #[test]
    fn metrics_from_client_origin_reconstructs_frame_origin() {
        let metrics = SsdFrameMetrics::from_client_origin((2, 34).into(), (640, 400).into(), 2, 32);

        assert_eq!(metrics.frame_origin, Point::from((0, 0)));
        assert_eq!(metrics.client_origin, Point::from((2, 34)));
        assert_eq!(metrics.frame_size, Size::from((644, 436)));
    }

    #[test]
    fn zero_titlebar_case_keeps_top_inset_to_border_only() {
        let metrics = SsdFrameMetrics::from_frame_origin((10, 20).into(), (640, 400).into(), 2, 0);

        assert_eq!(metrics.client_origin, Point::from((12, 22)));
        assert_eq!(metrics.frame_size, Size::from((644, 404)));
        assert_eq!(metrics.titlebar_rect.loc, Point::from((10, 20)));
        assert_eq!(metrics.titlebar_rect.size, Size::from((644, 2)));
    }

    #[test]
    fn button_rects_match_current_render_and_hit_formulas() {
        let frame = SsdFrameMetrics::from_frame_origin((0, 0).into(), (640, 400).into(), 2, 32);
        let chrome = SsdChromeMetrics::new(frame);
        let buttons = chrome.button_metrics().expect("titlebar buttons");

        assert_eq!(buttons.close_rect.loc, Point::from((620, 10)));
        assert_eq!(buttons.maximize_rect.loc, Point::from((600, 10)));
        assert_eq!(buttons.minimize_rect.loc, Point::from((580, 10)));
        assert_eq!(buttons.close_rect.size, Size::from((16, 16)));
        assert_eq!(buttons.maximize_rect.size, Size::from((16, 16)));
        assert_eq!(buttons.minimize_rect.size, Size::from((16, 16)));
    }

    #[test]
    fn button_rects_are_absent_when_titlebar_is_hidden() {
        let frame = SsdFrameMetrics::from_frame_origin((100, 200).into(), (640, 400).into(), 2, 0);
        let chrome = SsdChromeMetrics::new(frame);
        assert!(chrome.button_metrics().is_none());
    }

    #[test]
    fn offset_and_inset_match_existing_formulas_for_common_states() {
        let floating = SsdChromeMetrics::new(SsdFrameMetrics::from_frame_origin(
            (0, 0).into(),
            (640, 400).into(),
            2,
            32,
        ));
        assert_eq!(floating.decoration_offset(), (2, 34));
        assert_eq!(floating.decoration_inset(), (2, 34, 2, 2));

        let maximized = SsdChromeMetrics::new(SsdFrameMetrics::from_frame_origin(
            (0, 0).into(),
            (640, 400).into(),
            0,
            32,
        ));
        assert_eq!(maximized.decoration_offset(), (0, 32));
        assert_eq!(maximized.decoration_inset(), (0, 32, 0, 0));

        let no_titlebar = SsdChromeMetrics::new(SsdFrameMetrics::from_frame_origin(
            (0, 0).into(),
            (640, 400).into(),
            1,
            0,
        ));
        assert_eq!(no_titlebar.decoration_offset(), (1, 1));
        assert_eq!(no_titlebar.decoration_inset(), (1, 1, 1, 1));

        let no_decor = SsdChromeMetrics::new(SsdFrameMetrics::from_frame_origin(
            (0, 0).into(),
            (640, 400).into(),
            0,
            0,
        ));
        assert_eq!(no_decor.decoration_offset(), (0, 0));
        assert_eq!(no_decor.decoration_inset(), (0, 0, 0, 0));
    }

    #[test]
    fn resize_bands_and_corners_match_hit_region_edges() {
        let frame = SsdFrameMetrics::from_frame_origin((0, 0).into(), (640, 400).into(), 2, 32);
        let chrome = SsdChromeMetrics::new(frame);
        let bands = chrome.resize_band_metrics().expect("resize bands");

        assert_eq!(bands.thickness, SSD_RESIZE_HANDLE_THICKNESS);
        assert_eq!(bands.top_band.loc, Point::from((0, 0)));
        assert_eq!(bands.top_band.size, Size::from((644, 8)));
        assert_eq!(bands.left_band.loc, Point::from((0, 0)));
        assert_eq!(bands.left_band.size, Size::from((8, 436)));
        assert_eq!(bands.right_band.loc, Point::from((636, 0)));
        assert_eq!(bands.right_band.size, Size::from((8, 436)));
        assert_eq!(bands.bottom_band.loc, Point::from((0, 428)));
        assert_eq!(bands.bottom_band.size, Size::from((644, 8)));
        assert_eq!(bands.top_left_corner.loc, Point::from((0, 0)));
        assert_eq!(bands.top_right_corner.loc, Point::from((636, 0)));
        assert_eq!(bands.bottom_left_corner.loc, Point::from((0, 428)));
        assert_eq!(bands.bottom_right_corner.loc, Point::from((636, 428)));
    }

    #[test]
    fn shadow_extent_matches_current_buffer_formula() {
        let frame = SsdFrameMetrics::from_frame_origin((10, 20).into(), (640, 400).into(), 2, 32);
        let chrome = SsdChromeMetrics::new(frame);
        let shadow = chrome.shadow_metrics(16).expect("shadow metrics");

        assert_eq!(shadow.rect.loc, Point::from((-6, 4)));
        assert_eq!(shadow.rect.size, Size::from((676, 466)));
    }

    #[test]
    fn shadow_extent_absent_without_border() {
        let frame = SsdFrameMetrics::from_frame_origin((0, 0).into(), (640, 400).into(), 0, 32);
        let chrome = SsdChromeMetrics::new(frame);
        assert!(chrome.shadow_metrics(16).is_none());
    }
}
