use tiny_skia::PixmapMut;

use crate::{style::Theme, widget::Widget};

use super::{LayoutNode, LayoutTree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderError {
    LayoutTreeShapeMismatch,
}

/// Render a widget tree using a precomputed layout tree.
///
/// This function is on the render path and must not perform heap allocation.
pub fn render(
    root: &dyn Widget,
    layout: &LayoutTree,
    canvas: &mut PixmapMut<'_>,
    theme: &Theme,
) -> Result<(), RenderError> {
    render_node(root, &layout.root, canvas, theme)
}

fn render_node(
    widget: &dyn Widget,
    layout: &LayoutNode,
    canvas: &mut PixmapMut<'_>,
    theme: &Theme,
) -> Result<(), RenderError> {
    widget.paint(layout.rect, canvas, theme);

    let children = widget.children();
    if children.len() != layout.children.len() {
        return Err(RenderError::LayoutTreeShapeMismatch);
    }

    for (child_widget, child_layout) in children.iter().zip(layout.children.iter()) {
        render_node(child_widget.as_ref(), child_layout, canvas, theme)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use taffy::prelude::{length, Size, Style};
    use tiny_skia::Pixmap;

    use crate::{
        paint::{compute_layout, render, PixelSize},
        style::Theme,
        widget::Container,
    };

    #[test]
    fn render_smoke_does_not_crash() {
        let child = Box::new(Container::leaf(Style {
            size: Size {
                width: length(50.0),
                height: length(50.0),
            },
            ..Default::default()
        }));
        let root = Container::new(
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            vec![child],
        );
        let layout = compute_layout(
            &root,
            PixelSize {
                width: 100,
                height: 100,
            },
        )
        .expect("layout computes");
        let mut pixmap = Pixmap::new(100, 100).expect("pixmap");
        let mut canvas = pixmap.as_mut();

        render(&root, &layout, &mut canvas, &Theme::TOKYO_NIGHT_METRO).expect("render completes");
    }
}
