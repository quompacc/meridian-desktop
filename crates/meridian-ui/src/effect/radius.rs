use tiny_skia::{Path, PathBuilder};

use crate::paint::Rect;

/// Build a closed rounded-rectangle path for the given pixel rect.
///
/// Returns `None` for degenerate rectangles (`width <= 0` or `height <= 0`)
/// or if path finalization fails.
pub fn rounded_rect_path(rect: Rect, radius: i32) -> Option<Path> {
    if rect.width <= 0 || rect.height <= 0 {
        return None;
    }

    let x = rect.x as f32;
    let y = rect.y as f32;
    let w = rect.width as f32;
    let h = rect.height as f32;
    let right = x + w;
    let bottom = y + h;
    let r = (radius.max(0) as f32).min(w * 0.5).min(h * 0.5);

    let mut pb = PathBuilder::new();
    if r <= 0.0 {
        pb.move_to(x, y);
        pb.line_to(right, y);
        pb.line_to(right, bottom);
        pb.line_to(x, bottom);
        pb.close();
        return pb.finish();
    }

    pb.move_to(x + r, y);
    pb.line_to(right - r, y);
    pb.quad_to(right, y, right, y + r);
    pb.line_to(right, bottom - r);
    pb.quad_to(right, bottom, right - r, bottom);
    pb.line_to(x + r, bottom);
    pb.quad_to(x, bottom, x, bottom - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

#[cfg(test)]
mod tests {
    use super::rounded_rect_path;
    use crate::paint::Rect;

    fn assert_bounds_eq(bounds: tiny_skia::Rect, rect: Rect) {
        let eps = 0.001_f32;
        assert!((bounds.left() - rect.x as f32).abs() < eps);
        assert!((bounds.top() - rect.y as f32).abs() < eps);
        assert!((bounds.width() - rect.width as f32).abs() < eps);
        assert!((bounds.height() - rect.height as f32).abs() < eps);
    }

    #[test]
    fn radius_zero_matches_rect_bounds() {
        let rect = Rect {
            x: 4,
            y: 6,
            width: 40,
            height: 20,
        };
        let path = rounded_rect_path(rect, 0).expect("path");
        assert_bounds_eq(path.bounds(), rect);
    }

    #[test]
    fn positive_radius_produces_non_empty_path() {
        let rect = Rect {
            x: 10,
            y: 12,
            width: 80,
            height: 32,
        };
        let path = rounded_rect_path(rect, 8).expect("path");
        assert!(!path.is_empty());
        assert_bounds_eq(path.bounds(), rect);
    }

    #[test]
    fn radius_is_clamped_to_half_extent() {
        let rect = Rect {
            x: 2,
            y: 2,
            width: 12,
            height: 8,
        };
        let path = rounded_rect_path(rect, 100).expect("path");
        assert!(!path.is_empty());
        assert_bounds_eq(path.bounds(), rect);
    }

    #[test]
    fn degenerate_rect_returns_none() {
        assert!(rounded_rect_path(
            Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 8
            },
            4
        )
        .is_none());
        assert!(rounded_rect_path(
            Rect {
                x: 0,
                y: 0,
                width: 8,
                height: -1
            },
            4
        )
        .is_none());
    }
}
