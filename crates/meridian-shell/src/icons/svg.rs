use resvg::{tiny_skia, usvg};

use crate::icons::IconImage;

/// Decode SVG bytes into an IconImage rendered at exactly (size x size).
/// Returns None for invalid/unparseable SVG. Output is BGRA non-premultiplied.
#[allow(dead_code)]
pub fn decode_svg(data: &[u8], size: u32) -> Option<IconImage> {
    decode_svg_with_symbolic_color(data, size, "#c0caf5")
}

pub fn decode_svg_with_symbolic_color(
    data: &[u8],
    size: u32,
    symbolic_color: &str,
) -> Option<IconImage> {
    if size == 0 || data.is_empty() {
        return None;
    }

    let options = usvg::Options::default();
    let substituted = substitute_color_scheme(data, symbolic_color);
    let tree = usvg::Tree::from_data(&substituted, &options).ok()?;
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

fn substitute_color_scheme(data: &[u8], symbolic_color: &str) -> Vec<u8> {
    const NEEDLES: &[&[u8]] = &[
        b"color:#232629",
        b"color: #232629",
        b"color:#2a2e32",
        b"color: #2a2e32",
        b"color:#fcfcfc",
        b"color: #fcfcfc",
    ];
    if !NEEDLES.iter().any(|needle| memchr_contains(data, needle)) {
        return data.to_vec();
    }
    let s = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return data.to_vec(),
    };
    let color = if is_hex_color(symbolic_color) {
        symbolic_color
    } else {
        "#c0caf5"
    };
    let mut out = s.to_string();
    for needle in NEEDLES {
        let needle_str = std::str::from_utf8(needle).expect("ascii needle");
        let repl = if needle_str.contains(": ") {
            format!("color: {color}")
        } else {
            format!("color:{color}")
        };
        out = out.replace(needle_str, &repl);
    }
    out.into_bytes()
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}

fn memchr_contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
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
    use super::{decode_svg, decode_svg_with_symbolic_color, substitute_color_scheme};

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

    #[test]
    fn decode_svg_substitutes_breeze_color_scheme() {
        let svg =
            br#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
            <defs><style>.ColorScheme-Text { color:#232629; }</style></defs>
            <rect width="10" height="10" style="fill:currentColor" class="ColorScheme-Text"/>
        </svg>"#;
        let image = decode_svg(svg, 10).expect("decode");
        let off = (5 * 10 + 5) * 4;
        assert_eq!(image.bgra[off], 0xf5, "B");
        assert_eq!(image.bgra[off + 1], 0xca, "G");
        assert_eq!(image.bgra[off + 2], 0xc0, "R");
        assert!(image.bgra[off + 3] > 200, "alpha-opaque-ish");
    }

    #[test]
    fn decode_svg_uses_custom_symbolic_color() {
        let svg =
            br##"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
            <defs><style>.ColorScheme-Text { color:#232629; }</style></defs>
            <rect width="10" height="10" style="fill:currentColor" class="ColorScheme-Text"/>
        </svg>"##;
        let image = decode_svg_with_symbolic_color(svg, 10, "#3f372e").expect("decode");
        let off = (5 * 10 + 5) * 4;
        assert_eq!(image.bgra[off], 0x2e, "B");
        assert_eq!(image.bgra[off + 1], 0x37, "G");
        assert_eq!(image.bgra[off + 2], 0x3f, "R");
        assert!(image.bgra[off + 3] > 200, "alpha-opaque-ish");
    }

    #[test]
    fn substitute_color_scheme_no_breeze_marker_is_noop() {
        let svg = b"<svg><rect fill='#ff0000'/></svg>";
        let out = substitute_color_scheme(svg, "#c0caf5");
        assert_eq!(out.as_slice(), svg);
    }
}
