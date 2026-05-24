use std::cell::RefCell;

use meridian_config::Color;

use crate::{icons::IconImage, Rect};

use super::{bitmap, text::TextRenderer};

const DEFAULT_ROUNDISH_RADIUS: i32 = 6;
const CORNER_AA_SAMPLE_OFFSETS: [f32; 2] = [0.25, 0.75];

pub struct Painter<'a> {
    pub(crate) data: &'a mut [u8],
    pub(crate) width: i32,
    pub(crate) height: i32,
}

impl<'a> Painter<'a> {
    pub fn new(data: &'a mut [u8], width: i32, height: i32) -> Self {
        Self {
            data,
            width,
            height,
        }
    }

    pub fn clear(&mut self, color: Color) {
        let pixel = argb(color).to_le_bytes();
        for chunk in self.data.chunks_exact_mut(4) {
            chunk.copy_from_slice(&pixel);
        }
    }

    pub fn roundish_rect(&mut self, rect: Rect, color: Color) {
        self.fill_rounded_rect(rect, color, DEFAULT_ROUNDISH_RADIUS);
    }

    pub fn roundish_rect_with_radius(&mut self, rect: Rect, color: Color, radius: i32) {
        self.fill_rounded_rect(rect, color, radius);
    }

    pub fn rect(&mut self, rect: Rect, color: Color) {
        let x0 = rect.x.clamp(0, self.width);
        let y0 = rect.y.clamp(0, self.height);
        let x1 = (rect.x + rect.w).clamp(0, self.width);
        let y1 = (rect.y + rect.h).clamp(0, self.height);
        let pixel = argb(color).to_le_bytes();

        for y in y0..y1 {
            let row = (y * self.width * 4) as usize;
            for x in x0..x1 {
                let offset = row + (x * 4) as usize;
                self.data[offset..offset + 4].copy_from_slice(&pixel);
            }
        }
    }

    fn fill_rounded_rect(&mut self, rect: Rect, color: Color, desired_radius: i32) {
        let radius = clamped_radius(rect.w, rect.h, desired_radius);
        if radius <= 0 {
            self.rect(rect, color);
            return;
        }

        let center_w = rect.w - radius * 2;
        if center_w > 0 {
            self.rect(
                Rect {
                    x: rect.x + radius,
                    y: rect.y,
                    w: center_w,
                    h: rect.h,
                },
                color,
            );
        }

        let side_h = rect.h - radius * 2;
        if side_h > 0 {
            self.rect(
                Rect {
                    x: rect.x,
                    y: rect.y + radius,
                    w: radius,
                    h: side_h,
                },
                color,
            );
            self.rect(
                Rect {
                    x: rect.x + rect.w - radius,
                    y: rect.y + radius,
                    w: radius,
                    h: side_h,
                },
                color,
            );
        }

        let rr = (radius * radius) as f32;
        for dy in 0..radius {
            for dx in 0..radius {
                let coverage = corner_coverage(radius, dx, dy, rr);
                if coverage == 0 {
                    continue;
                }

                let tl = (rect.x + dx, rect.y + dy);
                let tr = (rect.x + rect.w - 1 - dx, rect.y + dy);
                let bl = (rect.x + dx, rect.y + rect.h - 1 - dy);
                let br = (rect.x + rect.w - 1 - dx, rect.y + rect.h - 1 - dy);
                if coverage == 255 {
                    self.fill_pixel(tl.0, tl.1, color);
                    self.fill_pixel(tr.0, tr.1, color);
                    self.fill_pixel(bl.0, bl.1, color);
                    self.fill_pixel(br.0, br.1, color);
                } else {
                    self.blend_pixel(tl.0, tl.1, color, coverage);
                    self.blend_pixel(tr.0, tr.1, color, coverage);
                    self.blend_pixel(bl.0, bl.1, color, coverage);
                    self.blend_pixel(br.0, br.1, color, coverage);
                }
            }
        }
    }

    fn fill_pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return;
        }

        let offset = ((y * self.width + x) * 4) as usize;
        self.data[offset] = color.b;
        self.data[offset + 1] = color.g;
        self.data[offset + 2] = color.r;
        self.data[offset + 3] = color.a;
    }

    pub fn stroke_rect(&mut self, rect: Rect, color: Color) {
        self.rect(
            Rect {
                x: rect.x,
                y: rect.y,
                w: rect.w,
                h: 1,
            },
            color,
        );
        self.rect(
            Rect {
                x: rect.x,
                y: rect.y + rect.h - 1,
                w: rect.w,
                h: 1,
            },
            color,
        );
        self.rect(
            Rect {
                x: rect.x,
                y: rect.y,
                w: 1,
                h: rect.h,
            },
            color,
        );
        self.rect(
            Rect {
                x: rect.x + rect.w - 1,
                y: rect.y,
                w: 1,
                h: rect.h,
            },
            color,
        );
    }

    pub fn text_centered(
        &mut self,
        font: &RefCell<Option<TextRenderer>>,
        text: &str,
        rect: Rect,
        color: Color,
    ) {
        let measured = font
            .borrow_mut()
            .as_mut()
            .map(|renderer| renderer.measure_text(text))
            .unwrap_or_else(|| text.chars().count() as i32 * 8);
        let x = rect.x + (rect.w - measured).max(0) / 2;
        let baseline = rect.y + (rect.h / 2) + 5;
        self.text_clipped(font, text, x, baseline, rect.w, color);
    }

    pub fn text_right_aligned(
        &mut self,
        font: &RefCell<Option<TextRenderer>>,
        text: &str,
        rect: Rect,
        color: Color,
    ) {
        const RIGHT_PAD: i32 = 8;
        let measured = font
            .borrow_mut()
            .as_mut()
            .map(|renderer| renderer.measure_text(text))
            .unwrap_or_else(|| text.chars().count() as i32 * 8);
        let x = rect.x + (rect.w - measured - RIGHT_PAD).max(0);
        let baseline = rect.y + (rect.h / 2) + 5;
        self.text_clipped(font, text, x, baseline, rect.w, color);
    }

    pub fn text_clipped(
        &mut self,
        font: &RefCell<Option<TextRenderer>>,
        text: &str,
        x: i32,
        baseline: i32,
        max_w: i32,
        color: Color,
    ) {
        if max_w <= 0 {
            return;
        }
        if let Some(renderer) = font.borrow_mut().as_mut() {
            if renderer.draw_text(self, text, x, baseline, max_w, color) {
                return;
            }
        }
        bitmap::draw_bitmap_text(self, text, x, baseline - 10, max_w, color);
    }

    pub fn blend_pixel(&mut self, x: i32, y: i32, color: Color, alpha: u8) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height || alpha == 0 {
            return;
        }
        let offset = ((y * self.width + x) * 4) as usize;
        let src_a = (u16::from(color.a) * u16::from(alpha)) / 255;
        let inv_a = 255 - src_a;

        let dst_b = u16::from(self.data[offset]);
        let dst_g = u16::from(self.data[offset + 1]);
        let dst_r = u16::from(self.data[offset + 2]);

        self.data[offset] = ((u16::from(color.b) * src_a + dst_b * inv_a) / 255) as u8;
        self.data[offset + 1] = ((u16::from(color.g) * src_a + dst_g * inv_a) / 255) as u8;
        self.data[offset + 2] = ((u16::from(color.r) * src_a + dst_r * inv_a) / 255) as u8;
        self.data[offset + 3] = 255;
    }

    pub fn draw_image(&mut self, rect: Rect, image: &IconImage) {
        if image.width == 0 || image.height == 0 {
            return;
        }

        let start_x = rect.x + (rect.w - image.width as i32) / 2;
        let start_y = rect.y + (rect.h - image.height as i32) / 2;

        for src_y in 0..image.height as i32 {
            let dst_y = start_y + src_y;
            if dst_y < 0 || dst_y >= self.height {
                continue;
            }
            for src_x in 0..image.width as i32 {
                let dst_x = start_x + src_x;
                if dst_x < 0 || dst_x >= self.width {
                    continue;
                }

                let src_offset = ((src_y as u32 * image.width + src_x as u32) * 4) as usize;
                let src_b = image.bgra[src_offset];
                let src_g = image.bgra[src_offset + 1];
                let src_r = image.bgra[src_offset + 2];
                let src_a = image.bgra[src_offset + 3];
                if src_a == 0 {
                    continue;
                }

                let dst_offset = ((dst_y * self.width + dst_x) * 4) as usize;
                let inv_a = 255 - u16::from(src_a);
                let src_a_u16 = u16::from(src_a);
                let dst_b = u16::from(self.data[dst_offset]);
                let dst_g = u16::from(self.data[dst_offset + 1]);
                let dst_r = u16::from(self.data[dst_offset + 2]);

                self.data[dst_offset] =
                    ((u16::from(src_b) * src_a_u16 + dst_b * inv_a) / 255) as u8;
                self.data[dst_offset + 1] =
                    ((u16::from(src_g) * src_a_u16 + dst_g * inv_a) / 255) as u8;
                self.data[dst_offset + 2] =
                    ((u16::from(src_r) * src_a_u16 + dst_r * inv_a) / 255) as u8;
                self.data[dst_offset + 3] = 255;
            }
        }
    }
}

fn clamped_radius(width: i32, height: i32, desired: i32) -> i32 {
    if width <= 0 || height <= 0 || desired <= 0 {
        return 0;
    }
    desired.min(width / 2).min(height / 2)
}

fn argb(color: Color) -> u32 {
    (u32::from(color.a) << 24)
        | (u32::from(color.r) << 16)
        | (u32::from(color.g) << 8)
        | u32::from(color.b)
}

fn corner_coverage(radius: i32, dx: i32, dy: i32, rr: f32) -> u8 {
    if radius <= 1 {
        let cx = radius as f32 - (dx as f32 + 0.5);
        let cy = radius as f32 - (dy as f32 + 0.5);
        return if cx * cx + cy * cy <= rr { 255 } else { 0 };
    }

    let mut inside = 0u8;
    for sy in CORNER_AA_SAMPLE_OFFSETS {
        for sx in CORNER_AA_SAMPLE_OFFSETS {
            let cx = radius as f32 - (dx as f32 + sx);
            let cy = radius as f32 - (dy as f32 + sy);
            if cx * cx + cy * cy <= rr {
                inside += 1;
            }
        }
    }
    match inside {
        0 => 0,
        4 => 255,
        _ => ((u16::from(inside) * 255) / 4) as u8,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use meridian_config::Color;

    use super::{clamped_radius, corner_coverage, Painter};
    use crate::{icons::IconImage, Rect};

    fn pixel_at(data: &[u8], width: i32, x: i32, y: i32) -> [u8; 4] {
        let off = ((y * width + x) * 4) as usize;
        [data[off], data[off + 1], data[off + 2], data[off + 3]]
    }

    fn min_lit_x(data: &[u8], width: i32, height: i32) -> Option<i32> {
        let mut min_x: Option<i32> = None;
        for y in 0..height {
            for x in 0..width {
                let off = ((y * width + x) * 4) as usize;
                if data[off + 3] != 0 {
                    min_x = Some(match min_x {
                        Some(current) => current.min(x),
                        None => x,
                    });
                }
            }
        }
        min_x
    }

    #[test]
    fn radius_is_clamped_to_half_extent() {
        assert_eq!(clamped_radius(10, 6, 8), 3);
        assert_eq!(clamped_radius(8, 8, 99), 4);
    }

    #[test]
    fn radius_zero_behaves_like_rect_fill() {
        let mut data = vec![0u8; 6 * 6 * 4];
        let mut painter = Painter::new(&mut data, 6, 6);
        let color = Color::rgb(0xaa, 0xbb, 0xcc);
        painter.fill_rounded_rect(
            Rect {
                x: 1,
                y: 1,
                w: 4,
                h: 4,
            },
            color,
            0,
        );
        assert_eq!(pixel_at(&data, 6, 1, 1), [0xcc, 0xbb, 0xaa, 0xff]);
        assert_eq!(pixel_at(&data, 6, 4, 4), [0xcc, 0xbb, 0xaa, 0xff]);
        assert_eq!(pixel_at(&data, 6, 0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn rounded_corners_clip_outer_pixels() {
        let mut data = vec![0u8; 8 * 8 * 4];
        let mut painter = Painter::new(&mut data, 8, 8);
        let color = Color::rgb(0x11, 0x22, 0x33);
        painter.fill_rounded_rect(
            Rect {
                x: 1,
                y: 1,
                w: 6,
                h: 6,
            },
            color,
            3,
        );
        assert_eq!(pixel_at(&data, 8, 1, 1), [0, 0, 0, 0]);
        assert_eq!(pixel_at(&data, 8, 2, 2), [0x33, 0x22, 0x11, 0xff]);
    }

    #[test]
    fn tiny_rectangles_are_handled_consistently() {
        let mut data = vec![0u8; 4 * 4 * 4];
        let color = Color::rgb(0x44, 0x55, 0x66);

        {
            let mut painter = Painter::new(&mut data, 4, 4);
            painter.fill_rounded_rect(
                Rect {
                    x: 1,
                    y: 1,
                    w: 0,
                    h: 2,
                },
                color,
                6,
            );
        }
        assert_eq!(pixel_at(&data, 4, 1, 1), [0, 0, 0, 0]);
        assert_eq!(pixel_at(&data, 4, 1, 2), [0, 0, 0, 0]);

        {
            let mut painter = Painter::new(&mut data, 4, 4);
            painter.fill_rounded_rect(
                Rect {
                    x: 0,
                    y: 0,
                    w: 1,
                    h: 1,
                },
                color,
                6,
            );
        }
        assert_eq!(pixel_at(&data, 4, 0, 0), [0x66, 0x55, 0x44, 0xff]);

        {
            let mut painter = Painter::new(&mut data, 4, 4);
            painter.fill_rounded_rect(
                Rect {
                    x: 2,
                    y: 2,
                    w: 2,
                    h: 2,
                },
                color,
                6,
            );
        }
        assert_eq!(pixel_at(&data, 4, 2, 2), [0x66, 0x55, 0x44, 0xff]);
        assert_eq!(pixel_at(&data, 4, 3, 2), [0x66, 0x55, 0x44, 0xff]);
        assert_eq!(pixel_at(&data, 4, 2, 3), [0x66, 0x55, 0x44, 0xff]);
        assert_eq!(pixel_at(&data, 4, 3, 3), [0x66, 0x55, 0x44, 0xff]);
    }

    #[test]
    fn rounded_fill_uses_rect_alpha_write_semantics() {
        let mut data = vec![0x11u8; 8 * 8 * 4];
        let mut painter = Painter::new(&mut data, 8, 8);
        let color = Color::rgba(0xaa, 0xbb, 0xcc, 0x80);
        painter.fill_rounded_rect(
            Rect {
                x: 1,
                y: 1,
                w: 6,
                h: 6,
            },
            color,
            3,
        );
        assert_eq!(pixel_at(&data, 8, 3, 3), [0xcc, 0xbb, 0xaa, 0x80]);
    }

    #[test]
    fn rounded_fill_full_coverage_pixels_are_raw_filled() {
        let mut data = vec![0x20u8; 8 * 8 * 4];
        let mut painter = Painter::new(&mut data, 8, 8);
        let color = Color::rgba(0xaa, 0xbb, 0xcc, 0x80);
        painter.fill_rounded_rect(
            Rect {
                x: 1,
                y: 1,
                w: 6,
                h: 6,
            },
            color,
            3,
        );
        assert_eq!(pixel_at(&data, 8, 3, 3), [0xcc, 0xbb, 0xaa, 0x80]);
    }

    #[test]
    fn rounded_fill_outside_corner_pixels_remain_untouched() {
        let mut data = vec![0x17u8; 8 * 8 * 4];
        let mut painter = Painter::new(&mut data, 8, 8);
        painter.fill_rounded_rect(
            Rect {
                x: 1,
                y: 1,
                w: 6,
                h: 6,
            },
            Color::rgb(0xff, 0xff, 0xff),
            3,
        );
        assert_eq!(pixel_at(&data, 8, 1, 1), [0x17, 0x17, 0x17, 0x17]);
    }

    #[test]
    fn rounded_fill_edge_pixels_use_partial_blending() {
        let mut data = vec![0u8; 8 * 8 * 4];
        let mut painter = Painter::new(&mut data, 8, 8);
        let color = Color::rgb(0xff, 0xff, 0xff);
        painter.fill_rounded_rect(
            Rect {
                x: 1,
                y: 1,
                w: 6,
                h: 6,
            },
            color,
            3,
        );
        let edge = pixel_at(&data, 8, 2, 1);
        assert!(edge[0] > 0 && edge[0] < 0xff);
        assert!(edge[1] > 0 && edge[1] < 0xff);
        assert!(edge[2] > 0 && edge[2] < 0xff);
        assert_eq!(edge[3], 0xff);
    }

    #[test]
    fn corner_coverage_reports_expected_partial_and_full_values() {
        let rr = (3 * 3) as f32;
        assert_eq!(corner_coverage(3, 0, 0, rr), 0);
        assert!(corner_coverage(3, 1, 0, rr) < 255);
        assert_eq!(corner_coverage(3, 1, 1, rr), 255);
    }

    #[test]
    fn text_centered_uses_fallback_measurement_when_font_missing() {
        let mut data = vec![0u8; 48 * 20 * 4];
        let mut painter = Painter::new(&mut data, 48, 20);
        let no_font = RefCell::new(None);
        let rect = Rect {
            x: 0,
            y: 0,
            w: 48,
            h: 20,
        };
        painter.text_centered(&no_font, "A", rect, Color::rgb(0xff, 0xff, 0xff));
        let expected_x = (rect.w - 8) / 2;
        assert_eq!(min_lit_x(&data, 48, 20), Some(expected_x));
    }

    #[test]
    fn text_right_aligned_uses_right_padding_when_font_missing() {
        let mut data = vec![0u8; 48 * 20 * 4];
        let mut painter = Painter::new(&mut data, 48, 20);
        let no_font = RefCell::new(None);
        let rect = Rect {
            x: 0,
            y: 0,
            w: 48,
            h: 20,
        };
        painter.text_right_aligned(&no_font, "A", rect, Color::rgb(0xff, 0xff, 0xff));
        let expected_x = rect.w - 8 - 8;
        assert_eq!(min_lit_x(&data, 48, 20), Some(expected_x));
    }

    #[test]
    fn draw_image_blits_centered_pixels() {
        let mut data = vec![0u8; 8 * 8 * 4];
        let mut painter = Painter::new(&mut data, 8, 8);
        let image = IconImage {
            width: 2,
            height: 2,
            bgra: vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255],
        };

        painter.draw_image(
            Rect {
                x: 2,
                y: 2,
                w: 4,
                h: 4,
            },
            &image,
        );

        assert_eq!(pixel_at(&data, 8, 3, 3), [1, 2, 3, 255]);
        assert_eq!(pixel_at(&data, 8, 4, 3), [4, 5, 6, 255]);
        assert_eq!(pixel_at(&data, 8, 3, 4), [7, 8, 9, 255]);
        assert_eq!(pixel_at(&data, 8, 4, 4), [10, 11, 12, 255]);
    }

    #[test]
    fn draw_image_clips_out_of_bounds_without_panic() {
        let mut data = vec![0u8; 4 * 4 * 4];
        let mut painter = Painter::new(&mut data, 4, 4);
        let mut pixels = Vec::with_capacity(3 * 3 * 4);
        for _ in 0..(3 * 3) {
            pixels.extend_from_slice(&[200, 150, 100, 255]);
        }
        let image = IconImage {
            width: 3,
            height: 3,
            bgra: pixels,
        };

        painter.draw_image(
            Rect {
                x: -1,
                y: -1,
                w: 3,
                h: 3,
            },
            &image,
        );

        assert_eq!(pixel_at(&data, 4, 0, 0), [200, 150, 100, 255]);
    }

    #[test]
    fn draw_image_alpha_blending_respects_transparent_and_opaque_pixels() {
        let mut data = vec![0u8; 8];
        let mut painter = Painter::new(&mut data, 2, 1);
        painter.clear(Color::rgb(20, 30, 40));

        let image = IconImage {
            width: 2,
            height: 1,
            bgra: vec![255, 0, 0, 0, 0, 255, 0, 255],
        };

        painter.draw_image(
            Rect {
                x: 0,
                y: 0,
                w: 2,
                h: 1,
            },
            &image,
        );

        assert_eq!(pixel_at(&data, 2, 0, 0), [40, 30, 20, 255]);
        assert_eq!(pixel_at(&data, 2, 1, 0), [0, 255, 0, 255]);
    }
}
