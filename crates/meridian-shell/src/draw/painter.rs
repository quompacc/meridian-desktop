use std::cell::RefCell;

use meridian_config::Color;

use crate::Rect;

use super::{bitmap, text::TextRenderer};

const DEFAULT_ROUNDISH_RADIUS: i32 = 6;

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
                let fx = dx as f32 + 0.5;
                let fy = dy as f32 + 0.5;
                let cx = radius as f32 - fx;
                let cy = radius as f32 - fy;
                if cx * cx + cy * cy > rr {
                    continue;
                }

                self.fill_pixel(rect.x + dx, rect.y + dy, color);
                self.fill_pixel(rect.x + rect.w - 1 - dx, rect.y + dy, color);
                self.fill_pixel(rect.x + dx, rect.y + rect.h - 1 - dy, color);
                self.fill_pixel(rect.x + rect.w - 1 - dx, rect.y + rect.h - 1 - dy, color);
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
        let approx_w = text.chars().count() as i32 * 8;
        let x = rect.x + (rect.w - approx_w).max(0) / 2;
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

#[cfg(test)]
mod tests {
    use meridian_config::Color;

    use super::{clamped_radius, Painter};
    use crate::Rect;

    fn pixel_at(data: &[u8], width: i32, x: i32, y: i32) -> [u8; 4] {
        let off = ((y * width + x) * 4) as usize;
        [data[off], data[off + 1], data[off + 2], data[off + 3]]
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
        assert_eq!(pixel_at(&data, 8, 2, 1), [0x33, 0x22, 0x11, 0xff]);
        assert_eq!(pixel_at(&data, 8, 1, 2), [0x33, 0x22, 0x11, 0xff]);
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
}
