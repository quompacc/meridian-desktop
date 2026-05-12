use meridian_config::Decorations;
use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Size},
};

use super::super::{
    DecorationHit, DecorationManager, DecorationResizeEdge, BUTTON_MARGIN, BUTTON_SIZE,
};
use super::geometry::SsdFrameMetrics;

const RESIZE_HANDLE_THICKNESS: i32 = 8;

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
    let px = pointer_pos.x as i32;
    let py = pointer_pos.y as i32;
    let frame_left = metrics.frame_origin.x;
    let frame_top = metrics.frame_origin.y;
    let frame_right = frame_left + metrics.frame_size.w;
    let frame_bottom = frame_top + metrics.frame_size.h;

    if px < frame_left || py < frame_top || px >= frame_right || py >= frame_bottom {
        return SsdFrameHitRegion::OutsideFrame;
    }

    if metrics.titlebar_height > 0 {
        let close_x = frame_right - BUTTON_SIZE - BUTTON_MARGIN;
        let close_y =
            frame_top + (metrics.titlebar_height - BUTTON_SIZE) / 2 + metrics.border_width;
        let max_x = close_x - BUTTON_SIZE - BUTTON_MARGIN / 2;
        let min_x = max_x - BUTTON_SIZE - BUTTON_MARGIN / 2;
        let btn_bot = close_y + BUTTON_SIZE;

        if px >= close_x && px < close_x + BUTTON_SIZE && py >= close_y && py < btn_bot {
            return SsdFrameHitRegion::CloseButton;
        }
        if px >= max_x && px < max_x + BUTTON_SIZE && py >= close_y && py < btn_bot {
            return SsdFrameHitRegion::MaximizeButton;
        }
        if px >= min_x && px < min_x + BUTTON_SIZE && py >= close_y && py < btn_bot {
            return SsdFrameHitRegion::MinimizeButton;
        }
    }

    let bw = metrics.border_width;
    if bw > 0 {
        let resize_w = bw.max(RESIZE_HANDLE_THICKNESS);
        let at_top = py < frame_top + resize_w;
        let at_left = px < frame_left + resize_w;
        let at_right = px >= frame_right - resize_w;
        let at_bottom = py >= frame_bottom - resize_w;

        if at_left && at_top {
            return SsdFrameHitRegion::TopLeftCorner;
        }
        if at_right && at_top {
            return SsdFrameHitRegion::TopRightCorner;
        }
        if at_left && at_bottom {
            return SsdFrameHitRegion::BottomLeftCorner;
        }
        if at_right && at_bottom {
            return SsdFrameHitRegion::BottomRightCorner;
        }
        if at_top {
            return SsdFrameHitRegion::TopBorder;
        }
        if at_left {
            return SsdFrameHitRegion::LeftBorder;
        }
        if at_right {
            return SsdFrameHitRegion::RightBorder;
        }
        if at_bottom {
            return SsdFrameHitRegion::BottomBorder;
        }
    }

    if py < frame_top + metrics.titlebar_height + metrics.border_width {
        return SsdFrameHitRegion::TitleBar;
    }

    SsdFrameHitRegion::ClientContent
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
