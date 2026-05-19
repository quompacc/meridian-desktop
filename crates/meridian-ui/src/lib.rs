#![deny(unsafe_code)]
//! Meridian UI foundation crate built on `taffy` and `tiny_skia`.
//! It will host style tokens, visual effects, widget abstractions, and paint paths.
//! This crate currently provides module skeletons and dependency smoke tests only.
//! Render-loop contracts forbid heap allocation and clone-heavy hot-path logic.

pub mod effect;
pub mod paint;
pub mod style;
pub mod widget;
pub use effect::{paint_border, paint_fill, paint_metro_surface, rounded_rect_path};
pub use paint::{compute_layout, render, PixelSize, Rect};
pub use style::Theme;
pub use widget::{Button, Tile, TileSize, Widget};

#[cfg(test)]
mod smoke {
    use taffy::prelude::*;
    use tiny_skia::Pixmap;

    #[test]
    fn taffy_computes_basic_flex_layout() {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        let child = tree
            .new_leaf(Style {
                size: Size {
                    width: length(50.0),
                    height: length(50.0),
                },
                ..Default::default()
            })
            .expect("create child node");
        let root = tree
            .new_with_children(
                Style {
                    size: Size {
                        width: length(200.0),
                        height: length(100.0),
                    },
                    ..Default::default()
                },
                &[child],
            )
            .expect("create root node");

        tree.compute_layout(root, Size::MAX_CONTENT)
            .expect("compute layout");
        let layout = tree.layout(child).expect("load child layout");
        assert_eq!(layout.size.width, 50.0);
        assert_eq!(layout.size.height, 50.0);
    }

    #[test]
    fn tiny_skia_pixmap_allocates() {
        let pixmap = Pixmap::new(64, 64).expect("pixmap alloc");
        assert_eq!(pixmap.width(), 64);
        assert_eq!(pixmap.height(), 64);
    }
}
