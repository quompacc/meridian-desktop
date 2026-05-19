#![deny(unsafe_code)]
//! Meridian UI foundation crate built on `taffy` and `tiny_skia`.
//! It will host style tokens, visual effects, widget abstractions, and paint paths.
//! This crate currently provides module skeletons and dependency smoke tests only.
//! Render-loop contracts forbid heap allocation and clone-heavy hot-path logic.

pub mod effect;
pub mod event;
pub mod paint;
pub mod style;
pub mod widget;
pub use effect::{
    measure_text, paint_border, paint_fill, paint_metro_surface, paint_text, rounded_rect_path,
    ui_font,
};
pub use event::{hit_test, Event, PointerButton, PointerPosition, WidgetPath, WidgetState};
pub use paint::{compute_layout, render, render_idle, PixelSize, Rect};
pub use style::Theme;
pub use widget::{Button, Tile, TileSize, Widget};

pub use taffy::prelude::length as ui_length;
pub use taffy::prelude::span as grid_span;
pub use taffy::prelude::Size as UiSize;
pub use taffy::prelude::Style as WidgetStyle;

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
