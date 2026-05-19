use tiny_skia::PixmapMut;

use crate::{
    paint::Rect,
    style::{Color, Theme},
};

use super::{paint_fill, rounded_rect_path};

/// Paint a Metro-like surface: body fill plus a top accent stripe.
///
/// This helper rebuilds paths during paint and therefore inherits the current
/// rounded-rect allocation trade-off used by the Tile/Button widgets.
pub fn paint_metro_surface(
    canvas: &mut PixmapMut<'_>,
    area: Rect,
    body_color: Color,
    accent: Color,
    theme: &Theme,
    stripe_height: i32,
) {
    if let Some(body_path) = rounded_rect_path(area, theme.radius.lg) {
        paint_fill(canvas, &body_path, body_color);
    }

    let stripe_height = stripe_height.max(0).min(area.height);
    if stripe_height <= 0 {
        return;
    }

    let stripe_rect = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: stripe_height,
    };
    if let Some(stripe_path) = rounded_rect_path(stripe_rect, 0) {
        paint_fill(canvas, &stripe_path, accent);
    }
}

#[cfg(test)]
mod tests {
    use tiny_skia::Pixmap;

    use crate::{
        effect::paint_metro_surface,
        paint::Rect,
        style::{Palette, Theme},
    };

    #[test]
    fn paint_metro_surface_draws_body_and_stripe() {
        let mut pixmap = Pixmap::new(96, 96).expect("pixmap");
        let mut canvas = pixmap.as_mut();

        let theme = Theme::TOKYO_NIGHT_METRO;
        paint_metro_surface(
            &mut canvas,
            Rect {
                x: 0,
                y: 0,
                width: 96,
                height: 96,
            },
            theme.palette.surface,
            Palette::TOKYO_NIGHT_METRO.accent_alt,
            &theme,
            4,
        );
        drop(canvas);

        let stripe_px = pixmap.pixel(48, 1).expect("stripe pixel");
        let body_px = pixmap.pixel(48, 48).expect("body pixel");
        assert!(stripe_px.alpha() > 0);
        assert!(body_px.alpha() > 0);
        assert!(stripe_px.red() > body_px.red());
    }
}
