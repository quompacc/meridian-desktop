use taffy::prelude::{
    length, AlignContent, AlignItems, FlexDirection, FlexWrap, JustifyContent, Size, Style,
};
use tiny_skia::PixmapMut;

use crate::{paint::Rect, style::Theme};

/// Trait implemented by all renderable UI widgets.
///
/// The style method is consumed during the setup/layout phase.
/// The paint method is called in the render path and must not allocate.
pub trait Widget {
    /// Return the taffy style used to build this node in the layout tree.
    fn style(&self) -> Style;

    /// Paint this widget into the assigned rectangle.
    ///
    /// Contract: must be allocation-free and side-effect free.
    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme);

    /// Child widgets in tree order.
    fn children(&self) -> &[Box<dyn Widget>] {
        &[]
    }
}

/// Minimal container widget used for bootstrapping and tests.
pub struct Container {
    style: Style,
    children: Vec<Box<dyn Widget>>,
}

impl Container {
    pub fn new(style: Style, children: Vec<Box<dyn Widget>>) -> Self {
        Self { style, children }
    }

    pub fn leaf(style: Style) -> Self {
        Self::new(style, Vec::new())
    }

    pub fn centered_viewport(width: u32, height: u32, children: Vec<Box<dyn Widget>>) -> Self {
        Self::new(
            Style {
                justify_content: Some(JustifyContent::Center),
                align_items: Some(AlignItems::Center),
                size: Size {
                    width: length(width as f32),
                    height: length(height as f32),
                },
                ..Default::default()
            },
            children,
        )
    }

    pub fn flow(
        viewport_width: u32,
        viewport_height: u32,
        gap: i32,
        children: Vec<Box<dyn Widget>>,
    ) -> Self {
        let gap_px = gap.max(0) as f32;
        Self::new(
            Style {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: Some(JustifyContent::Center),
                align_content: Some(AlignContent::Center),
                size: Size {
                    width: length(viewport_width as f32),
                    height: length(viewport_height as f32),
                },
                gap: Size {
                    width: length(gap_px),
                    height: length(gap_px),
                },
                ..Default::default()
            },
            children,
        )
    }
}

impl Widget for Container {
    fn style(&self) -> Style {
        self.style.clone()
    }

    fn paint(&self, _area: Rect, _canvas: &mut PixmapMut<'_>, _theme: &Theme) {}

    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }
}

#[cfg(test)]
mod tests {
    use taffy::prelude::{length, Size, Style};

    use super::{Container, Widget};
    use crate::paint::{compute_layout, PixelSize};

    #[test]
    fn container_leaf_has_no_children() {
        let leaf = Container::leaf(Style::default());
        assert!(leaf.children().is_empty());
    }

    #[test]
    fn container_style_roundtrips() {
        let widget = Container::leaf(Style {
            size: Size {
                width: length(42.0),
                height: length(24.0),
            },
            ..Default::default()
        });
        let style = widget.style();
        assert_eq!(style.size.width, length(42.0));
        assert_eq!(style.size.height, length(24.0));
    }

    #[test]
    fn centered_viewport_sets_center_alignment_and_size() {
        let container = Container::centered_viewport(880, 620, Vec::new());
        let style = container.style();
        assert_eq!(
            style.justify_content,
            Some(taffy::prelude::JustifyContent::Center)
        );
        assert_eq!(style.align_items, Some(taffy::prelude::AlignItems::Center));
        assert_eq!(style.size.width, length(880.0));
        assert_eq!(style.size.height, length(620.0));
    }

    #[test]
    fn flow_wraps_mixed_child_sizes() {
        let children: Vec<Box<dyn Widget>> = vec![
            Box::new(Container::leaf(Style {
                size: Size {
                    width: length(200.0),
                    height: length(80.0),
                },
                ..Default::default()
            })),
            Box::new(Container::leaf(Style {
                size: Size {
                    width: length(140.0),
                    height: length(90.0),
                },
                ..Default::default()
            })),
            Box::new(Container::leaf(Style {
                size: Size {
                    width: length(120.0),
                    height: length(70.0),
                },
                ..Default::default()
            })),
        ];
        let root = Container::flow(360, 260, 8, children);
        let layout = compute_layout(
            &root,
            PixelSize {
                width: 360,
                height: 260,
            },
        )
        .expect("layout computes");

        assert_eq!(layout.root.children.len(), 3);
        let first = &layout.root.children[0].rect;
        let second = &layout.root.children[1].rect;
        let third = &layout.root.children[2].rect;

        assert_eq!(first.width, 200);
        assert_eq!(second.width, 140);
        assert_eq!(third.width, 120);
        assert_eq!(first.height, 80);
        assert_eq!(second.height, 90);
        assert_eq!(third.height, 70);

        assert_eq!(first.y, second.y);
        assert!(third.y > second.y);
    }
}
