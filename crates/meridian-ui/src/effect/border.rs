use tiny_skia::{Paint, Path, PixmapMut, Stroke, Transform};

use crate::style::Color;

/// Stroke a precomputed path as a border.
///
/// Render-loop contract: allocation-free for valid inputs.
pub fn paint_border(canvas: &mut PixmapMut<'_>, path: &Path, color: Color, stroke_width: f32) {
    if path.is_empty() || stroke_width <= 0.0 {
        return;
    }

    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    paint.set_color(to_tiny_skia_color(color));

    let stroke = Stroke {
        width: stroke_width,
        ..Stroke::default()
    };
    canvas.stroke_path(path, &paint, &stroke, Transform::identity(), None);
}

fn to_tiny_skia_color(color: Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(color.r, color.g, color.b, color.a)
}

#[cfg(test)]
mod tests {
    use tiny_skia::{PathBuilder, Pixmap};

    use crate::{
        effect::{paint_border, rounded_rect_path},
        paint::Rect,
        style::Palette,
    };

    #[test]
    fn border_paint_changes_pixels() {
        let mut pixmap = Pixmap::new(32, 32).expect("pixmap");
        let path = rounded_rect_path(
            Rect {
                x: 4,
                y: 4,
                width: 24,
                height: 24,
            },
            4,
        )
        .expect("path");

        paint_border(
            &mut pixmap.as_mut(),
            &path,
            Palette::TOKYO_NIGHT_METRO.accent,
            2.0,
        );

        let changed = pixmap
            .pixels()
            .iter()
            .any(|p| p.alpha() > 0 && p.blue() >= p.red() && p.blue() >= p.green());
        assert!(changed);
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
        paint_border(
            &mut pixmap.as_mut(),
            &path,
            Palette::TOKYO_NIGHT_METRO.accent,
            1.0,
        );
        assert_eq!(pixmap.data(), before.as_slice());
    }
}
