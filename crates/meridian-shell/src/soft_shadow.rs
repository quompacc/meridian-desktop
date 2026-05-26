//! Software rounded-box drop shadow for client-drawn surfaces (panel, launcher).
//!
//! Matches the look of the compositor's SDF shadow shader but runs on the CPU
//! into a packed 8-bit buffer. The shadow is premultiplied black, so the RGB
//! byte order (RGBA vs ARGB) does not matter — only the alpha byte (index 3)
//! plus a uniform darkening of the existing RGB. Drawn *outside* the casting
//! rect only, so it never bleeds under a translucent surface.

fn rounded_box_sdf(px: f32, py: f32, cx: f32, cy: f32, hx: f32, hy: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - hx + r;
    let qy = (py - cy).abs() - hy + r;
    let outside = (qx.max(0.0)).hypot(qy.max(0.0));
    qx.max(qy).min(0.0) + outside - r
}

/// Blend a soft drop shadow of a rounded rect into `buf` (4 bytes/pixel,
/// premultiplied, alpha at byte 3). The shadow is only painted where pixels
/// fall *outside* the rounded rect (signed distance > 0), fading over `blur`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_soft_shadow(
    buf: &mut [u8],
    buf_w: i32,
    buf_h: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    radius: f32,
    blur: f32,
    alpha: f32,
    offset_y: i32,
    // When true (translucent surfaces like the panel island) the shadow is
    // only painted outside the rect, so it never bleeds dark through the glass.
    // When false (opaque surfaces like the launcher) the shadow is drawn across
    // the whole footprint and the opaque content covers the inner part — this
    // avoids the transparent gap an offset would otherwise leave below the rect.
    clip_inside: bool,
) {
    if w <= 0 || h <= 0 || blur <= 0.0 || alpha <= 0.0 {
        return;
    }
    let sy = y + offset_y;
    let cx = x as f32 + w as f32 / 2.0;
    let cy = sy as f32 + h as f32 / 2.0;
    let hx = w as f32 / 2.0;
    let hy = h as f32 / 2.0;
    let pad = blur.ceil() as i32 + 1;
    let x0 = (x - pad).max(0);
    let y0 = (sy - pad).max(0);
    let x1 = (x + w + pad).min(buf_w);
    let y1 = (sy + h + pad).min(buf_h);
    for py in y0..y1 {
        for px in x0..x1 {
            let d = rounded_box_sdf(px as f32, py as f32, cx, cy, hx, hy, radius);
            if clip_inside && d <= 0.0 {
                continue; // translucent surface owns its interior
            }
            // 1 - smoothstep(-blur, blur, d): soft, centred on the edge (0.5 at
            // the boundary) so there is no hard outline.
            let t = ((d + blur) / (2.0 * blur)).clamp(0.0, 1.0);
            let cov = 1.0 - t * t * (3.0 - 2.0 * t);
            let sa = cov * alpha;
            if sa <= 0.004 {
                continue;
            }
            let idx = ((py * buf_w + px) * 4) as usize;
            if idx + 4 > buf.len() {
                continue;
            }
            let inv = 1.0 - sa;
            buf[idx] = (buf[idx] as f32 * inv) as u8;
            buf[idx + 1] = (buf[idx + 1] as f32 * inv) as u8;
            buf[idx + 2] = (buf[idx + 2] as f32 * inv) as u8;
            buf[idx + 3] = (sa * 255.0 + buf[idx + 3] as f32 * inv) as u8;
        }
    }
}
