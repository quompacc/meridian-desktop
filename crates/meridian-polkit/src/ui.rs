// Tiny-skia / ab_glyph rendering of the polkit auth popup. Pure, no
// Wayland here. Output is RGBA which the wayland layer expects to be
// converted to BGRA (ARGB8888 little-endian) by the caller.
//
// Colors and corner radius come from the active meridian theme so the
// popup matches whatever the rest of the desktop is wearing — light,
// dark, custom — instead of a hardcoded Tokyo Night palette.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use meridian_config::{Color as ThemeColor, ThemeConfig};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, PixmapMut, Transform};

pub const CARD_W: u32 = 520;
pub const CARD_H: u32 = 340;

const FIELD_W: f32 = 420.0;
const FIELD_H: f32 = 44.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Idle,
    Checking,
    Failed,
}

pub struct View<'a> {
    pub title: &'a str,
    pub message: &'a str,
    pub username: &'a str,
    pub password_len: usize,
    pub status: Status,
    pub hint: &'a str,
}

/// Render the popup into `pixels` (RGBA, width*height*4 bytes).
pub fn render(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    font: &FontRef<'_>,
    theme: &ThemeConfig,
    view: &View<'_>,
) {
    let colors = &theme.colors;
    let radius = (theme.decorations.corner_radius.max(8) as f32).min(20.0);

    let card_bg = u32_from_color(colors.surface_alt, 0xff);
    let text = u32_from_color(colors.text, 0xff);
    let dim = u32_from_color(colors.text_dim, 0xff);
    let accent = u32_from_color(colors.accent, 0xff);
    let border = u32_from_color(colors.border, 0xff);
    let field_bg = u32_from_color(colors.surface, 0xff);
    let err = u32_from_color(colors.error, 0xff);

    let mut pm = Pixmap::new(width, height).expect("pixmap");
    {
        let mut pm_mut = pm.as_mut();
        // Transparent outside — the rounded card sits over the screen.
        fill_rect(&mut pm_mut, 0.0, 0.0, width as f32, height as f32, 0.0, 0);

        let cx = width as f32 / 2.0;
        let cy = height as f32 / 2.0;
        let card_x = cx - CARD_W as f32 / 2.0;
        let card_y = cy - CARD_H as f32 / 2.0;

        // Subtle 1-px hairline border under the card for separation
        // against bright wallpapers (meridian-light).
        fill_rect(
            &mut pm_mut,
            card_x - 1.0,
            card_y - 1.0,
            CARD_W as f32 + 2.0,
            CARD_H as f32 + 2.0,
            radius + 1.0,
            border,
        );
        fill_rect(
            &mut pm_mut,
            card_x,
            card_y,
            CARD_W as f32,
            CARD_H as f32,
            radius,
            card_bg,
        );

        // Title (calligraphic ambition, plain Adwaita Sans for now — font
        // upgrade later once meridian-ui hands us an Italianno reference).
        draw_text_centered(&mut pm_mut, font, 22.0, cx, card_y + 36.0, view.title, text);

        // Short cyan accent rule below the title.
        let rule_w: f32 = 60.0;
        fill_rect(
            &mut pm_mut,
            cx - rule_w / 2.0,
            card_y + 56.0,
            rule_w,
            1.0,
            0.0,
            accent,
        );

        // Polkit message — 2-line max with ellipsis.
        let mut msg_y = card_y + 86.0;
        let max_w = CARD_W as f32 - 56.0;
        let msg_lines = wrap_text(font, 14.0, view.message, max_w, 2);
        for line in &msg_lines {
            draw_text_centered(&mut pm_mut, font, 14.0, cx, msg_y, line, dim);
            msg_y += 22.0;
        }

        // Identity — prominent. Username in full text color, label in dim.
        let id_label = "Anmelden als";
        let id_label_y = card_y + 148.0;
        draw_text_centered(&mut pm_mut, font, 12.0, cx, id_label_y, id_label, dim);
        let id_name_y = card_y + 166.0;
        draw_text_centered(&mut pm_mut, font, 18.0, cx, id_name_y, view.username, text);

        // Password field.
        let field_x = cx - FIELD_W / 2.0;
        let field_y = card_y + 208.0;
        let border_col = if view.status == Status::Failed { err } else { border };
        fill_rect(
            &mut pm_mut,
            field_x - 1.0,
            field_y - 1.0,
            FIELD_W + 2.0,
            FIELD_H + 2.0,
            10.0,
            border_col,
        );
        fill_rect(
            &mut pm_mut,
            field_x,
            field_y,
            FIELD_W,
            FIELD_H,
            9.0,
            field_bg,
        );

        // Bullet dots.
        let dot_r = 5.0;
        let dot_gap = 14.0;
        let dot_w = dot_r * 2.0 + dot_gap;
        let n = view.password_len.min(30);
        let total_w = n as f32 * dot_w - dot_gap;
        let dots_start_x = cx - total_w / 2.0 + dot_r;
        let dots_y = field_y + FIELD_H / 2.0;
        for i in 0..n {
            let dx = dots_start_x + i as f32 * dot_w;
            fill_circle(&mut pm_mut, dx, dots_y, dot_r, accent);
        }
        if view.password_len == 0 && view.status != Status::Checking {
            let m = measure_text(font, 14.0, "Passwort");
            draw_text(
                &mut pm_mut,
                font,
                14.0,
                field_x + (FIELD_W - m.total_advance) / 2.0,
                field_y + FIELD_H / 2.0 + 5.0,
                "Passwort",
                dim,
            );
        }

        // Status line.
        let (status_text, status_col) = match view.status {
            Status::Idle => (view.hint, dim),
            Status::Checking => ("Wird geprüft …", text),
            Status::Failed => ("Falsches Passwort", err),
        };
        draw_text_centered(
            &mut pm_mut,
            font,
            12.0,
            cx,
            card_y + CARD_H as f32 - 28.0,
            status_text,
            status_col,
        );
    }

    let src = pm.data();
    let n = (width * height * 4) as usize;
    pixels[..n].copy_from_slice(&src[..n]);
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }
}

// ── primitives ───────────────────────────────────────────────────────────────

fn u32_from_color(c: ThemeColor, alpha: u8) -> u32 {
    ((alpha as u32) << 24) | ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32)
}

pub fn fill_rect(pm: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, r: f32, col: u32) {
    let mut pb = PathBuilder::new();
    if r > 0.0 {
        pb.move_to(x + r, y);
        pb.line_to(x + w - r, y);
        pb.quad_to(x + w, y, x + w, y + r);
        pb.line_to(x + w, y + h - r);
        pb.quad_to(x + w, y + h, x + w - r, y + h);
        pb.line_to(x + r, y + h);
        pb.quad_to(x, y + h, x, y + h - r);
        pb.line_to(x, y + r);
        pb.quad_to(x, y, x + r, y);
    } else {
        pb.move_to(x, y);
        pb.line_to(x + w, y);
        pb.line_to(x + w, y + h);
        pb.line_to(x, y + h);
    }
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(rgba_to_color(col));
        paint.anti_alias = true;
        pm.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn fill_circle(pm: &mut PixmapMut, cx: f32, cy: f32, r: f32, col: u32) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(rgba_to_color(col));
        paint.anti_alias = true;
        pm.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn rgba_to_color(col: u32) -> Color {
    let a = ((col >> 24) & 0xff) as u8;
    let r = ((col >> 16) & 0xff) as u8;
    let g = ((col >> 8) & 0xff) as u8;
    let b = (col & 0xff) as u8;
    Color::from_rgba8(r, g, b, a)
}

// ── text ─────────────────────────────────────────────────────────────────────

pub struct TextMetrics {
    pub total_advance: f32,
    pub ascent: f32,
}

pub fn measure_text(font: &FontRef<'_>, size: f32, text: &str) -> TextMetrics {
    let scaled = font.as_scaled(PxScale::from(size));
    let mut advance = 0.0;
    for ch in text.chars() {
        advance += scaled.h_advance(scaled.glyph_id(ch));
    }
    TextMetrics {
        total_advance: advance,
        ascent: scaled.ascent(),
    }
}

pub fn draw_text(
    pm: &mut PixmapMut,
    font: &FontRef<'_>,
    size: f32,
    pen_x: f32,
    baseline_y: f32,
    text: &str,
    col: u32,
) {
    let scaled = font.as_scaled(PxScale::from(size));
    let pw = pm.width() as i32;
    let ph = pm.height() as i32;
    let a_f = ((col >> 24) & 0xff) as f32 / 255.0;
    let cr = ((col >> 16) & 0xff) as u8;
    let cg = ((col >> 8) & 0xff) as u8;
    let cb = (col & 0xff) as u8;
    let mut x = pen_x;
    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        let glyph =
            id.with_scale_and_position(PxScale::from(size), ab_glyph::point(x, baseline_y));
        if let Some(outline) = font.outline_glyph(glyph) {
            let b = outline.px_bounds();
            outline.draw(|gx, gy, alpha| {
                let px = b.min.x as i32 + gx as i32;
                let py = b.min.y as i32 + gy as i32;
                if px < 0 || py < 0 || px >= pw || py >= ph {
                    return;
                }
                let idx = (py as usize * pw as usize + px as usize) * 4;
                let data = pm.data_mut();
                let a = (alpha * a_f * 255.0).clamp(0.0, 255.0) as u32;
                for (i, &c) in [cr, cg, cb].iter().enumerate() {
                    let dst = data[idx + i] as u32;
                    data[idx + i] = ((c as u32 * a + dst * (255 - a)) / 255) as u8;
                }
            });
        }
        x += scaled.h_advance(id);
    }
}

pub fn draw_text_centered(
    pm: &mut PixmapMut,
    font: &FontRef<'_>,
    size: f32,
    cx: f32,
    top_y: f32,
    text: &str,
    col: u32,
) {
    let m = measure_text(font, size, text);
    let pen_x = cx - m.total_advance / 2.0;
    let baseline_y = top_y + m.ascent;
    draw_text(pm, font, size, pen_x, baseline_y, text, col);
}

fn wrap_text(
    font: &FontRef<'_>,
    size: f32,
    text: &str,
    max_w: f32,
    max_lines: usize,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", current, word)
        };
        if measure_text(font, size, &candidate).total_advance > max_w && !current.is_empty() {
            lines.push(current.clone());
            if lines.len() == max_lines {
                let mut last = lines.pop().unwrap();
                last.push_str(" …");
                lines.push(last);
                return lines;
            }
            current = word.to_string();
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}
