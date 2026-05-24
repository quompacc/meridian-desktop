use std::collections::HashMap;

use meridian_config::ThemeConfig;

use crate::{Painter, Rect};

/// Compute the popup width for the given window IDs, using cached thumb widths
/// where available and falling back to the max thumb width for not-yet-loaded
/// ids. This is the source of truth for both layer-surface sizing and drawing.
pub(crate) fn popup_width_for(
    cache: &HashMap<String, (u32, u32, Vec<u8>)>,
    window_ids: &[String],
) -> u32 {
    let pad = crate::THUMBNAIL_CARD_PAD;
    let gap = crate::THUMBNAIL_CARD_GAP;
    let max_thumb_w = crate::THUMBNAIL_THUMB_W;
    let n = window_ids.len().min(crate::THUMBNAIL_MAX_WINDOWS);
    if n == 0 {
        return 2 * pad;
    }
    let mut total = 2 * pad + (n as u32 - 1) * gap;
    for id in window_ids.iter().take(n) {
        let w = cache.get(id.as_str()).map(|(w, _, _)| *w).unwrap_or(max_thumb_w);
        total += w;
    }
    total
}

pub(crate) fn draw_thumbnail_popup(
    painter: &mut Painter<'_>,
    theme: &ThemeConfig,
    cache: &HashMap<String, (u32, u32, Vec<u8>)>,
    window_ids: &[String],
    width: u32,
    height: u32,
) {
    let colors = &theme.colors;

    // Background + thin border around the popup
    painter.clear(colors.surface_alt);
    painter.stroke_rect(
        Rect { x: 0, y: 0, w: width as i32, h: height as i32 },
        colors.border,
    );

    let pad = crate::THUMBNAIL_CARD_PAD as i32;
    let gap = crate::THUMBNAIL_CARD_GAP as i32;
    let max_thumb_w = crate::THUMBNAIL_THUMB_W as i32;
    let max_thumb_h = crate::THUMBNAIL_THUMB_H as i32;
    let loading_bg = meridian_config::Color::rgb(40, 40, 55);

    let mut cursor_x = pad;
    for id in window_ids.iter().take(crate::THUMBNAIL_MAX_WINDOWS) {
        let slot_y = pad;
        match cache.get(id.as_str()) {
            Some((tw, th, data)) if *tw > 0 && *th > 0 && data.len() == (*tw * *th * 4) as usize => {
                let tw_i = *tw as i32;
                let th_i = *th as i32;
                // Center vertically within the max thumb_h band (in case thumb is shorter)
                let blit_x = cursor_x;
                let blit_y = slot_y + (max_thumb_h - th_i) / 2;
                blit_xrgb(painter, data, *tw, *th, blit_x, blit_y);
                cursor_x += tw_i + gap;
            }
            _ => {
                // Loading placeholder at max thumb width
                painter.rect(
                    Rect { x: cursor_x, y: slot_y, w: max_thumb_w, h: max_thumb_h },
                    loading_bg,
                );
                cursor_x += max_thumb_w + gap;
            }
        }
    }
}

fn blit_xrgb(painter: &mut Painter<'_>, data: &[u8], tw: u32, th: u32, blit_x: i32, blit_y: i32) {
    let canvas_w = painter.width;
    let canvas_h = painter.height;
    let canvas = &mut painter.data;
    for row in 0..th as i32 {
        let dst_y = blit_y + row;
        if dst_y < 0 || dst_y >= canvas_h {
            continue;
        }
        for col in 0..tw as i32 {
            let dst_x = blit_x + col;
            if dst_x < 0 || dst_x >= canvas_w {
                continue;
            }
            let si = (row as usize * tw as usize + col as usize) * 4;
            let di = (dst_y as usize * canvas_w as usize + dst_x as usize) * 4;
            if si + 3 < data.len() && di + 3 < canvas.len() {
                canvas[di]     = data[si];     // B
                canvas[di + 1] = data[si + 1]; // G
                canvas[di + 2] = data[si + 2]; // R
                canvas[di + 3] = 0xFF;         // A
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{draw_thumbnail_popup, popup_width_for};
    use std::collections::HashMap;

    #[test]
    fn draw_thumbnail_popup_does_not_panic_with_empty_cache() {
        let mut data = vec![0u8; 220 * 150 * 4];
        let mut painter = crate::Painter::new(&mut data, 220, 150);
        let theme = meridian_config::ThemeConfig::default();
        let cache: HashMap<String, (u32, u32, Vec<u8>)> = HashMap::new();
        let ids = vec!["win-1".to_string()];
        draw_thumbnail_popup(&mut painter, &theme, &cache, &ids, 220, 150);
    }

    #[test]
    fn popup_width_falls_back_to_max_for_uncached_ids() {
        let cache: HashMap<String, (u32, u32, Vec<u8>)> = HashMap::new();
        let ids = vec!["win-1".to_string(), "win-2".to_string()];
        // pad=12, gap=8, 2 thumbs * 200 max = 400 + 1 gap + 2*12 pad = 432
        assert_eq!(popup_width_for(&cache, &ids), 12 + 200 + 8 + 200 + 12);
    }

    #[test]
    fn popup_width_uses_cached_thumb_widths() {
        let mut cache: HashMap<String, (u32, u32, Vec<u8>)> = HashMap::new();
        cache.insert("win-1".to_string(), (178, 112, vec![0u8; 178 * 112 * 4]));
        cache.insert("win-2".to_string(), (149, 112, vec![0u8; 149 * 112 * 4]));
        let ids = vec!["win-1".to_string(), "win-2".to_string()];
        assert_eq!(popup_width_for(&cache, &ids), 12 + 178 + 8 + 149 + 12);
    }

    #[test]
    fn draw_thumbnail_popup_blits_thumbnail_pixels() {
        let w = 220u32;
        let h = 150u32;
        let mut data = vec![0u8; (w * h * 4) as usize];
        let mut painter = crate::Painter::new(&mut data, w as i32, h as i32);
        let theme = meridian_config::ThemeConfig::default();

        // 4x4 XRGB8888 thumb, all red
        let mut thumb = vec![0u8; 4 * 4 * 4];
        for px in thumb.chunks_exact_mut(4) {
            px[0] = 0;
            px[1] = 0;
            px[2] = 255;
            px[3] = 0;
        }
        let mut cache = HashMap::new();
        cache.insert("win-1".to_string(), (4u32, 4u32, thumb));

        let ids = vec!["win-1".to_string()];
        draw_thumbnail_popup(&mut painter, &theme, &cache, &ids, w, h);

        // First thumb: cursor_x = pad = 12, blit_x = 12
        // blit_y = pad + (112 - 4) / 2 = 12 + 54 = 66
        let blit_x = 12usize;
        let blit_y = 66usize;
        let off = (blit_y * w as usize + blit_x) * 4;
        assert_eq!(data[off],     0);
        assert_eq!(data[off + 1], 0);
        assert_eq!(data[off + 2], 255);
        assert_eq!(data[off + 3], 255);
    }
}
