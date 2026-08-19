// guard:allow-file: Kompass-Brand-Asset nur für Login/Bootsplash (Manifest §§11–12).

fn draw_sweep_glint(pm: &mut PixmapMut, cx: f32, cy: f32, r: f32, t: f32, style: &Style) {
    let deg = (t * 18.0) % 360.0;
    let rad = (deg - 90.0).to_radians();
    let width = 0.18_f32;
    let inner = r * 0.18;
    let outer = r * 0.98;
    let p1 = (
        cx + inner * (rad - width).cos(),
        cy + inner * (rad - width).sin(),
    );
    let p2 = (cx + outer * rad.cos(), cy + outer * rad.sin());
    let p3 = (
        cx + inner * (rad + width).cos(),
        cy + inner * (rad + width).sin(),
    );

    let mut pb = PathBuilder::new();
    pb.move_to(p1.0, p1.1);
    pb.line_to(p2.0, p2.1);
    pb.line_to(p3.0, p3.1);
    pb.close();
    let Some(path) = pb.finish() else {
        return;
    };

    let mut paint = Paint::default();
    paint.set_color(color_with_alpha(style.north, 22));
    paint.anti_alias = true;
    paint.blend_mode = BlendMode::Screen;
    pm.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn draw_rose_shadow(pm: &mut PixmapMut, cx: f32, cy: f32, len_main: f32) {
    let len_filler = len_main * 0.55;
    let offset = len_main * 0.018;
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(0, 0, 0, 72));
    paint.anti_alias = true;

    for i in 0..8 {
        let deg = i as f32 * 45.0;
        let rad = (deg - 90.0).to_radians();
        let length = if i % 2 == 0 { len_main } else { len_filler };
        let base_half = length * 0.13;
        let ox = offset;
        let oy = offset * 1.45;

        let tip = (cx + ox + length * rad.cos(), cy + oy + length * rad.sin());
        let perp = rad + std::f32::consts::FRAC_PI_2;
        let b1 = (
            cx + ox + base_half * perp.cos(),
            cy + oy + base_half * perp.sin(),
        );
        let b2 = (
            cx + ox - base_half * perp.cos(),
            cy + oy - base_half * perp.sin(),
        );

        let mut pb = PathBuilder::new();
        pb.move_to(cx + ox, cy + oy);
        pb.line_to(b1.0, b1.1);
        pb.line_to(tip.0, tip.1);
        pb.line_to(b2.0, b2.1);
        pb.close();
        if let Some(path) = pb.finish() {
            pm.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
}

fn draw_rose(pm: &mut PixmapMut, cx: f32, cy: f32, len_main: f32, style: &Style) {
    let len_filler = len_main * 0.55;
    for i in 0..8 {
        let deg = i as f32 * 45.0;
        let rad = (deg - 90.0).to_radians();
        let is_main = i % 2 == 0;
        let length = if is_main { len_main } else { len_filler };
        let base_half = length * 0.13;

        let tip = (cx + length * rad.cos(), cy + length * rad.sin());
        let perp = rad + std::f32::consts::FRAC_PI_2;
        let b1 = (cx + base_half * perp.cos(), cy + base_half * perp.sin());
        let b2 = (cx - base_half * perp.cos(), cy - base_half * perp.sin());

        let light = if is_main {
            style.rose_main_light
        } else {
            style.rose_filler_light
        };
        let dark = if is_main {
            style.rose_main_dark
        } else {
            style.rose_filler_dark
        };

        let mut pb1 = PathBuilder::new();
        pb1.move_to(cx, cy);
        pb1.line_to(b1.0, b1.1);
        pb1.line_to(tip.0, tip.1);
        pb1.close();
        let path1 = pb1.finish().unwrap();
        let mut paint1 = Paint::default();
        paint1.set_color(light);
        paint1.anti_alias = true;
        pm.fill_path(
            &path1,
            &paint1,
            FillRule::Winding,
            Transform::identity(),
            None,
        );

        let mut pb2 = PathBuilder::new();
        pb2.move_to(cx, cy);
        pb2.line_to(tip.0, tip.1);
        pb2.line_to(b2.0, b2.1);
        pb2.close();
        let path2 = pb2.finish().unwrap();
        let mut paint2 = Paint::default();
        paint2.set_color(dark);
        paint2.anti_alias = true;
        pm.fill_path(
            &path2,
            &paint2,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn draw_needle(pm: &mut PixmapMut, cx: f32, cy: f32, length: f32, compass_deg: f32, style: &Style) {
    let rad = (compass_deg - 90.0).to_radians();
    let perp = rad + std::f32::consts::FRAC_PI_2;
    let base_half = length * 0.045;

    let tip_n = (cx + length * rad.cos(), cy + length * rad.sin());
    let tip_s = (
        cx - length * 0.85 * rad.cos(),
        cy - length * 0.85 * rad.sin(),
    );
    let b1 = (cx + base_half * perp.cos(), cy + base_half * perp.sin());
    let b2 = (cx - base_half * perp.cos(), cy - base_half * perp.sin());

    let shadow_offset = length * 0.012;
    let mut shadow_paint = Paint::default();
    shadow_paint.set_color(Color::from_rgba8(0, 0, 0, 92));
    shadow_paint.anti_alias = true;
    let mut pb_shadow = PathBuilder::new();
    pb_shadow.move_to(b1.0 + shadow_offset, b1.1 + shadow_offset);
    pb_shadow.line_to(tip_n.0 + shadow_offset, tip_n.1 + shadow_offset);
    pb_shadow.line_to(b2.0 + shadow_offset, b2.1 + shadow_offset);
    pb_shadow.line_to(tip_s.0 + shadow_offset, tip_s.1 + shadow_offset);
    pb_shadow.close();
    if let Some(path) = pb_shadow.finish() {
        pm.fill_path(
            &path,
            &shadow_paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    let mut pb_n = PathBuilder::new();
    pb_n.move_to(b1.0, b1.1);
    pb_n.line_to(tip_n.0, tip_n.1);
    pb_n.line_to(b2.0, b2.1);
    pb_n.close();
    let path_n = pb_n.finish().unwrap();
    let mut paint_n = Paint::default();
    paint_n.set_color(style.north);
    paint_n.anti_alias = true;
    pm.fill_path(
        &path_n,
        &paint_n,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    let mut pb_s = PathBuilder::new();
    pb_s.move_to(b1.0, b1.1);
    pb_s.line_to(tip_s.0, tip_s.1);
    pb_s.line_to(b2.0, b2.1);
    pb_s.close();
    let path_s = pb_s.finish().unwrap();
    let mut paint_s = Paint::default();
    paint_s.set_color(style.south);
    paint_s.anti_alias = true;
    pm.fill_path(
        &path_s,
        &paint_s,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    let mut highlight = Paint::default();
    highlight.set_color(Color::from_rgba8(255, 255, 255, 90));
    highlight.anti_alias = true;
    highlight.blend_mode = BlendMode::Screen;
    let stroke = Stroke {
        width: 1.2,
        ..Default::default()
    };
    let mut pb_hi = PathBuilder::new();
    pb_hi.move_to(b1.0, b1.1);
    pb_hi.line_to(tip_n.0, tip_n.1);
    if let Some(path) = pb_hi.finish() {
        pm.stroke_path(&path, &highlight, &stroke, Transform::identity(), None);
    }
}

fn draw_needle_glow(
    pm: &mut PixmapMut,
    cx: f32,
    cy: f32,
    length: f32,
    compass_deg: f32,
    style: &Style,
) {
    let rad = (compass_deg - 90.0).to_radians();
    let tip = (cx + length * rad.cos(), cy + length * rad.sin());
    let north_rgba8 = (
        (style.north.red() * 255.0) as u8,
        (style.north.green() * 255.0) as u8,
        (style.north.blue() * 255.0) as u8,
    );

    for (radius_mult, alpha) in [(0.18_f32, 24u8), (0.12, 50), (0.08, 110)] {
        let circle = PathBuilder::from_circle(tip.0, tip.1, length * radius_mult).unwrap();
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(
            north_rgba8.0,
            north_rgba8.1,
            north_rgba8.2,
            alpha,
        ));
        paint.anti_alias = true;
        paint.blend_mode = BlendMode::Screen;
        pm.fill_path(
            &circle,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn draw_pivot(pm: &mut PixmapMut, cx: f32, cy: f32, style: &Style) {
    let outer = PathBuilder::from_circle(cx, cy, 8.0).unwrap();
    let pivot_shader = tiny_skia::RadialGradient::new(
        tiny_skia::Point::from_xy(cx - 3.0, cy - 4.0),
        tiny_skia::Point::from_xy(cx, cy),
        12.0,
        vec![
            tiny_skia::GradientStop::new(0.0, Color::from_rgba8(255, 255, 255, 210)),
            tiny_skia::GradientStop::new(0.42, style.pivot_inner),
            tiny_skia::GradientStop::new(1.0, style.pivot_outer),
        ],
        tiny_skia::SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(style.pivot_outer));
    let p_outer = Paint {
        shader: pivot_shader,
        anti_alias: true,
        ..Default::default()
    };
    pm.fill_path(
        &outer,
        &p_outer,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    let inner = PathBuilder::from_circle(cx, cy, 4.0).unwrap();
    let mut p_inner = Paint::default();
    p_inner.set_color(style.pivot_inner);
    p_inner.anti_alias = true;
    pm.fill_path(
        &inner,
        &p_inner,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}
