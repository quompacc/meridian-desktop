use tiny_skia::{
    Color, FillRule, FilterQuality, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap,
    PixmapPaint, Point, Rect, Shader, SpreadMode, Stroke, Transform,
};

const WALLPAPER_PNG: &[u8] = include_bytes!("../../../assets/bsd_wallpaper.png");

/// Fully composed static login background.
///
/// Wallpaper decoding, cover scaling, darkening, and guide-line rendering are
/// done once at startup. Redraws only copy this cached RGBA buffer before the
/// dynamic card, text, and cursor are painted.
pub(crate) struct LoginBackdrop {
    pixmap: Pixmap,
}

impl LoginBackdrop {
    pub(crate) fn new(width: u32, height: u32) -> Result<Self, &'static str> {
        let source = Pixmap::decode_png(WALLPAPER_PNG).map_err(|_| "wallpaper decode failed")?;
        let mut pixmap = Pixmap::new(width, height).ok_or("wallpaper allocation failed")?;

        draw_cover(&mut pixmap, &source);
        blur_cached_wallpaper(&mut pixmap)?;
        draw_tint(&mut pixmap, width as f32, height as f32);
        draw_compass_guides(&mut pixmap, width as f32, height as f32);

        Ok(Self { pixmap })
    }

    pub(crate) fn copy_rgba_to(&self, target: &mut [u8]) -> Result<(), &'static str> {
        if target.len() != self.pixmap.data().len() {
            return Err("wallpaper target size mismatch");
        }
        target.copy_from_slice(self.pixmap.data());
        Ok(())
    }
}

fn blur_cached_wallpaper(target: &mut Pixmap) -> Result<(), &'static str> {
    // A two-stage bicubic downsample/upscale produces a broad, stable
    // background blur without a per-frame convolution. This runs once during
    // LoginBackdrop construction; the resulting full-size RGBA frame is then
    // reused for every redraw.
    let small_w = (target.width() / 10).max(1);
    let small_h = (target.height() / 10).max(1);
    let mut small = Pixmap::new(small_w, small_h).ok_or("blur allocation failed")?;
    let downscale = (small_w as f32 / target.width() as f32)
        .min(small_h as f32 / target.height() as f32);
    let quality = PixmapPaint {
        quality: FilterQuality::Bicubic,
        ..Default::default()
    };
    small.draw_pixmap(
        0,
        0,
        target.as_ref(),
        &quality,
        Transform::from_scale(downscale, downscale),
        None,
    );

    target.fill(Color::TRANSPARENT);
    let upscale = (target.width() as f32 / small.width() as f32)
        .max(target.height() as f32 / small.height() as f32);
    target.draw_pixmap(
        0,
        0,
        small.as_ref(),
        &quality,
        Transform::from_scale(upscale, upscale),
        None,
    );
    Ok(())
}

fn draw_cover(target: &mut Pixmap, source: &Pixmap) {
    let scale = (target.width() as f32 / source.width() as f32)
        .max(target.height() as f32 / source.height() as f32);
    let drawn_w = source.width() as f32 * scale;
    let drawn_h = source.height() as f32 * scale;
    let tx = (target.width() as f32 - drawn_w) * 0.5;
    let ty = (target.height() as f32 - drawn_h) * 0.5;
    let paint = PixmapPaint {
        quality: FilterQuality::Bicubic,
        ..Default::default()
    };
    target.draw_pixmap(
        0,
        0,
        source.as_ref(),
        &paint,
        Transform::from_row(scale, 0.0, 0.0, scale, tx, ty),
        None,
    );
}

fn draw_tint(target: &mut Pixmap, width: f32, height: f32) {
    let shader = LinearGradient::new(
        Point::from_xy(0.0, 0.0),
        Point::from_xy(0.0, height),
        vec![
            GradientStop::new(0.0, Color::from_rgba8(14, 17, 22, 126)),
            GradientStop::new(0.48, Color::from_rgba8(18, 21, 26, 104)),
            GradientStop::new(1.0, Color::from_rgba8(8, 11, 15, 156)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(Color::from_rgba8(14, 17, 22, 126)));
    let paint = Paint {
        shader,
        anti_alias: false,
        ..Default::default()
    };
    let rect = Rect::from_xywh(0.0, 0.0, width, height).expect("valid output rect");
    target.fill_rect(rect, &paint, Transform::identity(), None);
}

fn draw_compass_guides(target: &mut Pixmap, width: f32, height: f32) {
    let cx = width * 0.5;
    let cy = height * 0.49;
    let radius = width.min(height) * 0.37;
    let line_color = Color::from_rgba8(164, 177, 190, 30);

    for factor in [0.48_f32, 0.68, 0.82, 1.0] {
        let Some(circle) = PathBuilder::from_circle(cx, cy, radius * factor) else {
            continue;
        };
        stroke(target, &circle, line_color, 1.0);
    }

    let mut axes = PathBuilder::new();
    axes.move_to(cx, 0.0);
    axes.line_to(cx, height);
    axes.move_to(cx - radius, cy);
    axes.line_to(cx + radius, cy);
    if let Some(path) = axes.finish() {
        stroke(target, &path, Color::from_rgba8(166, 182, 198, 38), 1.0);
    }

    let mut needle = PathBuilder::new();
    needle.move_to(cx, cy - radius * 0.72);
    needle.line_to(cx - radius * 0.035, cy - radius * 0.12);
    needle.line_to(cx, cy);
    needle.line_to(cx + radius * 0.035, cy - radius * 0.12);
    needle.close();
    if let Some(path) = needle.finish() {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(196, 207, 218, 50));
        paint.anti_alias = true;
        target.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    let marker = PathBuilder::from_circle(cx, cy - radius, 2.4);
    if let Some(marker) = marker {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(121, 166, 207, 190));
        paint.anti_alias = true;
        target.fill_path(
            &marker,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn stroke(target: &mut Pixmap, path: &tiny_skia::Path, color: Color, width: f32) {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    target.stroke_path(
        path,
        &paint,
        &Stroke {
            width,
            ..Default::default()
        },
        Transform::identity(),
        None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backdrop_renders_and_copies_typical_output() {
        let backdrop = LoginBackdrop::new(640, 360).expect("backdrop");
        let mut target = vec![0; 640 * 360 * 4];
        backdrop.copy_rgba_to(&mut target).expect("copy");
        assert!(target.chunks_exact(4).any(|px| px[0..3] != [0, 0, 0]));
    }

    #[test]
    fn backdrop_rejects_wrong_target_size() {
        let backdrop = LoginBackdrop::new(64, 64).expect("backdrop");
        assert!(backdrop.copy_rgba_to(&mut [0; 4]).is_err());
    }
}
