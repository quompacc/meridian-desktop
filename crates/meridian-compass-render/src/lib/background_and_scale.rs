// guard:allow-file: Kompass-Brand-Asset nur für Login/Bootsplash (Manifest §§11–12).

/// Compass needle angle in degrees as a function of animation time.
///
/// 0..1.6s: ease-out cubic spin ending at 1080° (= 0° mod 360, i.e. North).
/// 1.6s..:  damped oscillation around 1080° plus a gentle breathing term.

pub fn needle_angle_deg(t: f32) -> f32 {
    let spin = 1.6_f32;
    let breath = 1.4 * (t * 1.1).sin();
    if t < spin {
        let p = t / spin;
        let eased = 1.0 - (1.0 - p).powi(3);
        eased * 1080.0
    } else {
        let tt = t - spin;
        1080.0 + 70.0 * (-tt * 1.9).exp() * (tt * 6.2).cos() + breath
    }
}

// ---- internal layers ----

fn color_with_alpha(color: Color, alpha: u8) -> Color {
    Color::from_rgba8(
        (color.red() * 255.0) as u8,
        (color.green() * 255.0) as u8,
        (color.blue() * 255.0) as u8,
        alpha,
    )
}

fn draw_background(pm: &mut PixmapMut, w: f32, h: f32, cx: f32, cy: f32, t: f32, style: &Style) {
    let drift_x = (t * 0.18).sin() * w.min(h) * 0.018;
    let drift_y = (t * 0.14).cos() * w.min(h) * 0.014;
    let shader = tiny_skia::RadialGradient::new(
        tiny_skia::Point::from_xy(cx + drift_x, cy + drift_y),
        tiny_skia::Point::from_xy(cx, cy),
        (w.max(h)) * 0.75,
        vec![
            tiny_skia::GradientStop::new(0.0, style.bg_stops[0]),
            tiny_skia::GradientStop::new(0.55, style.bg_stops[1]),
            tiny_skia::GradientStop::new(1.0, style.bg_stops[2]),
        ],
        tiny_skia::SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(style.bg_stops[2]));

    let paint = Paint {
        shader,
        ..Default::default()
    };
    let rect = Rect::from_xywh(0.0, 0.0, w, h).unwrap();
    pm.fill_rect(rect, &paint, Transform::identity(), None);

    let vignette = tiny_skia::RadialGradient::new(
        tiny_skia::Point::from_xy(cx, cy),
        tiny_skia::Point::from_xy(cx, cy),
        (w.max(h)) * 0.62,
        vec![
            tiny_skia::GradientStop::new(0.0, Color::from_rgba8(0, 0, 0, 0)),
            tiny_skia::GradientStop::new(0.72, Color::from_rgba8(0, 0, 0, 35)),
            tiny_skia::GradientStop::new(1.0, Color::from_rgba8(0, 0, 0, 150)),
        ],
        tiny_skia::SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(Color::from_rgba8(0, 0, 0, 80)));
    let vignette_paint = Paint {
        shader: vignette,
        blend_mode: BlendMode::SourceOver,
        ..Default::default()
    };
    pm.fill_rect(rect, &vignette_paint, Transform::identity(), None);
}

fn draw_compass_shadow(pm: &mut PixmapMut, cx: f32, cy: f32, r: f32, style: &Style) {
    for (offset_y, radius, alpha) in [(r * 0.10, r * 1.08, 46), (r * 0.05, r * 0.94, 34)] {
        let Some(path) = PathBuilder::from_circle(cx, cy + offset_y, radius) else {
            continue;
        };
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(0, 0, 0, alpha));
        paint.anti_alias = true;
        pm.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    let glass = tiny_skia::RadialGradient::new(
        tiny_skia::Point::from_xy(cx - r * 0.28, cy - r * 0.34),
        tiny_skia::Point::from_xy(cx, cy),
        r * 1.08,
        vec![
            tiny_skia::GradientStop::new(0.0, color_with_alpha(style.north, 34)),
            tiny_skia::GradientStop::new(0.44, Color::from_rgba8(34, 50, 84, 24)),
            tiny_skia::GradientStop::new(1.0, Color::from_rgba8(0, 0, 0, 0)),
        ],
        tiny_skia::SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(Color::from_rgba8(0, 0, 0, 0)));
    let Some(glass_path) = PathBuilder::from_circle(cx, cy, r * 1.02) else {
        return;
    };
    let glass_paint = Paint {
        shader: glass,
        blend_mode: BlendMode::Screen,
        anti_alias: true,
        ..Default::default()
    };
    pm.fill_path(
        &glass_path,
        &glass_paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn draw_meridian_lines(pm: &mut PixmapMut, cx: f32, cy: f32, r: f32, style: &Style) {
    let mut paint = Paint::default();
    paint.set_color(style.meridian);
    paint.anti_alias = true;
    let stroke = Stroke {
        width: 1.0,
        ..Default::default()
    };

    for i in 0..12 {
        let deg = i as f32 * 30.0;
        let rad = (deg - 90.0).to_radians();
        let (sx, sy) = (cx + r * 0.18 * rad.cos(), cy + r * 0.18 * rad.sin());
        let (ex, ey) = (cx + r * 1.85 * rad.cos(), cy + r * 1.85 * rad.sin());
        let mut pb = PathBuilder::new();
        pb.move_to(sx, sy);
        pb.line_to(ex, ey);
        let path = pb.finish().unwrap();
        pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

fn draw_scale_ring(pm: &mut PixmapMut, cx: f32, cy: f32, r: f32, style: &Style) {
    let mut lower_paint = Paint::default();
    lower_paint.set_color(Color::from_rgba8(0, 0, 0, 95));
    lower_paint.anti_alias = true;
    let lower_stroke = Stroke {
        width: 4.2,
        ..Default::default()
    };
    let lower = PathBuilder::from_circle(cx + 1.5, cy + 2.5, r).unwrap();
    pm.stroke_path(
        &lower,
        &lower_paint,
        &lower_stroke,
        Transform::identity(),
        None,
    );

    let mut ring_paint = Paint::default();
    ring_paint.set_color(style.ring);
    ring_paint.anti_alias = true;
    let ring_stroke = Stroke {
        width: 1.6,
        ..Default::default()
    };
    let outer = PathBuilder::from_circle(cx, cy, r).unwrap();
    pm.stroke_path(
        &outer,
        &ring_paint,
        &ring_stroke,
        Transform::identity(),
        None,
    );
    let inner = PathBuilder::from_circle(cx, cy, r * 0.92).unwrap();
    pm.stroke_path(
        &inner,
        &ring_paint,
        &ring_stroke,
        Transform::identity(),
        None,
    );

    let mut highlight_paint = Paint::default();
    highlight_paint.set_color(Color::from_rgba8(255, 255, 255, 75));
    highlight_paint.anti_alias = true;
    highlight_paint.blend_mode = BlendMode::Screen;
    let highlight_stroke = Stroke {
        width: 1.0,
        ..Default::default()
    };
    let highlight = PathBuilder::from_circle(cx - 1.0, cy - 1.5, r * 0.965).unwrap();
    pm.stroke_path(
        &highlight,
        &highlight_paint,
        &highlight_stroke,
        Transform::identity(),
        None,
    );

    let tick_paint_minor = {
        let mut p = Paint::default();
        p.set_color(style.tick_minor);
        p.anti_alias = true;
        p
    };
    let tick_paint_major = {
        let mut p = Paint::default();
        p.set_color(style.tick_major);
        p.anti_alias = true;
        p
    };
    let stroke_minor = Stroke {
        width: 1.0,
        ..Default::default()
    };
    let stroke_major = Stroke {
        width: 2.0,
        ..Default::default()
    };

    for i in 0..72 {
        let deg = i as f32 * 5.0;
        let rad = (deg - 90.0).to_radians();
        let is_major = i % 6 == 0;
        let r_inner = if is_major { r * 0.84 } else { r * 0.89 };
        let mut pb = PathBuilder::new();
        pb.move_to(cx + r_inner * rad.cos(), cy + r_inner * rad.sin());
        pb.line_to(cx + r * 0.92 * rad.cos(), cy + r * 0.92 * rad.sin());
        let path = pb.finish().unwrap();
        if is_major {
            pm.stroke_path(
                &path,
                &tick_paint_major,
                &stroke_major,
                Transform::identity(),
                None,
            );
        } else {
            pm.stroke_path(
                &path,
                &tick_paint_minor,
                &stroke_minor,
                Transform::identity(),
                None,
            );
        }
    }
}
