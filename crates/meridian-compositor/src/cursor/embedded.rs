pub(super) const CURSOR_WIDTH: u32 = 32;
pub(super) const CURSOR_HEIGHT: u32 = 32;

const ARROW_POLYGON: [(f32, f32); 7] = [
    (0.0, 0.0),
    (0.0, 22.0),
    (5.0, 17.0),
    (8.0, 27.0),
    (12.0, 25.0),
    (9.0, 15.0),
    (16.0, 15.0),
];

const SAMPLE_GRID: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EmbeddedCursorKind {
    Default,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
}

fn point_in_polygon(x: f32, y: f32, poly: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        let crosses = (yi > y) != (yj > y);
        if crosses {
            let t = (y - yi) / (yj - yi);
            let x_cross = xi + t * (xj - xi);
            if x < x_cross {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

fn scaled_arrow_polygon(width: u32, height: u32) -> Vec<(f32, f32)> {
    let sx = if CURSOR_WIDTH > 1 {
        (width.saturating_sub(1)) as f32 / (CURSOR_WIDTH - 1) as f32
    } else {
        1.0
    };
    let sy = if CURSOR_HEIGHT > 1 {
        (height.saturating_sub(1)) as f32 / (CURSOR_HEIGHT - 1) as f32
    } else {
        1.0
    };
    ARROW_POLYGON
        .iter()
        .map(|(x, y)| (x * sx, y * sy))
        .collect()
}

fn point_segment_distance(x: f32, y: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let vx = bx - ax;
    let vy = by - ay;
    let wx = x - ax;
    let wy = y - ay;
    let c1 = vx * wx + vy * wy;
    if c1 <= 0.0 {
        let dx = x - ax;
        let dy = y - ay;
        return (dx * dx + dy * dy).sqrt();
    }
    let c2 = vx * vx + vy * vy;
    if c2 <= c1 {
        let dx = x - bx;
        let dy = y - by;
        return (dx * dx + dy * dy).sqrt();
    }
    let t = c1 / c2;
    let px = ax + t * vx;
    let py = ay + t * vy;
    let dx = x - px;
    let dy = y - py;
    (dx * dx + dy * dy).sqrt()
}

fn min_distance_to_polygon_edges(x: f32, y: f32, poly: &[(f32, f32)]) -> f32 {
    let mut min_d = f32::MAX;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (ax, ay) = poly[j];
        let (bx, by) = poly[i];
        min_d = min_d.min(point_segment_distance(x, y, ax, ay, bx, by));
        j = i;
    }
    min_d
}

pub(super) fn make_cursor_pixels(width: u32, height: u32) -> Vec<u8> {
    let mut px = vec![0u8; width as usize * height as usize * 4];
    let polygon = scaled_arrow_polygon(width, height);
    let sample_count = SAMPLE_GRID * SAMPLE_GRID;
    let border_width = (width.min(height) as f32 / CURSOR_WIDTH as f32).clamp(0.9, 1.8);

    for y in 0..height as usize {
        for x in 0..width as usize {
            let mut fill_samples = 0u32;
            let mut border_samples = 0u32;

            for sy in 0..SAMPLE_GRID {
                for sx in 0..SAMPLE_GRID {
                    let fx = x as f32 + (sx as f32 + 0.5) / SAMPLE_GRID as f32;
                    let fy = y as f32 + (sy as f32 + 0.5) / SAMPLE_GRID as f32;
                    if !point_in_polygon(fx, fy, &polygon) {
                        continue;
                    }

                    let edge_dist = min_distance_to_polygon_edges(fx, fy, &polygon);
                    if edge_dist <= border_width {
                        border_samples += 1;
                    } else {
                        fill_samples += 1;
                    }
                }
            }

            let visible_samples = fill_samples + border_samples;
            if visible_samples == 0 {
                continue;
            }

            let alpha = ((visible_samples * 255 + sample_count / 2) / sample_count) as u8;
            // premultiplied alpha: white fill contributes RGB, black border contributes only alpha
            let fill_alpha = ((fill_samples * 255 + sample_count / 2) / sample_count) as u8;

            let off = (y * width as usize + x) * 4;
            px[off] = fill_alpha;
            px[off + 1] = fill_alpha;
            px[off + 2] = fill_alpha;
            px[off + 3] = alpha;
        }
    }

    // Force exact hotspot pixel to an opaque black point for precise click feedback.
    if !px.is_empty() {
        px[0] = 0;
        px[1] = 0;
        px[2] = 0;
        px[3] = 255;
    }

    px
}

fn draw_polyline_cursor(width: u32, height: u32, segments: &[((f32, f32), (f32, f32))]) -> Vec<u8> {
    let mut px = vec![0u8; width as usize * height as usize * 4];
    let min_dim = width.min(height) as f32;
    let core = (min_dim / 24.0).clamp(1.0, 2.0);
    let border = core + 1.2;

    for y in 0..height as usize {
        for x in 0..width as usize {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let min_d = segments.iter().fold(f32::MAX, |acc, seg| {
                let ((ax, ay), (bx, by)) = *seg;
                acc.min(point_segment_distance(fx, fy, ax, ay, bx, by))
            });
            if min_d > border {
                continue;
            }
            let off = (y * width as usize + x) * 4;
            px[off + 3] = 255;
            if min_d <= core {
                px[off] = 255;
                px[off + 1] = 255;
                px[off + 2] = 255;
            }
        }
    }

    px
}

fn resize_cursor_segments(
    width: u32,
    height: u32,
    kind: EmbeddedCursorKind,
) -> Vec<((f32, f32), (f32, f32))> {
    let w = width.saturating_sub(1) as f32;
    let h = height.saturating_sub(1) as f32;
    let min_dim = width.min(height) as f32;
    let margin = (min_dim * 0.2).clamp(4.0, 10.0);
    let head = (min_dim * 0.22).clamp(5.0, 12.0);
    let cx = w * 0.5;
    let cy = h * 0.5;

    match kind {
        EmbeddedCursorKind::EwResize => vec![
            ((margin, cy), (w - margin, cy)),
            ((margin, cy), (margin + head, cy - head)),
            ((margin, cy), (margin + head, cy + head)),
            ((w - margin, cy), (w - margin - head, cy - head)),
            ((w - margin, cy), (w - margin - head, cy + head)),
        ],
        EmbeddedCursorKind::NsResize => vec![
            ((cx, margin), (cx, h - margin)),
            ((cx, margin), (cx - head, margin + head)),
            ((cx, margin), (cx + head, margin + head)),
            ((cx, h - margin), (cx - head, h - margin - head)),
            ((cx, h - margin), (cx + head, h - margin - head)),
        ],
        EmbeddedCursorKind::NeswResize => vec![
            ((margin, h - margin), (w - margin, margin)),
            ((margin, h - margin), (margin + head, h - margin)),
            ((margin, h - margin), (margin, h - margin - head)),
            ((w - margin, margin), (w - margin - head, margin)),
            ((w - margin, margin), (w - margin, margin + head)),
        ],
        EmbeddedCursorKind::NwseResize => vec![
            ((margin, margin), (w - margin, h - margin)),
            ((margin, margin), (margin + head, margin)),
            ((margin, margin), (margin, margin + head)),
            ((w - margin, h - margin), (w - margin - head, h - margin)),
            ((w - margin, h - margin), (w - margin, h - margin - head)),
        ],
        EmbeddedCursorKind::Default => Vec::new(),
    }
}

pub(super) fn make_cursor_pixels_for_kind(
    width: u32,
    height: u32,
    kind: EmbeddedCursorKind,
) -> Vec<u8> {
    match kind {
        EmbeddedCursorKind::Default => make_cursor_pixels(width, height),
        EmbeddedCursorKind::EwResize
        | EmbeddedCursorKind::NsResize
        | EmbeddedCursorKind::NeswResize
        | EmbeddedCursorKind::NwseResize => {
            let segments = resize_cursor_segments(width, height, kind);
            draw_polyline_cursor(width, height, &segments)
        }
    }
}
