use tiny_skia::{FillRule, Paint, Path, PixmapMut, Transform};

use crate::style::Color;

/// Fill a precomputed path with a solid color.
///
/// Render-loop contract: allocation-free for valid inputs.
pub fn paint_fill(canvas: &mut PixmapMut<'_>, path: &Path, color: Color) {
    if path.is_empty() {
        return;
    }

    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    paint.set_color(to_tiny_skia_color(color));
    canvas.fill_path(path, &paint, FillRule::Winding, Transform::identity(), None);
}

fn to_tiny_skia_color(color: Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(color.r, color.g, color.b, color.a)
}

#[cfg(test)]
mod tests {
    use tiny_skia::{PathBuilder, Pixmap};

    use crate::{
        effect::{paint_fill, rounded_rect_path},
        paint::Rect,
        style::Palette,
    };

    #[test]
    fn fill_paint_changes_pixels() {
        let mut pixmap = Pixmap::new(32, 32).expect("pixmap");
        let path = rounded_rect_path(
            Rect {
                x: 6,
                y: 6,
                width: 20,
                height: 20,
            },
            4,
        )
        .expect("path");

        paint_fill(
            &mut pixmap.as_mut(),
            &path,
            Palette::TOKYO_NIGHT_METRO.accent_alt,
        );

        assert!(pixmap.pixels().iter().any(|p| p.alpha() > 0));
    }

    #[test]
    fn empty_like_path_is_noop() {
        let mut pb = PathBuilder::new();
        pb.move_to(10.0, 10.0);
        pb.line_to(10.0, 10.0);
        pb.close();
        let path = pb.finish().expect("path");

        let mut pixmap = Pixmap::new(32, 32).expect("pixmap");
        let before = pixmap.data().to_vec();
        paint_fill(
            &mut pixmap.as_mut(),
            &path,
            Palette::TOKYO_NIGHT_METRO.accent_alt,
        );
        assert_eq!(pixmap.data(), before.as_slice());
    }
}
