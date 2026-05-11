use meridian_config::Decorations;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle, Size};

use super::super::{DecorationManager, TITLE_BAR_HEIGHT};

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

    #[cfg(test)]
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

        SsdFrameMetrics::from_frame_origin(window_loc, content_size, border_width, titlebar_height)
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

    use super::SsdFrameMetrics;

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
}
