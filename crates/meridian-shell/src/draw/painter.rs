use std::cell::RefCell;

use meridian_config::Color;

use crate::Rect;

use super::{bitmap, text::TextRenderer};

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
        self.rect(rect, color);
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

fn argb(color: Color) -> u32 {
    (u32::from(color.a) << 24)
        | (u32::from(color.r) << 16)
        | (u32::from(color.g) << 8)
        | u32::from(color.b)
}
