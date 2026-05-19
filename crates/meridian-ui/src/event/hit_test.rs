use crate::paint::LayoutNode;

use super::{PointerPosition, WidgetPath};

pub fn hit_test(
    layout: &crate::paint::LayoutTree,
    position: PointerPosition,
) -> Option<WidgetPath> {
    hit_test_node(&layout.root, 0, 0, position).map(WidgetPath::from_vec)
}

fn hit_test_node(
    node: &LayoutNode,
    parent_x: i32,
    parent_y: i32,
    position: PointerPosition,
) -> Option<Vec<usize>> {
    let abs_x = parent_x + node.rect.x;
    let abs_y = parent_y + node.rect.y;

    if position.x < abs_x
        || position.y < abs_y
        || position.x >= abs_x + node.rect.width
        || position.y >= abs_y + node.rect.height
    {
        return None;
    }

    for (i, child) in node.children.iter().enumerate() {
        if let Some(mut child_path) = hit_test_node(child, abs_x, abs_y, position) {
            child_path.insert(0, i);
            return Some(child_path);
        }
    }

    Some(Vec::new())
}

#[cfg(test)]
mod tests {
    use taffy::prelude::{length, Size, Style};

    use crate::{
        paint::{compute_layout, PixelSize},
        widget::Container,
    };

    use super::{hit_test, PointerPosition, WidgetPath};

    #[test]
    fn hit_test_returns_none_outside_root() {
        let root = Container::leaf(Style {
            size: Size {
                width: length(100.0),
                height: length(100.0),
            },
            ..Default::default()
        });
        let layout = compute_layout(
            &root,
            PixelSize {
                width: 100,
                height: 100,
            },
        )
        .expect("layout computes");

        assert_eq!(hit_test(&layout, PointerPosition { x: -1, y: 0 }), None);
        assert_eq!(hit_test(&layout, PointerPosition { x: 0, y: -1 }), None);
        assert_eq!(hit_test(&layout, PointerPosition { x: 100, y: 50 }), None);
        assert_eq!(hit_test(&layout, PointerPosition { x: 50, y: 100 }), None);
    }

    #[test]
    fn hit_test_returns_empty_path_for_root_only_hit() {
        let root = Container::leaf(Style {
            size: Size {
                width: length(100.0),
                height: length(100.0),
            },
            ..Default::default()
        });
        let layout = compute_layout(
            &root,
            PixelSize {
                width: 100,
                height: 100,
            },
        )
        .expect("layout computes");

        let path = hit_test(&layout, PointerPosition { x: 50, y: 50 }).expect("hit inside root");
        assert!(path.is_empty());
        assert_eq!(path.len(), 0);
        assert!(path.as_slice().is_empty());
    }

    #[test]
    fn hit_test_picks_correct_sibling() {
        let left = Box::new(Container::leaf(Style {
            size: Size {
                width: length(50.0),
                height: length(50.0),
            },
            ..Default::default()
        }));
        let right = Box::new(Container::leaf(Style {
            size: Size {
                width: length(50.0),
                height: length(50.0),
            },
            ..Default::default()
        }));
        let root = Container::new(
            Style {
                size: Size {
                    width: length(200.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            vec![left, right],
        );
        let layout = compute_layout(
            &root,
            PixelSize {
                width: 200,
                height: 100,
            },
        )
        .expect("layout computes");

        let left_path =
            hit_test(&layout, PointerPosition { x: 10, y: 25 }).expect("hit left child");
        assert_eq!(left_path.as_slice(), &[0]);

        let right_path =
            hit_test(&layout, PointerPosition { x: 60, y: 25 }).expect("hit right child");
        assert_eq!(right_path.as_slice(), &[1]);
    }

    #[test]
    fn hit_test_picks_deepest_child() {
        let deep = Box::new(Container::leaf(Style {
            size: Size {
                width: length(50.0),
                height: length(50.0),
            },
            ..Default::default()
        }));
        let inner = Box::new(Container::new(
            Style {
                size: Size {
                    width: length(100.0),
                    height: length(100.0),
                },
                ..Default::default()
            },
            vec![deep],
        ));
        let root = Container::new(
            Style {
                size: Size {
                    width: length(200.0),
                    height: length(200.0),
                },
                ..Default::default()
            },
            vec![inner],
        );
        let layout = compute_layout(
            &root,
            PixelSize {
                width: 200,
                height: 200,
            },
        )
        .expect("layout computes");

        // inner at (0,0) 100x100, deep at (0,0) 50x50
        let deep_path =
            hit_test(&layout, PointerPosition { x: 25, y: 25 }).expect("hit deep child");
        assert_eq!(deep_path.as_slice(), &[0, 0]);

        let inner_edge =
            hit_test(&layout, PointerPosition { x: 60, y: 25 }).expect("hit inner edge");
        assert_eq!(inner_edge.as_slice(), &[0]);
    }

    #[test]
    fn hit_test_accumulates_parent_offsets() {
        // Outer: 200x200 with padding (10,10,0,0)
        // Inner: 80x80 placed inside root content area, shifted by padding to (10,10)
        // Deep: 40x40 leaf child placed inside inner at (0,0)
        // Absolute coords: root(0,0), inner(10,10) to (90,90), deep(10,10) to (50,50)
        let deep = Box::new(Container::leaf(Style {
            size: Size {
                width: length(40.0),
                height: length(40.0),
            },
            ..Default::default()
        }));
        let inner = Box::new(Container::new(
            Style {
                size: Size {
                    width: length(80.0),
                    height: length(80.0),
                },
                ..Default::default()
            },
            vec![deep],
        ));
        let root = Container::new(
            Style {
                size: Size {
                    width: length(200.0),
                    height: length(200.0),
                },
                padding: taffy::prelude::Rect {
                    left: length(10.0),
                    top: length(10.0),
                    right: length(0.0),
                    bottom: length(0.0),
                },
                ..Default::default()
            },
            vec![inner],
        );
        let layout = compute_layout(
            &root,
            PixelSize {
                width: 200,
                height: 200,
            },
        )
        .expect("layout computes");

        // Hit padding area — should hit root only
        let hit_root = hit_test(&layout, PointerPosition { x: 5, y: 5 });
        assert!(hit_root.is_some(), "padding area is inside root");
        assert!(
            hit_root.unwrap().is_empty(),
            "only root hit in padding area"
        );

        // Hit deep at abs (15,15) — deep spans (10,10)-(50,50)
        let hit_deep = hit_test(&layout, PointerPosition { x: 15, y: 15 });
        assert!(hit_deep.is_some(), "absolute (15,15) should be inside deep");
        assert_eq!(
            hit_deep.unwrap().as_slice(),
            &[0, 0],
            "deep node path at absolute (15,15)"
        );

        // Hit inner at abs (60,25) — inner spans (10,10)-(90,90), deep is (10,10)-(50,50)
        let hit_inner = hit_test(&layout, PointerPosition { x: 60, y: 25 });
        assert!(
            hit_inner.is_some(),
            "absolute (60,25) should hit inner (not deep)"
        );
        assert_eq!(
            hit_inner.unwrap().as_slice(),
            &[0],
            "inner node path at absolute (60,25)"
        );
    }

    #[test]
    fn widget_path_iter_yields_indices_in_order() {
        let path = WidgetPath::from_vec(vec![2, 1, 0]);
        let collected: Vec<&usize> = path.iter().collect();
        assert_eq!(collected, vec![&2, &1, &0]);
    }

    #[test]
    fn widget_path_empty_is_empty() {
        let path = WidgetPath::empty();
        assert!(path.is_empty());
        assert_eq!(path.len(), 0);

        let with_indices = WidgetPath::from_vec(vec![0]);
        assert!(!with_indices.is_empty());
        assert_eq!(with_indices.len(), 1);
    }

    #[test]
    fn pointer_button_eq_per_variant() {
        use crate::event::PointerButton;
        assert_eq!(PointerButton::Left, PointerButton::Left);
        assert_eq!(PointerButton::Right, PointerButton::Right);
        assert_eq!(PointerButton::Middle, PointerButton::Middle);
        assert_ne!(PointerButton::Left, PointerButton::Right);
        assert_ne!(PointerButton::Left, PointerButton::Middle);
        assert_ne!(PointerButton::Right, PointerButton::Middle);
    }

    #[test]
    fn event_pointer_press_carries_position_and_button() {
        use crate::event::{Event, PointerButton, PointerPosition};

        let pos = PointerPosition { x: 42, y: 17 };
        let ev = Event::PointerPress {
            position: pos,
            button: PointerButton::Right,
        };

        match ev {
            Event::PointerPress { position, button } => {
                assert_eq!(position, pos);
                assert_eq!(button, PointerButton::Right);
            }
            _ => panic!("expected PointerPress"),
        }
    }
}
