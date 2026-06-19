// Phase A1.3: paint a single notification into a shm canvas.
//
// Visual: same metro-card pattern as `network_popup` — flat surface_alt
// body + 1px outer border, accent strip under the title, body text in
// muted. Single-notification rendering for v1; stacking + multi-line
// body wrapping + icons land later.

use std::cell::RefCell;

use meridian_config::ThemeConfig;

use crate::{
    notifications::Notification, Painter, Rect, TextRenderer, NOTIFICATION_HEIGHT,
    NOTIFICATION_WIDTH,
};

pub fn draw_notification(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    theme: &ThemeConfig,
    notif: &Notification,
) {
    let colors = &theme.colors;
    let treatment = theme
        .decorations
        .surface_treatment(meridian_config::ThemeSurface::Popup);
    let width = NOTIFICATION_WIDTH as i32;
    let height = NOTIFICATION_HEIGHT as i32;

    // Central glass solidity (glass_alpha) like every other surface — not a
    // local opaque fill. The compositor glass behind shows through the gap.
    painter.clear(meridian_config::Color::rgba(
        colors.surface_alt.r,
        colors.surface_alt.g,
        colors.surface_alt.b,
        treatment.fill_alpha,
    ));
    painter.stroke_rect(
        Rect {
            x: 0,
            y: 0,
            w: width,
            h: height,
        },
        meridian_config::Color::rgba(
            colors.border.r,
            colors.border.g,
            colors.border.b,
            treatment.frame_alpha,
        ),
    );

    const PAD_X: i32 = 14;
    const TITLE_TOP: i32 = 12;
    const TITLE_HEIGHT: i32 = 22;
    const SEP_HEIGHT: i32 = 2;
    const BODY_TOP_PAD: i32 = 10;
    const APP_LABEL_TOP: i32 = 4;

    // App label at the very top (small, muted) — distinguishes which
    // app sent the notification when several are flashing past.
    if !notif.app.is_empty() {
        painter.text_clipped(
            font,
            &notif.app,
            PAD_X,
            APP_LABEL_TOP + 8,
            width - 2 * PAD_X,
            colors.text_dim,
        );
    }

    // Title — the `summary` field from the spec.
    painter.text_clipped(
        font,
        &notif.title,
        PAD_X,
        TITLE_TOP + 14,
        width - 2 * PAD_X,
        colors.accent,
    );

    // 2px accent strip under the title (metro signature).
    let sep_y = TITLE_TOP + TITLE_HEIGHT;
    painter.rect(
        Rect {
            x: 0,
            y: sep_y,
            w: width,
            h: SEP_HEIGHT,
        },
        colors.accent,
    );

    // Body — first line only for v1; multi-line wrap is A1.3+ polish.
    if !notif.body.is_empty() {
        painter.text_clipped(
            font,
            &notif.body,
            PAD_X,
            sep_y + SEP_HEIGHT + BODY_TOP_PAD + 10,
            width - 2 * PAD_X,
            colors.text,
        );
    }
}
