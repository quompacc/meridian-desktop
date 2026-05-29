// ui_preview removed – command palette replaces tile view.
// Tests for icon_image_to_pixmap kept here to avoid churn.

#[cfg(test)]
mod tests {
    #[test]
    fn icon_image_to_pixmap_bgra_to_premul() {
        use crate::icons::{icon_image_to_pixmap, IconImage};
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
