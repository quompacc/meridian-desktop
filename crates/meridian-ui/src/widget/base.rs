use taffy::prelude::Style;
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
}
