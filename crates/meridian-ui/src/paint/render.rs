use tiny_skia::PixmapMut;

use crate::{paint::Rect, style::Theme, widget::Widget};

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
    render_node(root, &layout.root, 0, 0, canvas, theme)
}

fn render_node(
    widget: &dyn Widget,
    layout: &LayoutNode,
    parent_x: i32,
    parent_y: i32,
    canvas: &mut PixmapMut<'_>,
    theme: &Theme,
) -> Result<(), RenderError> {
    let absolute = Rect {
        x: parent_x + layout.rect.x,
        y: parent_y + layout.rect.y,
        width: layout.rect.width,
        height: layout.rect.height,
    };
    widget.paint(absolute, canvas, theme);

    let children = widget.children();
    if children.len() != layout.children.len() {
        return Err(RenderError::LayoutTreeShapeMismatch);
    }

    for (child_widget, child_layout) in children.iter().zip(layout.children.iter()) {
        render_node(
            child_widget.as_ref(),
            child_layout,
            absolute.x,
            absolute.y,
            canvas,
            theme,
        )?;
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

    #[test]
    fn render_accumulates_offset_across_nested_containers() {
        use crate::{style::Palette, widget::Button, widget::Widget};

        // Outer column stacks two children vertically: a 100x40 spacer and a
        // 100x40 button. Button must paint at y=40, not y=0 — that requires
        // the recursive render to add the parent container's offset.
        let spacer = Box::new(Container::leaf(Style {
            size: Size {
                width: length(100.0),
                height: length(40.0),
            },
            ..Default::default()
        }));
        let button = Box::new(Button::new("p", Palette::TOKYO_NIGHT_METRO.accent, 100, 40))
            as Box<dyn Widget>;
        let root = Container::column(0, vec![spacer, button]);

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
        drop(canvas);

        // Top half (y < 40) was just the spacer — Container paint is a no-op,
        // so those pixels stay at the initial transparent state.
        let top_px = pixmap.pixel(50, 5).expect("top pixel");
        assert_eq!(top_px.alpha(), 0, "spacer area must remain unpainted");

        // Bottom half (y >= 40) is the button — stripe at y=40, body below.
        // Stripe carries accent color; body carries surface. Both opaque.
        let stripe_px = pixmap.pixel(50, 41).expect("stripe pixel");
        let body_px = pixmap.pixel(50, 60).expect("body pixel");
        assert!(stripe_px.alpha() > 0, "stripe must be painted at y=40+");
        assert!(body_px.alpha() > 0, "body must be painted at y=40+");
    }
}
