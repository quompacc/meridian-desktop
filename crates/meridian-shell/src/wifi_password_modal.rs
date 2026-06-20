//! Centered modal for entering a Wi-Fi password. Mirrors the screenshot
//! consent modal (`screenshot_consent.rs`): a layer surface exactly the modal
//! size, centered by its anchor, so everything is drawn at the origin. The
//! shell opens it when the user picks a secured network (from the network tray
//! popup or the Settings network page); the typed password lives in
//! `wifi_password_input` and is shown masked. Enter / "Verbinden" connects,
//! Esc / "Abbrechen" cancels.

use meridian_config::{ThemeConfig, ThemeSurface};
use meridian_tokens::Interaction;
use meridian_ui::{
    effect::{paint_border, paint_fill, paint_text, rounded_rect_path},
    paint::Rect,
};
use tiny_skia::Pixmap;

use crate::ui::tokens::{color_with_alpha, palette_from_config};

pub(crate) const MODAL_WIDTH: i32 = 460;
pub(crate) const MODAL_HEIGHT: i32 = 230;

const PAD_X: i32 = 28;
const FIELD_H: i32 = 40;
const FIELD_TOP: i32 = 96;
const BTN_W: i32 = 180;
const BTN_H: i32 = 44;
const BTN_GAP: i32 = 16;
const BTN_BOTTOM_MARGIN: i32 = 24;
/// Cap on rendered mask dots so a very long password never overflows the field.
const MAX_MASK_DOTS: usize = 28;

/// Which modal button the pointer is over / clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalButton {
    Connect,
    Cancel,
}

/// The two button rectangles, laid out side by side near the bottom. Cancel on
/// the left, Connect (accent) on the right — same arrangement as the consent
/// modal so the muscle memory carries over.
fn button_rects() -> (Rect, Rect) {
    let total = BTN_W * 2 + BTN_GAP;
    let left_x = (MODAL_WIDTH - total) / 2;
    let y = MODAL_HEIGHT - BTN_BOTTOM_MARGIN - BTN_H;
    let cancel = Rect {
        x: left_x,
        y,
        width: BTN_W,
        height: BTN_H,
    };
    let connect = Rect {
        x: left_x + BTN_W + BTN_GAP,
        y,
        width: BTN_W,
        height: BTN_H,
    };
    (cancel, connect)
}

/// The password field rectangle (modal-local coordinates).
fn field_rect() -> Rect {
    Rect {
        x: PAD_X,
        y: FIELD_TOP,
        width: MODAL_WIDTH - 2 * PAD_X,
        height: FIELD_H,
    }
}

/// Hit-test a pointer position (modal-local coordinates) against the buttons.
pub(crate) fn hit_button(px: f64, py: f64) -> Option<ModalButton> {
    let (cancel, connect) = button_rects();
    let inside = |r: &Rect| {
        px >= r.x as f64
            && px < (r.x + r.width) as f64
            && py >= r.y as f64
            && py < (r.y + r.height) as f64
    };
    if inside(&connect) {
        Some(ModalButton::Connect)
    } else if inside(&cancel) {
        Some(ModalButton::Cancel)
    } else {
        None
    }
}

/// Render the modal onto a BGRA `canvas` of MODAL_WIDTH x MODAL_HEIGHT.
/// `ssid` is the network being joined; `password_len` drives the mask dots.
pub(crate) fn draw_wifi_password_modal(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    ssid: &str,
    password_len: usize,
    hovered: Option<ModalButton>,
    theme_config: &ThemeConfig,
) {
    let w = MODAL_WIDTH as u32;
    let h = MODAL_HEIGHT as u32;
    let Some(mut pm) = Pixmap::new(w, h) else {
        return;
    };
    let pal = palette_from_config(theme_config);
    let treatment = theme_config
        .decorations
        .surface_treatment(ThemeSurface::Modal);
    let modal_radius = treatment.radius.round() as i32;
    let card_fill = color_with_alpha(pal.surface_alt, treatment.fill_alpha);
    let card_border = color_with_alpha(pal.border, treatment.frame_alpha);
    let control_radius = theme_config
        .decorations
        .surface_radius(ThemeSurface::Control)
        .round() as i32;

    // Card background + border.
    let bg = Rect {
        x: 0,
        y: 0,
        width: MODAL_WIDTH,
        height: MODAL_HEIGHT,
    };
    if let Some(p) = rounded_rect_path(bg, modal_radius) {
        paint_fill(&mut pm.as_mut(), &p, card_fill);
        paint_border(&mut pm.as_mut(), &p, card_border, 1.0);
    }

    // Title + the SSID being joined.
    paint_text(
        &mut pm.as_mut(),
        "Mit WLAN verbinden",
        PAD_X,
        46,
        17.0,
        pal.text,
    );
    let who = fit(ssid.trim(), 40);
    paint_text(&mut pm.as_mut(), &who, PAD_X, 76, 13.5, pal.text_dim);

    // Password field: neutral surface box with masked dots.
    let field = field_rect();
    if let Some(p) = rounded_rect_path(field, control_radius) {
        paint_fill(&mut pm.as_mut(), &p, pal.surface);
        paint_border(&mut pm.as_mut(), &p, card_border, 1.0);
    }
    if password_len == 0 {
        paint_text(
            &mut pm.as_mut(),
            "Passwort",
            field.x + 12,
            field.y + 26,
            13.5,
            pal.text_dim,
        );
    } else {
        let dots: String = "\u{25cf}".repeat(password_len.min(MAX_MASK_DOTS));
        paint_text(
            &mut pm.as_mut(),
            &dots,
            field.x + 12,
            field.y + 26,
            13.5,
            pal.text,
        );
    }

    // Buttons.
    let (cancel, connect) = button_rects();
    let cancel_bg = if hovered == Some(ModalButton::Cancel) {
        Interaction::DEFAULT.hover(pal.surface)
    } else {
        pal.surface
    };
    if let Some(p) = rounded_rect_path(cancel, control_radius) {
        paint_fill(&mut pm.as_mut(), &p, cancel_bg);
        paint_border(&mut pm.as_mut(), &p, card_border, 1.0);
    }
    let connect_bg = if hovered == Some(ModalButton::Connect) {
        Interaction::DEFAULT.hover(pal.accent)
    } else {
        pal.accent
    };
    if let Some(p) = rounded_rect_path(connect, control_radius) {
        paint_fill(&mut pm.as_mut(), &p, connect_bg);
    }
    paint_text(
        &mut pm.as_mut(),
        "Abbrechen",
        cancel.x + 50,
        cancel.y + 28,
        13.5,
        pal.text,
    );
    paint_text(
        &mut pm.as_mut(),
        "Verbinden",
        connect.x + 52,
        connect.y + 28,
        13.5,
        pal.surface,
    );

    blit(canvas, canvas_w as i32, canvas_h as i32, &pm);
}

fn fit(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

/// Blit a premultiplied Pixmap onto the straight BGRA canvas at the origin.
fn blit(canvas: &mut [u8], cw: i32, ch: i32, pm: &Pixmap) {
    let pw = pm.width() as i32;
    let ph = pm.height() as i32;
    let src = pm.pixels();
    for my in 0..ph {
        if my >= ch {
            break;
        }
        for mx in 0..pw {
            if mx >= cw {
                break;
            }
            let pi = (my * pw + mx) as usize;
            if pi >= src.len() {
                continue;
            }
            let px = src[pi];
            let a = px.alpha();
            if a == 0 {
                continue;
            }
            let ci = ((my * cw + mx) * 4) as usize;
            if ci + 3 >= canvas.len() {
                continue;
            }
            let rp = px.red() as u32;
            let gp = px.green() as u32;
            let bp = px.blue() as u32;
            let inv = 255 - a as u32;
            let db = canvas[ci] as u32;
            let dg = canvas[ci + 1] as u32;
            let dr = canvas[ci + 2] as u32;
            canvas[ci] = (bp + db * inv / 255).min(255) as u8;
            canvas[ci + 1] = (gp + dg * inv / 255).min(255) as u8;
            canvas[ci + 2] = (rp + dr * inv / 255).min(255) as u8;
            canvas[ci + 3] = 255;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_button_distinguishes_connect_and_cancel() {
        let (cancel, connect) = button_rects();
        let cc = (
            (cancel.x + cancel.width / 2) as f64,
            (cancel.y + cancel.height / 2) as f64,
        );
        let nc = (
            (connect.x + connect.width / 2) as f64,
            (connect.y + connect.height / 2) as f64,
        );
        assert_eq!(hit_button(cc.0, cc.1), Some(ModalButton::Cancel));
        assert_eq!(hit_button(nc.0, nc.1), Some(ModalButton::Connect));
    }

    #[test]
    fn hit_button_misses_outside_buttons() {
        // Title area is not a button.
        assert_eq!(hit_button((MODAL_WIDTH / 2) as f64, 10.0), None);
        // Gap between the two buttons.
        let (cancel, _connect) = button_rects();
        let gap_x = (cancel.x + cancel.width + BTN_GAP / 2) as f64;
        let gap_y = (cancel.y + cancel.height / 2) as f64;
        assert_eq!(hit_button(gap_x, gap_y), None);
    }

    #[test]
    fn buttons_and_field_lie_within_the_modal() {
        let (cancel, connect) = button_rects();
        assert!(cancel.x >= 0 && connect.x + connect.width <= MODAL_WIDTH);
        assert!(connect.y + connect.height <= MODAL_HEIGHT);
        assert!(cancel.x < connect.x); // Cancel left of Connect.
        let field = field_rect();
        assert!(field.x >= 0 && field.x + field.width <= MODAL_WIDTH);
        assert!(field.y + field.height < cancel.y); // field above the buttons
    }

    #[test]
    fn draw_does_not_panic_for_empty_and_long_input() {
        let w = MODAL_WIDTH as u32;
        let h = MODAL_HEIGHT as u32;
        let mut canvas = vec![0u8; (w * h * 4) as usize];
        let theme = ThemeConfig::default();
        draw_wifi_password_modal(&mut canvas, w, h, "MyHomeNet", 0, None, &theme);
        draw_wifi_password_modal(
            &mut canvas,
            w,
            h,
            "A very very long network name that should be truncated cleanly",
            200,
            Some(ModalButton::Connect),
            &theme,
        );
    }
}
