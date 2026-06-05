//! Centered modal asking the user whether to allow a screenshot request that
//! came in via the portal bridge. The compositor holds the request and emits
//! `ShellEvent::ScreenshotConsentRequest`; the shell shows this modal and sends
//! back `ShellCommand::ScreenshotConsentResponse{allowed}` on the user's click
//! (or Esc = deny / Enter = allow). The surface is exactly the modal size and
//! centered by its layer anchor, so everything is drawn at the origin.

use meridian_config::{ThemeConfig, ThemeSurface};
use meridian_tokens::Interaction;
use meridian_ui::{
    effect::{paint_border, paint_fill, paint_text, rounded_rect_path},
    paint::Rect,
};
use tiny_skia::Pixmap;

use crate::ui::tokens::{color_with_alpha, palette_from_config};

pub(crate) const MODAL_WIDTH: i32 = 460;
pub(crate) const MODAL_HEIGHT: i32 = 200;

const PAD_X: i32 = 28;
const BTN_W: i32 = 180;
const BTN_H: i32 = 44;
const BTN_GAP: i32 = 16;
const BTN_BOTTOM_MARGIN: i32 = 24;

/// Which consent button the pointer is over / was clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConsentButton {
    Allow,
    Deny,
}

/// The two button rectangles, laid out side by side near the bottom. Deny on
/// the left (safe default position), Allow on the right.
fn button_rects() -> (Rect, ConsentButton, Rect, ConsentButton) {
    let total = BTN_W * 2 + BTN_GAP;
    let left_x = (MODAL_WIDTH - total) / 2;
    let y = MODAL_HEIGHT - BTN_BOTTOM_MARGIN - BTN_H;
    let deny = Rect {
        x: left_x,
        y,
        width: BTN_W,
        height: BTN_H,
    };
    let allow = Rect {
        x: left_x + BTN_W + BTN_GAP,
        y,
        width: BTN_W,
        height: BTN_H,
    };
    (deny, ConsentButton::Deny, allow, ConsentButton::Allow)
}

/// Hit-test a pointer position (modal-local coordinates) against the buttons.
pub(crate) fn hit_button(px: f64, py: f64) -> Option<ConsentButton> {
    let (deny, _, allow, _) = button_rects();
    let inside = |r: &Rect| {
        px >= r.x as f64
            && px < (r.x + r.width) as f64
            && py >= r.y as f64
            && py < (r.y + r.height) as f64
    };
    if inside(&allow) {
        Some(ConsentButton::Allow)
    } else if inside(&deny) {
        Some(ConsentButton::Deny)
    } else {
        None
    }
}

/// Render the modal onto a BGRA `canvas` of MODAL_WIDTH x MODAL_HEIGHT.
/// `app_id` is the best-effort requesting app identity ("" when unknown).
pub(crate) fn draw_consent_overlay(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    app_id: &str,
    hovered: Option<ConsentButton>,
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

    // Title + body text.
    paint_text(
        &mut pm.as_mut(),
        "Bildschirmfoto erlauben?",
        PAD_X,
        46,
        17.0,
        pal.text,
    );
    let who = if app_id.trim().is_empty() {
        "Eine Anwendung".to_string()
    } else {
        app_id.trim().to_string()
    };
    let body = fit(&format!("{} möchte den Bildschirm aufnehmen.", who), 52);
    paint_text(&mut pm.as_mut(), &body, PAD_X, 80, 13.5, pal.text_dim);

    // Buttons.
    let (deny, _, allow, _) = button_rects();
    // Deny: neutral surface; Allow: accent fill.
    let deny_bg = if hovered == Some(ConsentButton::Deny) {
        Interaction::DEFAULT.hover(pal.surface)
    } else {
        pal.surface
    };
    if let Some(p) = rounded_rect_path(deny, control_radius) {
        paint_fill(&mut pm.as_mut(), &p, deny_bg);
        paint_border(&mut pm.as_mut(), &p, card_border, 1.0);
    }
    let allow_bg = if hovered == Some(ConsentButton::Allow) {
        Interaction::DEFAULT.hover(pal.accent)
    } else {
        pal.accent
    };
    if let Some(p) = rounded_rect_path(allow, control_radius) {
        paint_fill(&mut pm.as_mut(), &p, allow_bg);
    }
    // Button labels, roughly centered.
    paint_text(
        &mut pm.as_mut(),
        "Ablehnen",
        deny.x + 52,
        deny.y + 28,
        13.5,
        pal.text,
    );
    paint_text(
        &mut pm.as_mut(),
        "Erlauben",
        allow.x + 54,
        allow.y + 28,
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
    out.push('…');
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
    fn hit_button_distinguishes_allow_and_deny() {
        let (deny, _, allow, _) = button_rects();
        // Center of each button hits the right one.
        let dc = (
            (deny.x + deny.width / 2) as f64,
            (deny.y + deny.height / 2) as f64,
        );
        let ac = (
            (allow.x + allow.width / 2) as f64,
            (allow.y + allow.height / 2) as f64,
        );
        assert_eq!(hit_button(dc.0, dc.1), Some(ConsentButton::Deny));
        assert_eq!(hit_button(ac.0, ac.1), Some(ConsentButton::Allow));
    }

    #[test]
    fn hit_button_misses_outside_buttons() {
        // Top of the modal (title area) is not a button.
        assert_eq!(hit_button((MODAL_WIDTH / 2) as f64, 10.0), None);
        // Gap between the two buttons.
        let (deny, _, _allow, _) = button_rects();
        let gap_x = (deny.x + deny.width + BTN_GAP / 2) as f64;
        let gap_y = (deny.y + deny.height / 2) as f64;
        assert_eq!(hit_button(gap_x, gap_y), None);
    }

    #[test]
    fn buttons_lie_within_the_modal() {
        let (deny, _, allow, _) = button_rects();
        assert!(deny.x >= 0 && allow.x + allow.width <= MODAL_WIDTH);
        assert!(allow.y + allow.height <= MODAL_HEIGHT);
        // Deny is left of Allow.
        assert!(deny.x < allow.x);
    }
}
