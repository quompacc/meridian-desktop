// meridian-login — Phase 5a: card content (title, labels, input boxes, caret).
//
// Animation timeline (t_anim relative to handover finish):
//   0.00..0.20s  hold the settle frame (handover-friendly)
//   0.20..1.40s  compass dims toward watermark; glow falls + grows
//   1.40..1.70s  card outline fades in over the glow
//   1.70..2.00s  card content (title, labels, boxes) fades in
//   2.00..12.00s caret blinks (Phase 5a temporary 10s hold; Phase 5b replaces
//                with actual keyboard input loop)
//   12.00s       release DRM master, exit
//
// Phase 5a is rendering-only. No input, no PAM.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use drm::buffer::DrmFourcc;
use drm::control::{connector, ClipRect, Device as ControlDevice};
use drm::Device as DrmDevice;

use meridian_compass_render::{CompassPainter, Fonts, FrameOpts, TextStyle, SETTLE_T};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, PixmapMut, Stroke, Transform};
use tracing::{info, warn};

const BOOTSPLASH_SOCKET: &str = "/run/bootsplash.sock";
const HANDOVER_SETTLE_MS: u64 = 200;

// Animation parameters
const WATERMARK_START_MS: u64 = 200;
const WATERMARK_END_MS: u64 = 1400;
const WATERMARK_FINAL_ALPHA: u8 = 180;
const FALL_END_MS: u64 = 1400;
const CARD_FADE_START_MS: u64 = 1400;
const CARD_FADE_END_MS: u64 = 1700;
const UI_FADE_START_MS: u64 = 1700;
const UI_FADE_END_MS: u64 = 2000;
const GLOW_HIDE_MS: u64 = 1700;
const GLOW_FINAL_SCALE: f32 = 4.0;
// Phase 5a temporary — Phase 5b replaces with input-driven exit
const LOGIN_HOLD_MS: u64 = 10_000;

struct Card(File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl DrmDevice for Card {}
impl ControlDevice for Card {}

#[derive(Default, Clone)]
struct LoginUiState {
    username: String,
    password_len: usize,
    focus: Field,
}

#[derive(Default, Clone, Copy, PartialEq)]
enum Field {
    #[default]
    Username,
    Password,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    info!("meridian-login starting (Phase 5a)");

    match bootsplash_handover() {
        Ok(()) => {
            info!("bootsplash handover acked");
            std::thread::sleep(Duration::from_millis(HANDOVER_SETTLE_MS));
        }
        Err(e) => warn!(error = %e, "bootsplash handover failed (not running?); proceeding"),
    }

    let card = Card(
        OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/dri/card0")?,
    );

    let res = card.resource_handles()?;
    let conn_info = res
        .connectors()
        .iter()
        .map(|&h| card.get_connector(h, false).unwrap())
        .find(|c| c.state() == connector::State::Connected)
        .ok_or("no connected connector")?;
    let mode = pick_mode(conn_info.modes()).ok_or("connector has no usable mode")?;
    let (w, h) = mode.size();
    let (w, h) = (w as u32, h as u32);
    info!(width = w, height = h, refresh = mode.vrefresh(), "drm mode");

    let crtc = if let Some(enc_h) = conn_info.current_encoder() {
        card.get_encoder(enc_h)?.crtc()
    } else {
        None
    }
    .unwrap_or_else(|| res.crtcs()[0]);

    let mut db = card.create_dumb_buffer((w, h), DrmFourcc::Xrgb8888, 32)?;
    let fb = card.add_framebuffer(&db, 24, 32)?;
    card.set_crtc(crtc, Some(fb), (0, 0), &[conn_info.handle()], Some(mode))?;

    let painter = CompassPainter::new(Fonts::quompacc())?;

    // Render the settle frame first so the bootsplash → login handover is
    // pixel-continuous before we start animating.
    {
        let mut mapping = card.map_dumb_buffer(&mut db)?;
        let buf = mapping.as_mut();
        let mut pm = PixmapMut::from_bytes(buf, w, h).ok_or("pixmap bind failed")?;
        painter.render(&mut pm, w as f32, h as f32, SETTLE_T, &FrameOpts::default());
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
    }
    let clip = ClipRect::new(0, 0, w as u16, h as u16);
    let _ = card.dirty_framebuffer(fb, &[clip]);
    info!("settle frame rendered");

    match bootsplash_exit() {
        Ok(()) => info!("bootsplash exit signalled"),
        Err(e) => warn!(error = %e, "bootsplash exit signal failed"),
    }

    let ui_state = LoginUiState::default();
    run_animation(
        &card,
        &mut db,
        fb,
        &painter,
        w,
        h,
        mode.vrefresh().max(60),
        &ui_state,
    )?;

    match card.release_master_lock() {
        Ok(()) => info!("drm: released master"),
        Err(e) => warn!(error = %e, "drm: release_master failed"),
    }

    info!("meridian-login exiting");
    Ok(())
}

struct AnimFrame {
    watermark_alpha: u8,
    glow_visible: bool,
    glow_pos: (f32, f32),
    glow_scale: f32,
    card_alpha: f32,
    ui_alpha: f32,
}

fn compute_anim_frame(t_anim_secs: f32, painter: &CompassPainter, w: f32, h: f32) -> AnimFrame {
    let t_ms = (t_anim_secs * 1000.0) as u64;

    let watermark_alpha = ramp_u8(
        t_ms,
        WATERMARK_START_MS,
        WATERMARK_END_MS,
        0,
        WATERMARK_FINAL_ALPHA,
    );

    let origin = painter.north_glow_position(w, h, SETTLE_T);
    let target = (w / 2.0, h / 2.0);
    let p_fall = (t_ms as f32 / FALL_END_MS as f32).clamp(0.0, 1.0);
    let p_eased = p_fall * p_fall;
    let glow_pos = (
        origin.0 + (target.0 - origin.0) * p_eased,
        origin.1 + (target.1 - origin.1) * p_eased,
    );
    let glow_scale = 1.0 + (GLOW_FINAL_SCALE - 1.0) * p_eased;
    let glow_visible = t_ms < GLOW_HIDE_MS;

    let card_alpha = ramp_f32(t_ms, CARD_FADE_START_MS, CARD_FADE_END_MS, 0.0, 1.0);
    let ui_alpha = ramp_f32(t_ms, UI_FADE_START_MS, UI_FADE_END_MS, 0.0, 1.0);

    AnimFrame {
        watermark_alpha,
        glow_visible,
        glow_pos,
        glow_scale,
        card_alpha,
        ui_alpha,
    }
}

fn ramp_u8(t: u64, start: u64, end: u64, from: u8, to: u8) -> u8 {
    if t <= start {
        from
    } else if t >= end {
        to
    } else {
        let p = (t - start) as f32 / (end - start) as f32;
        (from as f32 + (to as f32 - from as f32) * p) as u8
    }
}

fn ramp_f32(t: u64, start: u64, end: u64, from: f32, to: f32) -> f32 {
    if t <= start {
        from
    } else if t >= end {
        to
    } else {
        let p = (t - start) as f32 / (end - start) as f32;
        from + (to - from) * p
    }
}

#[allow(clippy::too_many_arguments)]
fn run_animation(
    card: &Card,
    db: &mut drm::control::dumbbuffer::DumbBuffer,
    fb: drm::control::framebuffer::Handle,
    painter: &CompassPainter,
    w: u32,
    h: u32,
    refresh_hz: u32,
    ui_state: &LoginUiState,
) -> Result<(), Box<dyn std::error::Error>> {
    let anim_start = Instant::now();
    let frame_dur = Duration::from_micros(1_000_000 / refresh_hz as u64);
    let total = Duration::from_millis(UI_FADE_END_MS + LOGIN_HOLD_MS);
    let mut frame_idx: u64 = 0;

    while anim_start.elapsed() < total {
        let t = anim_start.elapsed();
        let t_secs = t.as_secs_f32();
        let af = compute_anim_frame(t_secs, painter, w as f32, h as f32);

        // caret only blinks once the UI has fully faded in
        let caret_on = af.ui_alpha >= 1.0 && ((t_secs * 2.0) as i64) % 2 == 0;

        {
            let mut mapping = card.map_dumb_buffer(db)?;
            let buf = mapping.as_mut();
            let mut pm = PixmapMut::from_bytes(buf, w, h).ok_or("pixmap bind failed")?;

            painter.render(
                &mut pm,
                w as f32,
                h as f32,
                SETTLE_T,
                &FrameOpts {
                    include_north_glow: false,
                    watermark_alpha: af.watermark_alpha,
                    ..Default::default()
                },
            );

            if af.glow_visible {
                let r0 = painter.glow_base_radius(w as f32, h as f32);
                painter.render_glow_at(&mut pm, af.glow_pos.0, af.glow_pos.1, r0 * af.glow_scale);
            }

            if af.card_alpha > 0.0 {
                draw_card(&mut pm, w as f32, h as f32, af.card_alpha, painter);
            }

            if af.ui_alpha > 0.0 {
                draw_login_ui(&mut pm, w as f32, h as f32, painter, ui_state, af.ui_alpha, caret_on);
            }

            for px in buf.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
        }

        let clip = ClipRect::new(0, 0, w as u16, h as u16);
        let _ = card.dirty_framebuffer(fb, &[clip]);

        frame_idx += 1;
        let next = anim_start + frame_dur * frame_idx as u32;
        if let Some(wait) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
    }
    Ok(())
}

fn draw_card(pm: &mut PixmapMut, w: f32, h: f32, alpha: f32, painter: &CompassPainter) {
    let (left, top, cw, ch) = card_rect(w, h);
    let path = rounded_rect_path(left, top, cw, ch, 20.0);

    let mut fill = Paint::default();
    fill.set_color(Color::from_rgba8(32, 42, 76, (alpha * 235.0) as u8));
    fill.anti_alias = true;
    pm.fill_path(&path, &fill, FillRule::Winding, Transform::identity(), None);

    let north = painter.style().north;
    let stroke_color = rgba_with_alpha(north, (alpha * 220.0) as u8);
    let mut stroke_paint = Paint::default();
    stroke_paint.set_color(stroke_color);
    stroke_paint.anti_alias = true;
    let stroke = Stroke {
        width: 2.0,
        ..Default::default()
    };
    pm.stroke_path(
        &path,
        &stroke_paint,
        &stroke,
        Transform::identity(),
        None,
    );
}

fn draw_login_ui(
    pm: &mut PixmapMut,
    w: f32,
    h: f32,
    painter: &CompassPainter,
    ui: &LoginUiState,
    alpha: f32,
    caret_on: bool,
) {
    let (card_left, card_top, cw, ch) = card_rect(w, h);
    let pad = 32.0;
    let inner_left = card_left + pad;
    let inner_top = card_top + pad;
    let inner_w = cw - 2.0 * pad;
    let cx = card_left + cw / 2.0;

    let north = painter.style().north;
    let text_color = Color::from_rgba8(225, 230, 240, (alpha * 240.0) as u8);
    let label_color = Color::from_rgba8(180, 195, 220, (alpha * 200.0) as u8);
    let hint_color = Color::from_rgba8(180, 195, 220, (alpha * 150.0) as u8);
    let title_color = Color::from_rgba8(230, 236, 248, (alpha * 220.0) as u8);
    let caret_color = rgba_with_alpha(north, (alpha * 220.0) as u8);
    let box_fill = Color::from_rgba8(8, 12, 22, (alpha * 200.0) as u8);
    let box_outline = rgba_with_alpha(north, (alpha * 70.0) as u8);

    // Title
    painter.render_text_centered(
        pm,
        TextStyle::Script(44.0),
        "Willkommen",
        cx,
        inner_top + 32.0,
        title_color,
    );

    let box_h = 36.0;
    let label_size = 14.0;
    let text_size = 22.0;

    // Username row
    let user_label_y = inner_top + 84.0;
    painter.render_text_left(
        pm,
        TextStyle::SansBold(label_size),
        "User",
        inner_left,
        user_label_y,
        label_color,
    );
    let user_box_top = inner_top + 96.0;
    draw_input_box(pm, inner_left, user_box_top, inner_w, box_h, box_fill, box_outline);
    let user_text_x = inner_left + 12.0;
    let user_baseline = user_box_top + box_h - 12.0;
    let after_user = painter.render_text_left(
        pm,
        TextStyle::SansBold(text_size),
        &ui.username,
        user_text_x,
        user_baseline,
        text_color,
    );
    if ui.focus == Field::Username && caret_on {
        draw_caret(pm, after_user, user_baseline, text_size, caret_color);
    }

    // Password row
    let pwd_label_y = inner_top + 156.0;
    painter.render_text_left(
        pm,
        TextStyle::SansBold(label_size),
        "Passwort",
        inner_left,
        pwd_label_y,
        label_color,
    );
    let pwd_box_top = inner_top + 168.0;
    draw_input_box(pm, inner_left, pwd_box_top, inner_w, box_h, box_fill, box_outline);
    let pwd_text_x = inner_left + 12.0;
    let pwd_baseline = pwd_box_top + box_h - 12.0;
    let dots = "•".repeat(ui.password_len);
    let after_pwd = painter.render_text_left(
        pm,
        TextStyle::SansBold(text_size),
        &dots,
        pwd_text_x,
        pwd_baseline,
        text_color,
    );
    if ui.focus == Field::Password && caret_on {
        draw_caret(pm, after_pwd, pwd_baseline, text_size, caret_color);
    }

    // Bottom hint
    let hint_y = card_top + ch - pad + 4.0;
    painter.render_text_centered(
        pm,
        TextStyle::SansBold(13.0),
        "Enter zum Anmelden",
        cx,
        hint_y,
        hint_color,
    );
}

fn draw_input_box(
    pm: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fill: Color,
    outline: Color,
) {
    let path = rounded_rect_path(x, y, w, h, 6.0);
    let mut fill_paint = Paint::default();
    fill_paint.set_color(fill);
    fill_paint.anti_alias = true;
    pm.fill_path(&path, &fill_paint, FillRule::Winding, Transform::identity(), None);

    let mut stroke_paint = Paint::default();
    stroke_paint.set_color(outline);
    stroke_paint.anti_alias = true;
    let stroke = Stroke {
        width: 1.0,
        ..Default::default()
    };
    pm.stroke_path(
        &path,
        &stroke_paint,
        &stroke,
        Transform::identity(),
        None,
    );
}

fn draw_caret(pm: &mut PixmapMut, x: f32, baseline_y: f32, font_size: f32, color: Color) {
    let top = baseline_y - 0.75 * font_size;
    let bottom = baseline_y + 0.1 * font_size;
    let mut pb = PathBuilder::new();
    pb.move_to(x, top);
    pb.line_to(x, bottom);
    let path = pb.finish().unwrap();
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    let stroke = Stroke {
        width: 1.5,
        ..Default::default()
    };
    pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
}

fn card_rect(w: f32, h: f32) -> (f32, f32, f32, f32) {
    let cw = (w * 0.32).clamp(360.0, 720.0);
    let ch = (h * 0.22).clamp(220.0, 380.0);
    let left = w / 2.0 - cw / 2.0;
    let top = h / 2.0 - ch / 2.0;
    (left, top, cw, ch)
}

fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish().unwrap()
}

fn rgba_with_alpha(c: Color, a: u8) -> Color {
    Color::from_rgba8(
        (c.red() * 255.0) as u8,
        (c.green() * 255.0) as u8,
        (c.blue() * 255.0) as u8,
        a,
    )
}

fn bootsplash_handover() -> std::io::Result<()> {
    send_command(BOOTSPLASH_SOCKET, b"handover\n").map(|_| ())
}

fn bootsplash_exit() -> std::io::Result<()> {
    send_command(BOOTSPLASH_SOCKET, b"exit\n").map(|_| ())
}

fn send_command(path: &str, cmd: &[u8]) -> std::io::Result<String> {
    let mut s = UnixStream::connect(path)?;
    s.set_read_timeout(Some(Duration::from_millis(500)))?;
    s.set_write_timeout(Some(Duration::from_millis(500)))?;
    s.write_all(cmd)?;
    let mut buf = [0u8; 256];
    let n = s.read(&mut buf).unwrap_or(0);
    let resp = String::from_utf8_lossy(&buf[..n]).into_owned();
    if resp.starts_with("ok") {
        Ok(resp)
    } else {
        Err(std::io::Error::other(format!(
            "peer refused: {}",
            resp.trim()
        )))
    }
}

fn pick_mode(modes: &[drm::control::Mode]) -> Option<drm::control::Mode> {
    let mut filtered: Vec<_> = modes
        .iter()
        .copied()
        .filter(|m| {
            let (w, h) = m.size();
            w.max(h) <= 2560 && w >= 1280 && h >= 720
        })
        .collect();
    filtered.sort_by_key(|m| {
        let (w, h) = m.size();
        std::cmp::Reverse(w as u32 * h as u32)
    });
    filtered.first().copied().or_else(|| modes.first().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> CompassPainter<'static> {
        CompassPainter::new(Fonts::quompacc()).unwrap()
    }

    #[test]
    fn anim_frame_at_t0_matches_settle_state() {
        let painter = p();
        let af = compute_anim_frame(0.0, &painter, 1920.0, 1080.0);
        assert_eq!(af.watermark_alpha, 0);
        assert!(af.glow_visible);
        assert!((af.glow_scale - 1.0).abs() < 1e-3);
        assert_eq!(af.card_alpha, 0.0);
        assert_eq!(af.ui_alpha, 0.0);
    }

    #[test]
    fn anim_frame_at_ui_fade_end_is_full() {
        let painter = p();
        let t = UI_FADE_END_MS as f32 / 1000.0;
        let af = compute_anim_frame(t, &painter, 1920.0, 1080.0);
        assert!((af.card_alpha - 1.0).abs() < 1e-3);
        assert!((af.ui_alpha - 1.0).abs() < 1e-3);
        assert_eq!(af.watermark_alpha, WATERMARK_FINAL_ALPHA);
        assert!(!af.glow_visible);
    }

    #[test]
    fn ramp_u8_clamps_outside_window() {
        assert_eq!(ramp_u8(50, 100, 200, 10, 90), 10);
        assert_eq!(ramp_u8(150, 100, 200, 10, 90), 50);
        assert_eq!(ramp_u8(300, 100, 200, 10, 90), 90);
    }

    #[test]
    fn card_rect_clamped_dimensions() {
        let (_, _, cw, ch) = card_rect(1920.0, 1440.0);
        assert!(cw >= 360.0 && cw <= 720.0);
        assert!(ch >= 220.0 && ch <= 380.0);
    }

    #[test]
    fn rounded_rect_path_does_not_panic_on_small_inputs() {
        let _ = rounded_rect_path(0.0, 0.0, 4.0, 4.0, 10.0);
    }
}
