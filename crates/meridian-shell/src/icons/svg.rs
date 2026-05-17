use resvg::{tiny_skia, usvg};

use crate::icons::IconImage;

/// Decode SVG bytes into an IconImage rendered at exactly (size x size).
/// Returns None for invalid/unparseable SVG. Output is BGRA non-premultiplied.
pub fn decode_svg(data: &[u8], size: u32) -> Option<IconImage> {
    if size == 0 || data.is_empty() {
        return None;
    }

    let options = usvg::Options::default();
    let tree = usvg::Tree::from_data(data, &options).ok()?;
    let native_size = tree.size();
    let native_w = native_size.width();
    let native_h = native_size.height();
    if native_w <= 0.0 || native_h <= 0.0 {
        return None;
    }

    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    let target = size as f32;
    let scale = (target / native_w).min(target / native_h);
    let tx = (target - native_w * scale) * 0.5;
    let ty = (target - native_h * scale) * 0.5;
    let transform = tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, tx, ty);
    let mut pixmap_mut = pixmap.as_mut();
    resvg::render(&tree, transform, &mut pixmap_mut);

    let bgra = premultiplied_rgba_to_bgra_nonpremul(pixmap.data());
    Some(IconImage {
        width: size,
        height: size,
        bgra,
    })
}

fn premultiplied_rgba_to_bgra_nonpremul(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks_exact(4) {
        let alpha = chunk[3];
        let r = unpremultiply_channel(chunk[0], alpha);
        let g = unpremultiply_channel(chunk[1], alpha);
        let b = unpremultiply_channel(chunk[2], alpha);
        out.push(b);
        out.push(g);
        out.push(r);
        out.push(alpha);
    }
    out
}

fn unpremultiply_channel(value: u8, alpha: u8) -> u8 {
    if alpha == 0 {
        return 0;
    }

    ((u32::from(value) * 255 + (u32::from(alpha) / 2)) / u32::from(alpha)).min(255) as u8
}

#[cfg(test)]
mod tests {
    use super::decode_svg;

    fn pixel_at(bgra: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let offset = ((y * width + x) * 4) as usize;
        [
            bgra[offset],
            bgra[offset + 1],
            bgra[offset + 2],
            bgra[offset + 3],
        ]
    }

    #[test]
    fn decode_svg_minimal_valid_returns_image() {
        let svg = br#"<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'><rect width='10' height='10' fill='red'/></svg>"#;
        let image = decode_svg(svg, 24).expect("valid svg decode");
        assert_eq!(image.width, 24);
        assert_eq!(image.height, 24);
        let center = pixel_at(&image.bgra, image.width, 12, 12);
        assert!(center[2] > 200);
        assert_eq!(center[3], 255);
    }

    #[test]
    fn decode_svg_renders_at_requested_size() {
        let svg = br#"<svg xmlns='http://www.w3.org/2000/svg' width='100' height='100'><circle cx='50' cy='50' r='40' fill='blue'/></svg>"#;
        let image = decode_svg(svg, 24).expect("scaled svg decode");
        assert_eq!(image.width, 24);
        assert_eq!(image.height, 24);
    }

    #[test]
    fn decode_svg_invalid_returns_none() {
        assert!(decode_svg(b"not svg", 24).is_none());
    }

    #[test]
    fn decode_svg_empty_returns_none() {
        assert!(decode_svg(b"", 24).is_none());
    }

    #[test]
    fn decode_svg_size_zero_returns_none() {
        let svg = br#"<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'></svg>"#;
        assert!(decode_svg(svg, 0).is_none());
    }

    #[test]
    fn decode_svg_alpha_unpremultiplied() {
        let svg = br#"<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'><rect width='10' height='10' fill='#ff0000' fill-opacity='0.5'/></svg>"#;
        let image = decode_svg(svg, 24).expect("alpha svg decode");
        let center = pixel_at(&image.bgra, image.width, 12, 12);
        assert!(center[2] > 200);
        assert!(center[3] >= 120 && center[3] <= 136);
    }
}
