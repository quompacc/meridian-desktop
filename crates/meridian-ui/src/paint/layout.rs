use taffy::{
    prelude::{AvailableSpace, Size, TaffyTree},
    tree::NodeId,
    TaffyError,
};

use crate::{paint::PixelSize, widget::Widget};

use super::Rect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutNode {
    pub rect: Rect,
    pub children: Vec<LayoutNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutTree {
    pub root: LayoutNode,
}

struct PendingNode {
    id: NodeId,
    children: Vec<PendingNode>,
}

/// Compute full widget-tree layout using taffy.
///
/// This is a setup/resize phase operation and may allocate while building
/// the temporary taffy tree and resulting layout tree.
pub fn compute_layout(root: &dyn Widget, viewport: PixelSize) -> Result<LayoutTree, TaffyError> {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let pending_root = build_taffy_subtree(&mut tree, root)?;

    tree.compute_layout(
        pending_root.id,
        Size {
            width: AvailableSpace::Definite(viewport.width as f32),
            height: AvailableSpace::Definite(viewport.height as f32),
        },
    )?;

    Ok(LayoutTree {
        root: extract_layout_subtree(&tree, &pending_root),
    })
}

fn build_taffy_subtree(
    tree: &mut TaffyTree<()>,
    widget: &dyn Widget,
) -> Result<PendingNode, TaffyError> {
    let mut children = Vec::with_capacity(widget.children().len());
    let mut child_ids = Vec::with_capacity(widget.children().len());
    for child in widget.children() {
        let pending = build_taffy_subtree(tree, child.as_ref())?;
        child_ids.push(pending.id);
        children.push(pending);
    }

    let id = if child_ids.is_empty() {
        tree.new_leaf(widget.style())?
    } else {
        tree.new_with_children(widget.style(), &child_ids)?
    };

    Ok(PendingNode { id, children })
}

fn extract_layout_subtree(tree: &TaffyTree<()>, pending: &PendingNode) -> LayoutNode {
    let layout = tree
        .layout(pending.id)
        .expect("taffy layout must exist after successful compute_layout");
    let rect = Rect {
        x: layout.location.x.round() as i32,
        y: layout.location.y.round() as i32,
        width: layout.size.width.round() as i32,
        height: layout.size.height.round() as i32,
    };
    let children = pending
        .children
        .iter()
        .map(|child| extract_layout_subtree(tree, child))
        .collect();
    LayoutNode { rect, children }
}

#[cfg(test)]
mod tests {
    use taffy::prelude::{length, FlexDirection, Size, Style};

    use crate::{
        paint::{compute_layout, PixelSize},
        widget::Container,
    };

    #[test]
    fn computes_row_layout_for_two_fixed_children() {
        let child_a = Box::new(Container::leaf(Style {
            size: Size {
                width: length(50.0),
                height: length(50.0),
            },
            ..Default::default()
        }));
        let child_b = Box::new(Container::leaf(Style {
            size: Size {
                width: length(50.0),
                height: length(50.0),
            },
            ..Default::default()
        }));
        let root = Container::new(
            Style {
                flex_direction: FlexDirection::Row,
                size: Size {
                    width: length(200.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            vec![child_a, child_b],
        );

        let layout = compute_layout(
            &root,
            PixelSize {
                width: 200,
                height: 100,
            },
        )
        .expect("layout computes");
        assert_eq!(layout.root.children.len(), 2);
        let first = &layout.root.children[0].rect;
        let second = &layout.root.children[1].rect;

        assert_eq!(first.width, 50);
        assert_eq!(first.height, 50);
        assert_eq!(second.width, 50);
        assert_eq!(second.height, 50);
        assert!(second.x > first.x);
    }

    #[test]
    fn root_matches_requested_size() {
        let root = Container::leaf(Style {
            size: Size {
                width: length(200.0),
                height: length(100.0),
            },
            ..Default::default()
        });
        let layout = compute_layout(
            &root,
            PixelSize {
                width: 200,
                height: 100,
            },
        )
        .expect("layout computes");
        assert_eq!(layout.root.rect.width, 200);
        assert_eq!(layout.root.rect.height, 100);
    }
}
