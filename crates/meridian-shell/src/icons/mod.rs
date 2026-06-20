mod cache;
mod loader;
mod rcc;
pub mod svg;
mod theme_index;

pub use cache::IconCache;

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

    // Relocated from the removed ui_preview.rs tombstone: this exercises
    // icon_image_to_pixmap, which lives in this module.
    #[test]
    fn icon_image_to_pixmap_bgra_to_premul() {
        use super::{icon_image_to_pixmap, IconImage};
        let img = IconImage {
            width: 1,
            height: 1,
            bgra: vec![0, 0, 255, 128],
        };
        let pixmap = icon_image_to_pixmap(&img).expect("pixmap");
        assert_eq!(pixmap.width(), 1);
        let px = pixmap.pixel(0, 0).expect("pixel");
        assert_eq!(px.red(), 128);
        assert_eq!(px.green(), 0);
        assert_eq!(px.blue(), 0);
        assert_eq!(px.alpha(), 128);
    }
}
