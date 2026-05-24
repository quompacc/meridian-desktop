mod cache;
mod loader;
mod rcc;
pub mod svg;
mod theme_index;

pub use cache::IconCache;

#[cfg(test)]
pub(crate) use loader::IconLoader;

#[derive(Debug, Clone)]
pub struct IconImage {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

pub(crate) fn icon_image_to_pixmap(img: &IconImage) -> Option<tiny_skia::Pixmap> {
    let mut pixmap = tiny_skia::Pixmap::new(img.width, img.height)?;
    let data = pixmap.data_mut();
    for (i, chunk) in img.bgra.chunks_exact(4).enumerate() {
        let (b, g, r, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
        let out_idx = i * 4;
        data[out_idx] = ((r as u16 * a as u16) / 255) as u8;
        data[out_idx + 1] = ((g as u16 * a as u16) / 255) as u8;
        data[out_idx + 2] = ((b as u16 * a as u16) / 255) as u8;
        data[out_idx + 3] = a;
    }
    Some(pixmap)
}

pub fn lookup_default_theme() -> &'static str {
    "breeze"
}

#[cfg(test)]
mod tests {
    use super::lookup_default_theme;

    #[test]
    fn default_theme_is_breeze() {
        assert_eq!(lookup_default_theme(), "breeze");
    }
}
