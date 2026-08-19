fn decode_to_rgba8(
    src: &[u8],
    color_type: ColorType,
    bit_depth: BitDepth,
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    if bit_depth != BitDepth::Eight {
        return None;
    }

    let pixels = width as usize * height as usize;
    let mut rgba = vec![0u8; pixels * 4];

    match color_type {
        ColorType::Rgba => {
            if src.len() != pixels * 4 {
                return None;
            }
            rgba.copy_from_slice(src);
        }
        ColorType::Rgb => {
            if src.len() != pixels * 3 {
                return None;
            }
            for (index, chunk) in src.chunks_exact(3).enumerate() {
                let out = index * 4;
                rgba[out] = chunk[0];
                rgba[out + 1] = chunk[1];
                rgba[out + 2] = chunk[2];
                rgba[out + 3] = 255;
            }
        }
        ColorType::Grayscale => {
            if src.len() != pixels {
                return None;
            }
            for (index, gray) in src.iter().enumerate() {
                let out = index * 4;
                rgba[out] = *gray;
                rgba[out + 1] = *gray;
                rgba[out + 2] = *gray;
                rgba[out + 3] = 255;
            }
        }
        ColorType::GrayscaleAlpha => {
            if src.len() != pixels * 2 {
                return None;
            }
            for (index, chunk) in src.chunks_exact(2).enumerate() {
                let out = index * 4;
                rgba[out] = chunk[0];
                rgba[out + 1] = chunk[0];
                rgba[out + 2] = chunk[0];
                rgba[out + 3] = chunk[1];
            }
        }
        ColorType::Indexed => {
            return None;
        }
    }

    Some(rgba)
}

fn rgba_to_bgra(rgba: &[u8]) -> Vec<u8> {
    let mut bgra = Vec::with_capacity(rgba.len());
    for chunk in rgba.chunks_exact(4) {
        bgra.push(chunk[2]);
        bgra.push(chunk[1]);
        bgra.push(chunk[0]);
        bgra.push(chunk[3]);
    }
    bgra
}

pub(crate) fn resize_bilinear_bgra(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> Vec<u8> {
    if src_w == dst_w && src_h == dst_h {
        return src.to_vec();
    }

    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];
    let src_wf = src_w as f32;
    let src_hf = src_h as f32;
    let dst_wf = dst_w as f32;
    let dst_hf = dst_h as f32;

    for y in 0..dst_h {
        let fy = ((y as f32 + 0.5) * src_hf / dst_hf - 0.5).clamp(0.0, (src_h - 1) as f32);
        let y0 = fy.floor() as u32;
        let y1 = (y0 + 1).min(src_h - 1);
        let wy = fy - y0 as f32;

        for x in 0..dst_w {
            let fx = ((x as f32 + 0.5) * src_wf / dst_wf - 0.5).clamp(0.0, (src_w - 1) as f32);
            let x0 = fx.floor() as u32;
            let x1 = (x0 + 1).min(src_w - 1);
            let wx = fx - x0 as f32;

            let idx00 = ((y0 * src_w + x0) * 4) as usize;
            let idx10 = ((y0 * src_w + x1) * 4) as usize;
            let idx01 = ((y1 * src_w + x0) * 4) as usize;
            let idx11 = ((y1 * src_w + x1) * 4) as usize;
            let out = ((y * dst_w + x) * 4) as usize;

            for channel in 0..4 {
                let p00 = src[idx00 + channel] as f32;
                let p10 = src[idx10 + channel] as f32;
                let p01 = src[idx01 + channel] as f32;
                let p11 = src[idx11 + channel] as f32;

                let top = p00 + (p10 - p00) * wx;
                let bottom = p01 + (p11 - p01) * wx;
                let value = top + (bottom - top) * wy;
                dst[out + channel] = value.round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    dst
}
