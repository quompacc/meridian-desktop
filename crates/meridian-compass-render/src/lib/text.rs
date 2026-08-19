// guard:allow-file: Kompass-Brand-Asset nur für Login/Bootsplash (Manifest §§11–12).

fn draw_signature(pm: &mut PixmapMut, font: &FontRef<'_>, w: f32, h: f32, style: &Style) {
    let size = (h * 0.042).clamp(28.0, 56.0);
    let y = h * 0.93;
    draw_text_centered(pm, font, size, w / 2.0, y, "QuompaCC", style.signature);
}

fn draw_cardinals(pm: &mut PixmapMut, font: &FontRef<'_>, cx: f32, cy: f32, r: f32, style: &Style) {
    let size = (r * 0.10).max(14.0);
    let label_radius = r * 0.76;

    let labels = [
        (0.0_f32, "N", style.north),
        (90.0, "O", style.cardinal_other),
        (180.0, "S", style.cardinal_other),
        (270.0, "W", style.cardinal_other),
    ];

    for (deg, text, color) in labels {
        let rad = (deg - 90.0).to_radians();
        let tx = cx + label_radius * rad.cos();
        let ty = cy + label_radius * rad.sin();
        draw_text_centered(pm, font, size, tx, ty, text, color);
    }
}

fn draw_heading_mark(pm: &mut PixmapMut, cx: f32, cy: f32, r: f32, style: &Style) {
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
    paint.set_color(style.north);
    paint.anti_alias = true;
    pm.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

/// Measurements for a piece of text laid out in a given font + size.
struct TextMetrics {
    total_advance: f32,
    text_h: f32,
    max_y: f32,
}

fn measure_text(font: &FontRef<'_>, size: f32, text: &str) -> TextMetrics {
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
    TextMetrics {
        total_advance,
        text_h,
        max_y,
    }
}

/// Rasterizes `text` starting at left edge `pen_x` and given baseline,
/// returning the pen_x just past the last glyph (useful for caret placement).
fn draw_text_at_baseline(
    pm: &mut PixmapMut,
    font: &FontRef<'_>,
    size: f32,
    mut pen_x: f32,
    baseline_y: f32,
    text: &str,
    color: Color,
) -> f32 {
    let scaled = font.as_scaled(PxScale::from(size));
    let pm_w = pm.width() as i32;
    let pm_h = pm.height() as i32;
    let cr = (color.red() * 255.0) as u8;
    let cg = (color.green() * 255.0) as u8;
    let cb = (color.blue() * 255.0) as u8;
    let opacity = color.alpha();

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
                let a = (alpha * opacity * 255.0).clamp(0.0, 255.0) as u32;
                for (i, &c) in [cr, cg, cb].iter().enumerate() {
                    let dst = data[idx + i] as u32;
                    data[idx + i] = ((c as u32 * a + dst * (255 - a)) / 255) as u8;
                }
            });
        }
        pen_x += advance;
    }
    pen_x
}

fn draw_text_centered(
    pm: &mut PixmapMut,
    font: &FontRef<'_>,
    size: f32,
    cx: f32,
    cy: f32,
    text: &str,
    color: Color,
) {
    let m = measure_text(font, size, text);
    let pen_x = cx - m.total_advance / 2.0;
    let baseline_y = cy + m.text_h / 2.0 - m.max_y.max(0.0);
    draw_text_at_baseline(pm, font, size, pen_x, baseline_y, text, color);
}
