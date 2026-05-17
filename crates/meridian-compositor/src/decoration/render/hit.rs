use meridian_config::Decorations;
use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle, Size},
};

use super::super::{DecorationHit, DecorationManager, DecorationResizeEdge};
use super::geometry::{SsdChromeMetrics, SsdFrameMetrics};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SsdFrameHitRegion {
    OutsideFrame,
    ClientContent,
    TitleBar,
    CloseButton,
    MaximizeButton,
    MinimizeButton,
    TopBorder,
    LeftBorder,
    RightBorder,
    BottomBorder,
    TopLeftCorner,
    TopRightCorner,
    BottomLeftCorner,
    BottomRightCorner,
}

pub(crate) fn classify_ssd_frame_hit(
    pointer_pos: Point<f64, Logical>,
    metrics: SsdFrameMetrics,
) -> SsdFrameHitRegion {
    let chrome = SsdChromeMetrics::new(metrics);
    let frame = chrome.frame;
    let px = pointer_pos.x as i32;
    let py = pointer_pos.y as i32;
    let frame_left = frame.frame_origin.x;
    let frame_top = frame.frame_origin.y;
    let frame_right = frame_left + frame.frame_size.w;
    let frame_bottom = frame_top + frame.frame_size.h;

    if px < frame_left || py < frame_top || px >= frame_right || py >= frame_bottom {
        return SsdFrameHitRegion::OutsideFrame;
    }

    if let Some(buttons) = chrome.button_metrics() {
        if point_in_rect(px, py, buttons.close_rect) {
            return SsdFrameHitRegion::CloseButton;
        }
        if point_in_rect(px, py, buttons.maximize_rect) {
            return SsdFrameHitRegion::MaximizeButton;
        }
        if point_in_rect(px, py, buttons.minimize_rect) {
            return SsdFrameHitRegion::MinimizeButton;
        }
    }

    if let Some(resize) = chrome.resize_band_metrics() {
        if point_in_rect(px, py, resize.top_left_corner) {
            return SsdFrameHitRegion::TopLeftCorner;
        }
        if point_in_rect(px, py, resize.top_right_corner) {
            return SsdFrameHitRegion::TopRightCorner;
        }
        if point_in_rect(px, py, resize.bottom_left_corner) {
            return SsdFrameHitRegion::BottomLeftCorner;
        }
        if point_in_rect(px, py, resize.bottom_right_corner) {
            return SsdFrameHitRegion::BottomRightCorner;
        }
        if point_in_rect(px, py, resize.top_band) {
            return SsdFrameHitRegion::TopBorder;
        }
        if point_in_rect(px, py, resize.left_band) {
            return SsdFrameHitRegion::LeftBorder;
        }
        if point_in_rect(px, py, resize.right_band) {
            return SsdFrameHitRegion::RightBorder;
        }
        if point_in_rect(px, py, resize.bottom_band) {
            return SsdFrameHitRegion::BottomBorder;
        }
    }

    if point_in_rect(px, py, frame.titlebar_rect) {
        return SsdFrameHitRegion::TitleBar;
    }

    SsdFrameHitRegion::ClientContent
}

fn point_in_rect(px: i32, py: i32, rect: Rectangle<i32, Logical>) -> bool {
    let right = rect.loc.x + rect.size.w;
    let bottom = rect.loc.y + rect.size.h;
    px >= rect.loc.x && px < right && py >= rect.loc.y && py < bottom
}

impl DecorationManager {
    pub fn hit_test(
        &self,
        surface: &WlSurface,
        pointer_pos: Point<f64, Logical>,
        window_loc: Point<i32, Logical>,
        content_size: Size<i32, Logical>,
        theme: &Decorations,
    ) -> Option<DecorationHit> {
        let deco = self.decorations.get(&Self::key(surface))?;
        if !deco.should_draw() || !deco.should_draw_title_bar() {
            return None;
        }

        let metrics = self.ssd_render_metrics(surface, window_loc, content_size, theme);

        match classify_ssd_frame_hit(pointer_pos, metrics) {
            SsdFrameHitRegion::CloseButton => Some(DecorationHit::CloseButton),
            SsdFrameHitRegion::MaximizeButton => Some(DecorationHit::MaximizeButton),
            SsdFrameHitRegion::MinimizeButton => Some(DecorationHit::MinimizeButton),
            SsdFrameHitRegion::TopBorder => Some(DecorationHit::Resize(DecorationResizeEdge::Top)),
            SsdFrameHitRegion::LeftBorder => {
                Some(DecorationHit::Resize(DecorationResizeEdge::Left))
            }
            SsdFrameHitRegion::RightBorder => {
                Some(DecorationHit::Resize(DecorationResizeEdge::Right))
            }
            SsdFrameHitRegion::BottomBorder => {
                Some(DecorationHit::Resize(DecorationResizeEdge::Bottom))
            }
            SsdFrameHitRegion::TopLeftCorner => {
                Some(DecorationHit::Resize(DecorationResizeEdge::TopLeft))
            }
            SsdFrameHitRegion::TopRightCorner => {
                Some(DecorationHit::Resize(DecorationResizeEdge::TopRight))
            }
            SsdFrameHitRegion::BottomLeftCorner => {
                Some(DecorationHit::Resize(DecorationResizeEdge::BottomLeft))
            }
            SsdFrameHitRegion::BottomRightCorner => {
                Some(DecorationHit::Resize(DecorationResizeEdge::BottomRight))
            }
            SsdFrameHitRegion::TitleBar => Some(DecorationHit::TitleBar),
            SsdFrameHitRegion::OutsideFrame | SsdFrameHitRegion::ClientContent => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_ssd_frame_hit, SsdFrameHitRegion};
    use crate::decoration::render::geometry::SsdFrameMetrics;

    #[test]
    fn titlebar_point_hits_titlebar_region() {
        let metrics = SsdFrameMetrics::from_frame_origin((0, 0).into(), (640, 400).into(), 2, 32);
        let hit = classify_ssd_frame_hit((120.0, 10.0).into(), metrics);
        assert_eq!(hit, SsdFrameHitRegion::TitleBar);
    }

    #[test]
    fn titlebar_lower_boundary_is_exclusive() {
        let metrics = SsdFrameMetrics::from_frame_origin((0, 0).into(), (640, 400).into(), 2, 32);
        let hit = classify_ssd_frame_hit((120.0, 34.0).into(), metrics);
        assert_eq!(hit, SsdFrameHitRegion::ClientContent);
    }

    #[test]
    fn resize_top_band_precedence_is_before_titlebar() {
        let metrics = SsdFrameMetrics::from_frame_origin((0, 0).into(), (640, 400).into(), 2, 32);
        let hit = classify_ssd_frame_hit((120.0, 1.0).into(), metrics);
        assert_eq!(hit, SsdFrameHitRegion::TopBorder);
    }

    #[test]
    fn fractional_pointer_coordinates_keep_truncation_behavior_at_titlebar_boundary() {
        let metrics = SsdFrameMetrics::from_frame_origin((0, 0).into(), (640, 400).into(), 2, 32);
        let titlebar_hit = classify_ssd_frame_hit((120.9, 33.9).into(), metrics);
        let client_hit = classify_ssd_frame_hit((120.9, 34.1).into(), metrics);
        assert_eq!(titlebar_hit, SsdFrameHitRegion::TitleBar);
        assert_eq!(client_hit, SsdFrameHitRegion::ClientContent);
    }

    #[test]
    fn button_points_hit_close_maximize_and_minimize_regions() {
        let metrics = SsdFrameMetrics::from_frame_origin((0, 0).into(), (640, 400).into(), 2, 32);
        assert_eq!(
            classify_ssd_frame_hit((620.0, 18.0).into(), metrics),
            SsdFrameHitRegion::CloseButton
        );
        assert_eq!(
            classify_ssd_frame_hit((584.0, 18.0).into(), metrics),
            SsdFrameHitRegion::MaximizeButton
        );
        assert_eq!(
            classify_ssd_frame_hit((548.0, 18.0).into(), metrics),
            SsdFrameHitRegion::MinimizeButton
        );
    }

    #[test]
    fn border_points_hit_left_right_top_and_bottom_regions() {
        let metrics = SsdFrameMetrics::from_frame_origin((0, 0).into(), (640, 400).into(), 2, 32);
        assert_eq!(
            classify_ssd_frame_hit((1.0, 220.0).into(), metrics),
            SsdFrameHitRegion::LeftBorder
        );
        assert_eq!(
            classify_ssd_frame_hit((643.0, 220.0).into(), metrics),
            SsdFrameHitRegion::RightBorder
        );
        assert_eq!(
            classify_ssd_frame_hit((320.0, 1.0).into(), metrics),
            SsdFrameHitRegion::TopBorder
        );
        assert_eq!(
            classify_ssd_frame_hit((100.0, 435.0).into(), metrics),
            SsdFrameHitRegion::BottomBorder
        );
    }

    #[test]
    fn thin_visual_border_still_has_practical_resize_hit_area() {
        let metrics = SsdFrameMetrics::from_frame_origin((0, 0).into(), (640, 400).into(), 2, 32);
        assert_eq!(
            classify_ssd_frame_hit((6.0, 220.0).into(), metrics),
            SsdFrameHitRegion::LeftBorder
        );
        assert_eq!(
            classify_ssd_frame_hit((638.0, 220.0).into(), metrics),
            SsdFrameHitRegion::RightBorder
        );
        assert_eq!(
            classify_ssd_frame_hit((320.0, 430.0).into(), metrics),
            SsdFrameHitRegion::BottomBorder
        );
    }

    #[test]
    fn border_corner_points_hit_top_and_bottom_corner_regions() {
        let metrics = SsdFrameMetrics::from_frame_origin((0, 0).into(), (640, 400).into(), 2, 32);
        assert_eq!(
            classify_ssd_frame_hit((1.0, 1.0).into(), metrics),
            SsdFrameHitRegion::TopLeftCorner
        );
        assert_eq!(
            classify_ssd_frame_hit((642.0, 1.0).into(), metrics),
            SsdFrameHitRegion::TopRightCorner
        );
        assert_eq!(
            classify_ssd_frame_hit((1.0, 435.0).into(), metrics),
            SsdFrameHitRegion::BottomLeftCorner
        );
        assert_eq!(
            classify_ssd_frame_hit((642.0, 435.0).into(), metrics),
            SsdFrameHitRegion::BottomRightCorner
        );
    }

    #[test]
    fn client_content_point_hits_client_region() {
        let metrics = SsdFrameMetrics::from_frame_origin((0, 0).into(), (640, 400).into(), 2, 32);
        let hit = classify_ssd_frame_hit((320.0, 100.0).into(), metrics);
        assert_eq!(hit, SsdFrameHitRegion::ClientContent);
    }

    #[test]
    fn point_outside_frame_misses() {
        let metrics = SsdFrameMetrics::from_frame_origin((0, 0).into(), (640, 400).into(), 2, 32);
        let hit = classify_ssd_frame_hit((-1.0, 10.0).into(), metrics);
        assert_eq!(hit, SsdFrameHitRegion::OutsideFrame);
    }
}
