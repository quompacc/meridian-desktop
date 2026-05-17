#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowIcon {
    Minimize,
    Maximize,
    Restore,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconTint {
    OnSurface,
    OnAccentRed,
}

type Segment = ((f32, f32), (f32, f32));

const SAMPLE_GRID: u32 = 4;
const VIEWBOX_SIZE: f32 = 16.0;

const MINIMIZE_SEGMENTS: [Segment; 1] = [((3.0, 8.0), (13.0, 8.0))];
const MAXIMIZE_SEGMENTS: [Segment; 4] = [
    ((3.0, 3.0), (13.0, 3.0)),
    ((13.0, 3.0), (13.0, 13.0)),
    ((13.0, 13.0), (3.0, 13.0)),
    ((3.0, 13.0), (3.0, 3.0)),
];
const RESTORE_SEGMENTS: [Segment; 8] = [
    ((4.0, 4.0), (13.0, 4.0)),
    ((13.0, 4.0), (13.0, 11.0)),
    ((13.0, 11.0), (4.0, 11.0)),
    ((4.0, 11.0), (4.0, 4.0)),
    ((3.0, 6.0), (11.0, 6.0)),
    ((11.0, 6.0), (11.0, 13.0)),
    ((11.0, 13.0), (3.0, 13.0)),
    ((3.0, 13.0), (3.0, 6.0)),
];
const CLOSE_SEGMENTS: [Segment; 2] = [((4.0, 4.0), (12.0, 12.0)), ((12.0, 4.0), (4.0, 12.0))];

fn icon_segments(kind: WindowIcon) -> &'static [Segment] {
    match kind {
        WindowIcon::Minimize => &MINIMIZE_SEGMENTS,
        WindowIcon::Maximize => &MAXIMIZE_SEGMENTS,
        WindowIcon::Restore => &RESTORE_SEGMENTS,
        WindowIcon::Close => &CLOSE_SEGMENTS,
    }
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

fn scale_viewbox_point(size_px: u32, x: f32, y: f32) -> (f32, f32) {
    let px_extent = (size_px.saturating_sub(1)) as f32;
    let s = if VIEWBOX_SIZE > 1.0 {
        px_extent / (VIEWBOX_SIZE - 1.0)
    } else {
        1.0
    };
    (x * s, y * s)
}

pub fn rasterize(
    kind: WindowIcon,
    size_px: u32,
    stroke_color_rgba: [u8; 4],
    stroke_width: f32,
) -> Vec<u8> {
    if size_px == 0 {
        return Vec::new();
    }

    let mut out = vec![0u8; size_px as usize * size_px as usize * 4];
    let samples_per_px = SAMPLE_GRID * SAMPLE_GRID;
    let stroke_radius = (stroke_width.max(0.1)) * 0.5;
    let segments = icon_segments(kind);
    let src_alpha = stroke_color_rgba[3] as f32 / 255.0;

    for y in 0..size_px as usize {
        for x in 0..size_px as usize {
            let mut covered = 0u32;
            for sy in 0..SAMPLE_GRID {
                for sx in 0..SAMPLE_GRID {
                    let fx = x as f32 + (sx as f32 + 0.5) / SAMPLE_GRID as f32;
                    let fy = y as f32 + (sy as f32 + 0.5) / SAMPLE_GRID as f32;
                    let mut hit = false;
                    for segment in segments {
                        let ((ax, ay), (bx, by)) = *segment;
                        let (pax, pay) = scale_viewbox_point(size_px, ax, ay);
                        let (pbx, pby) = scale_viewbox_point(size_px, bx, by);
                        if point_segment_distance(fx, fy, pax, pay, pbx, pby) <= stroke_radius {
                            hit = true;
                            break;
                        }
                    }
                    if hit {
                        covered += 1;
                    }
                }
            }

            if covered == 0 {
                continue;
            }

            let coverage = covered as f32 / samples_per_px as f32;
            let a = (coverage * src_alpha).clamp(0.0, 1.0);
            let off = (y * size_px as usize + x) * 4;
            out[off] = (stroke_color_rgba[0] as f32 * a).round() as u8;
            out[off + 1] = (stroke_color_rgba[1] as f32 * a).round() as u8;
            out[off + 2] = (stroke_color_rgba[2] as f32 * a).round() as u8;
            out[off + 3] = (255.0 * a).round() as u8;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{rasterize, WindowIcon};

    fn pixel(buf: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let off = ((y * size + x) * 4) as usize;
        [buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]
    }

    #[test]
    fn test_rasterize_minimize_returns_correct_size() {
        let out = rasterize(WindowIcon::Minimize, 16, [255, 255, 255, 255], 1.5);
        assert_eq!(out.len(), 16 * 16 * 4);
    }

    #[test]
    fn test_rasterize_close_has_pixels_near_diagonals() {
        let out = rasterize(WindowIcon::Close, 16, [255, 255, 255, 255], 1.5);
        assert!(pixel(&out, 16, 4, 4)[3] > 100);
        assert!(pixel(&out, 16, 8, 8)[3] > 100);
        assert!(pixel(&out, 16, 12, 4)[3] > 100);
    }

    #[test]
    fn test_rasterize_close_has_transparent_corners() {
        let out = rasterize(WindowIcon::Close, 16, [255, 255, 255, 255], 1.5);
        assert_eq!(pixel(&out, 16, 0, 0)[3], 0);
        assert_eq!(pixel(&out, 16, 15, 0)[3], 0);
        assert_eq!(pixel(&out, 16, 0, 15)[3], 0);
        assert_eq!(pixel(&out, 16, 15, 15)[3], 0);
    }

    #[test]
    fn test_rasterize_uses_stroke_color() {
        let color = [0x40, 0x90, 0xe0, 255];
        let out = rasterize(WindowIcon::Close, 16, color, 1.5);
        let p = pixel(&out, 16, 8, 8);
        assert!(p[3] > 120);
        assert!((p[0] as i32 - color[0] as i32).abs() <= 8);
        assert!((p[1] as i32 - color[1] as i32).abs() <= 8);
        assert!((p[2] as i32 - color[2] as i32).abs() <= 8);
    }
}
