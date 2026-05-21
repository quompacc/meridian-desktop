// Compass renderer — settle frame only.
//
// This is a near-duplicate of the bootsplash renderer with the animation
// loop stripped: we render one deterministic frame past the spin/settle.
// Phase 3 of the plan extracts this code (and the bootsplash equivalent)
// into a shared crate `meridian-compass-render` so both binaries link the
// same source of truth.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use tiny_skia::{
    BlendMode, Color, FillRule, Paint, PathBuilder, PixmapMut, Rect, Shader, Stroke, Transform,
};

static FONT_BYTES: &[u8] = include_bytes!("../assets/DejaVuSans-Bold.ttf");
static SCRIPT_FONT_BYTES: &[u8] = include_bytes!("../assets/Italianno-Regular.ttf");

// `t` value chosen so the bootsplash animation has fully settled (1.6s spin
// + several damped oscillations) and the breathing term contributes a small,
// stable offset. Picking a fixed t makes the Phase 2 frame deterministic.
pub const SETTLE_T: f32 = 10.0;

fn north_color() -> Color {
    Color::from_rgba8(120, 210, 255, 255)
}
fn south_color() -> Color {
    Color::from_rgba8(214, 92, 76, 255)
}

pub fn render_settle_frame(pm: &mut PixmapMut, w: f32, h: f32) {
    let cx = w / 2.0;
    let cy = h / 2.0;
    let r = (w.min(h) * 0.32).round();

    draw_background(pm, w, h, cx, cy);
    draw_meridian_lines(pm, cx, cy, r);
    draw_scale_ring(pm, cx, cy, r);
    draw_rose(pm, cx, cy, r * 0.72);
    let angle = needle_angle_deg(SETTLE_T);
    draw_needle_glow(pm, cx, cy, r * 0.78, angle);
    draw_needle(pm, cx, cy, r * 0.78, angle);
    draw_pivot(pm, cx, cy);
    draw_heading_mark(pm, cx, cy, r);
    draw_cardinals(pm, cx, cy, r);
    draw_signature(pm, w, h);
}

fn needle_angle_deg(t: f32) -> f32 {
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

fn draw_background(pm: &mut PixmapMut, w: f32, h: f32, cx: f32, cy: f32) {
    // tiny-skia 0.11 radial gradient: start_point, end_point, radius, stops, mode, transform.
    // With both points at center we get a standard center→outer radial fill.
    let shader = tiny_skia::RadialGradient::new(
        tiny_skia::Point::from_xy(cx, cy),
        tiny_skia::Point::from_xy(cx, cy),
        (w.max(h)) * 0.75,
        vec![
            tiny_skia::GradientStop::new(0.0, Color::from_rgba8(22, 30, 56, 255)),
            tiny_skia::GradientStop::new(0.55, Color::from_rgba8(10, 14, 28, 255)),
            tiny_skia::GradientStop::new(1.0, Color::from_rgba8(4, 6, 14, 255)),
        ],
        tiny_skia::SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(Color::from_rgba8(8, 10, 22, 255)));

    let paint = Paint {
        shader,
        ..Default::default()
    };
    let rect = Rect::from_xywh(0.0, 0.0, w, h).unwrap();
    pm.fill_rect(rect, &paint, Transform::identity(), None);
}

fn draw_meridian_lines(pm: &mut PixmapMut, cx: f32, cy: f32, r: f32) {
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(70, 100, 160, 36));
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

fn draw_scale_ring(pm: &mut PixmapMut, cx: f32, cy: f32, r: f32) {
    let mut ring_paint = Paint::default();
    ring_paint.set_color(Color::from_rgba8(210, 222, 240, 190));
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

    let tick_paint_minor = {
        let mut p = Paint::default();
        p.set_color(Color::from_rgba8(180, 200, 230, 120));
        p.anti_alias = true;
        p
    };
    let tick_paint_major = {
        let mut p = Paint::default();
        p.set_color(Color::from_rgba8(220, 232, 250, 220));
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

fn draw_rose(pm: &mut PixmapMut, cx: f32, cy: f32, len_main: f32) {
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
            Color::from_rgba8(235, 240, 250, 235)
        } else {
            Color::from_rgba8(155, 170, 200, 200)
        };
        let dark = if is_main {
            Color::from_rgba8(95, 110, 140, 235)
        } else {
            Color::from_rgba8(70, 80, 100, 200)
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

fn draw_needle(pm: &mut PixmapMut, cx: f32, cy: f32, length: f32, compass_deg: f32) {
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

    let mut pb_n = PathBuilder::new();
    pb_n.move_to(b1.0, b1.1);
    pb_n.line_to(tip_n.0, tip_n.1);
    pb_n.line_to(b2.0, b2.1);
    pb_n.close();
    let path_n = pb_n.finish().unwrap();
    let mut paint_n = Paint::default();
    paint_n.set_color(north_color());
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
    paint_s.set_color(south_color());
    paint_s.anti_alias = true;
    pm.fill_path(
        &path_s,
        &paint_s,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn draw_needle_glow(pm: &mut PixmapMut, cx: f32, cy: f32, length: f32, compass_deg: f32) {
    let rad = (compass_deg - 90.0).to_radians();
    let tip = (cx + length * rad.cos(), cy + length * rad.sin());

    for (radius_mult, alpha) in [(0.18_f32, 24u8), (0.12, 50), (0.08, 110)] {
        let circle = PathBuilder::from_circle(tip.0, tip.1, length * radius_mult).unwrap();
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(120, 210, 255, alpha));
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

fn draw_pivot(pm: &mut PixmapMut, cx: f32, cy: f32) {
    let outer = PathBuilder::from_circle(cx, cy, 8.0).unwrap();
    let mut p_outer = Paint::default();
    p_outer.set_color(Color::from_rgba8(40, 50, 70, 240));
    p_outer.anti_alias = true;
    pm.fill_path(
        &outer,
        &p_outer,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    let inner = PathBuilder::from_circle(cx, cy, 4.0).unwrap();
    let mut p_inner = Paint::default();
    p_inner.set_color(Color::from_rgba8(220, 230, 245, 255));
    p_inner.anti_alias = true;
    pm.fill_path(
        &inner,
        &p_inner,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn draw_signature(pm: &mut PixmapMut, w: f32, h: f32) {
    let font = FontRef::try_from_slice(SCRIPT_FONT_BYTES).expect("script font load");
    let size = (h * 0.042).clamp(28.0, 56.0);
    let y = h * 0.93;
    let color = Color::from_rgba8(230, 236, 248, 140);
    draw_text_centered(pm, &font, size, w / 2.0, y, "QuompaCC", color);
}

fn draw_cardinals(pm: &mut PixmapMut, cx: f32, cy: f32, r: f32) {
    let font = FontRef::try_from_slice(FONT_BYTES).expect("font load");
    let size = (r * 0.10).max(14.0);
    let label_radius = r * 0.76;

    let labels = [
        (0.0_f32, "N", north_color()),
        (90.0, "O", Color::from_rgba8(225, 230, 240, 240)),
        (180.0, "S", Color::from_rgba8(225, 230, 240, 240)),
        (270.0, "W", Color::from_rgba8(225, 230, 240, 240)),
    ];

    for (deg, text, color) in labels {
        let rad = (deg - 90.0).to_radians();
        let tx = cx + label_radius * rad.cos();
        let ty = cy + label_radius * rad.sin();
        draw_text_centered(pm, &font, size, tx, ty, text, color);
    }
}

fn draw_text_centered(
    pm: &mut PixmapMut,
    font: &FontRef,
    size: f32,
    cx: f32,
    cy: f32,
    text: &str,
    color: Color,
) {
    let scaled = font.as_scaled(PxScale::from(size));

    let mut total_advance = 0.0_f32;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        total_advance += scaled.h_advance(id);
        if let Some(outline) = scaled.outline_glyph(id.with_scale(PxScale::from(size))) {
            let b = outline.px_bounds();
            min_y = min_y.min(b.min.y);
            max_y = max_y.max(b.max.y);
        }
    }
    let text_h = if max_y.is_finite() {
        max_y - min_y
    } else {
        size
    };

    let mut pen_x = cx - total_advance / 2.0;
    let baseline_y = cy + text_h / 2.0 - max_y.max(0.0);

    let pm_w = pm.width() as i32;
    let pm_h = pm.height() as i32;
    let cr = (color.red() * 255.0) as u8;
    let cg = (color.green() * 255.0) as u8;
    let cb = (color.blue() * 255.0) as u8;

    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        let advance = scaled.h_advance(id);
        let glyph =
            id.with_scale_and_position(PxScale::from(size), ab_glyph::point(pen_x, baseline_y));
        if let Some(outline) = font.outline_glyph(glyph) {
            let b = outline.px_bounds();
            outline.draw(|gx, gy, alpha| {
                let px = b.min.x as i32 + gx as i32;
                let py = b.min.y as i32 + gy as i32;
                if px < 0 || py < 0 || px >= pm_w || py >= pm_h {
                    return;
                }
                let idx = (py as usize * pm_w as usize + px as usize) * 4;
                let data = pm.data_mut();
                let a = (alpha * 255.0).clamp(0.0, 255.0) as u32;
                for (i, &c) in [cr, cg, cb].iter().enumerate() {
                    let dst = data[idx + i] as u32;
                    data[idx + i] = ((c as u32 * a + dst * (255 - a)) / 255) as u8;
                }
            });
        }
        pen_x += advance;
    }
}

fn draw_heading_mark(pm: &mut PixmapMut, cx: f32, cy: f32, r: f32) {
    let tip = (cx, cy - r * 0.97);
    let b1 = (cx - 7.0, cy - r * 1.06);
    let b2 = (cx + 7.0, cy - r * 1.06);
    let mut pb = PathBuilder::new();
    pb.move_to(tip.0, tip.1);
    pb.line_to(b1.0, b1.1);
    pb.line_to(b2.0, b2.1);
    pb.close();
    let path = pb.finish().unwrap();
    let mut paint = Paint::default();
    paint.set_color(north_color());
    paint.anti_alias = true;
    pm.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::Pixmap;

    #[test]
    fn settle_frame_renders_without_panic_at_typical_resolutions() {
        for (w, h) in [(1280u32, 720u32), (1920, 1080), (1920, 1440), (2560, 1440)] {
            let mut pm = Pixmap::new(w, h).expect("pixmap");
            let mut pmm = pm.as_mut();
            render_settle_frame(&mut pmm, w as f32, h as f32);

            // sanity: center pixel must not be transparent / fully black
            let idx = ((h / 2) as usize * w as usize + (w / 2) as usize) * 4;
            let data = pm.data();
            assert!(
                data[idx] != 0 || data[idx + 1] != 0 || data[idx + 2] != 0,
                "center pixel is fully black at {}x{}",
                w,
                h
            );
        }
    }

    #[test]
    fn needle_angle_finite_for_relevant_t_range() {
        for t in [0.0_f32, 0.5, 1.5, 1.6, 2.0, 5.0, SETTLE_T, 60.0] {
            let a = needle_angle_deg(t);
            assert!(a.is_finite(), "needle_angle_deg({}) = {} not finite", t, a);
        }
    }

    #[test]
    fn needle_angle_at_settle_is_near_north() {
        // After full settle (oscillation decayed), angle should land near
        // 1080° (= 3 full turns, equivalent to 0° = North). Allow a small
        // breathing offset.
        let a = needle_angle_deg(SETTLE_T);
        let off = (a - 1080.0).abs();
        assert!(off < 5.0, "settle angle {} too far from 1080°", a);
    }
}
