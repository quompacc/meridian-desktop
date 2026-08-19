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
fn rounded_fill_uses_premultiplied_alpha_write_semantics() {
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
    assert_eq!(pixel_at(&data, 8, 3, 3), [0x66, 0x5d, 0x55, 0x80]);
}

#[test]
fn rounded_fill_full_coverage_pixels_are_premultiplied() {
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
    assert_eq!(pixel_at(&data, 8, 3, 3), [0x66, 0x5d, 0x55, 0x80]);
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
    assert_eq!(edge[0], edge[3]);
    assert_eq!(edge[1], edge[3]);
    assert_eq!(edge[2], edge[3]);
    assert!(edge[3] > 0 && edge[3] < 0xff);
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
