//! Software rounded-box drop shadow for client-drawn surfaces (panel, launcher, popups).
//!
//! Popups draw their card into a temp buffer of the card's size, then call
//! [`composite_card_onto_surface`] to alpha-blend it onto the surrounding
//! shadow at the offset (PAD, PAD).
//!
//! Matches the look of the compositor's SDF shadow shader but runs on the CPU
//! into a packed 8-bit buffer. The shadow is premultiplied black, so the RGB
//! byte order (RGBA vs ARGB) does not matter — only the alpha byte (index 3)
//! plus a uniform darkening of the existing RGB. Drawn *outside* the casting
//! rect only, so it never bleeds under a translucent surface.

/// Premultiplied SRC_OVER composite of an RGBA card buffer onto a larger
/// surface buffer at `(dst_x, dst_y)`. Both buffers are BGRA-ordered, alpha
/// at byte 3. The source card has already had `round_buffer_corners` applied,
/// which scales corner-pixel RGB by coverage — that's the premultiplied form.
/// Doing straight-alpha here would multiply by alpha again and darken the
/// AA ring at every rounded corner.
#[allow(clippy::too_many_arguments)]
pub(crate) fn composite_card_onto_surface(
    dst: &mut [u8],
    dst_w: usize,
    dst_h: usize,
    src: &[u8],
    src_w: usize,
    src_h: usize,
    dst_x: usize,
    dst_y: usize,
) {
    for y in 0..src_h {
        let dy = dst_y + y;
        if dy >= dst_h {
            break;
        }
        for x in 0..src_w {
            let dx = dst_x + x;
            if dx >= dst_w {
                continue;
            }
            let si = (y * src_w + x) * 4;
            let di = (dy * dst_w + dx) * 4;
            if si + 4 > src.len() || di + 4 > dst.len() {
                continue;
            }
            let sa = src[si + 3] as u32;
            if sa == 0 {
                continue;
            }
            if sa == 255 {
                dst[di..di + 4].copy_from_slice(&src[si..si + 4]);
                continue;
            }
            let inv = 255 - sa;
            // Premultiplied: src.rgb already carries the cov factor.
            dst[di] = (src[si] as u32 + dst[di] as u32 * inv / 255) as u8;
            dst[di + 1] = (src[si + 1] as u32 + dst[di + 1] as u32 * inv / 255) as u8;
            dst[di + 2] = (src[si + 2] as u32 + dst[di + 2] as u32 * inv / 255) as u8;
            dst[di + 3] = (sa + dst[di + 3] as u32 * inv / 255) as u8;
        }
    }
}

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
    // Un-offset element centre. The translucent-safe clip below must test
    // the ACTUAL element, not the drop-shifted shape, or offset_y leaves a
    // transparent strip directly below it (the recurring "second bar").
    let cy_win = y as f32 + h as f32 / 2.0;
    let hx = w as f32 / 2.0;
    let hy = h as f32 / 2.0;
    let pad = blur.ceil() as i32 + 1;
    let x0 = (x - pad).max(0);
    let y0 = (sy - pad).max(0);
    let x1 = (x + w + pad).min(buf_w);
    let y1 = (sy + h + pad).min(buf_h);
    for py in y0..y1 {
        for px in x0..x1 {
            if clip_inside {
                // Discard inside the actual (un-offset) element; coverage
                // still comes from the drop-shifted SDF below, so the shadow
                // stays continuous directly under the element (no offset gap).
                let d_win = rounded_box_sdf(px as f32, py as f32, cx, cy_win, hx, hy, radius);
                if d_win < 0.0 {
                    continue; // translucent surface owns its interior
                }
            }
            let d = rounded_box_sdf(px as f32, py as f32, cx, cy, hx, hy, radius);
            // 1 - smoothstep(-blur, blur, d): soft, centred on the edge.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translucent_clip_keeps_the_actual_edge_pixel() {
        let mut buf = vec![0u8; 64 * 64 * 4];
        draw_soft_shadow(&mut buf, 64, 64, 16, 16, 24, 16, 6.0, 13.0, 0.12, 3, true);

        let center_x = 16 + 12;
        let bottom_edge_y = 16 + 16;
        let edge_idx = ((bottom_edge_y * 64 + center_x) * 4) as usize;
        assert!(
            buf[edge_idx + 3] > 0,
            "the exact un-offset bottom edge must receive shadow coverage"
        );

        let inner_y = bottom_edge_y - 1;
        let inner_idx = ((inner_y * 64 + center_x) * 4) as usize;
        assert_eq!(
            buf[inner_idx + 3],
            0,
            "pixels inside the translucent card remain untouched"
        );
    }
}
