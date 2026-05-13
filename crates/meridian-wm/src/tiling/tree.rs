use smithay::utils::{Logical, Rectangle};

use super::types::SplitDir;

pub(super) enum Node<T> {
    Leaf(T),
    Internal {
        dir: SplitDir,
        ratio: f32,
        left: Box<Node<T>>,
        right: Box<Node<T>>,
    },
}

pub(super) fn collect_windows<T: Clone>(node: &Node<T>, out: &mut Vec<T>) {
    match node {
        Node::Leaf(window) => out.push(window.clone()),
        Node::Internal { left, right, .. } => {
            collect_windows(left, out);
            collect_windows(right, out);
        }
    }
}

fn split_rect(
    rect: Rectangle<i32, Logical>,
    dir: SplitDir,
    ratio: f32,
    gap: i32,
) -> (Rectangle<i32, Logical>, Rectangle<i32, Logical>) {
    let half = gap / 2;
    let width = rect.size.w.max(1);
    let height = rect.size.h.max(1);
    match dir {
        SplitDir::Horizontal => {
            let split = ((width as f32 * ratio) as i32).clamp(1, (width - 1).max(1));
            let left_w = (split - half).max(1);
            let right_x = rect.loc.x + split + half;
            let right_w = (width - split - half).max(1);
            (
                Rectangle::new(rect.loc, (left_w, height).into()),
                Rectangle::new((right_x, rect.loc.y).into(), (right_w, height).into()),
            )
        }
        SplitDir::Vertical => {
            let split = ((height as f32 * ratio) as i32).clamp(1, (height - 1).max(1));
            let top_h = (split - half).max(1);
            let bot_y = rect.loc.y + split + half;
            let bot_h = (height - split - half).max(1);
            (
                Rectangle::new(rect.loc, (width, top_h).into()),
                Rectangle::new((rect.loc.x, bot_y).into(), (width, bot_h).into()),
            )
        }
    }
}

pub(super) fn collect_rects<T>(
    node: &Node<T>,
    rect: Rectangle<i32, Logical>,
    gap: i32,
    out: &mut Vec<(T, Rectangle<i32, Logical>)>,
) where
    T: Clone,
{
    match node {
        Node::Leaf(window) => out.push((window.clone(), rect)),
        Node::Internal {
            dir,
            ratio,
            left,
            right,
        } => {
            let (left_rect, right_rect) = split_rect(rect, *dir, *ratio, gap);
            collect_rects(left, left_rect, gap, out);
            collect_rects(right, right_rect, gap, out);
        }
    }
}

pub(super) fn insert_next_to<T>(
    node: &mut Node<T>,
    focused: &T,
    new_window: T,
    dir: SplitDir,
) -> bool
where
    T: Clone + PartialEq,
{
    let is_target = matches!(node, Node::Leaf(window) if window == focused);
    if is_target {
        let existing = match node {
            Node::Leaf(window) => window.clone(),
            _ => unreachable!(),
        };
        *node = Node::Internal {
            dir,
            ratio: 0.5,
            left: Box::new(Node::Leaf(existing)),
            right: Box::new(Node::Leaf(new_window)),
        };
        return true;
    }

    match node {
        Node::Leaf(_) => false,
        Node::Internal { left, right, .. } => {
            insert_next_to(left, focused, new_window.clone(), dir)
                || insert_next_to(right, focused, new_window, dir)
        }
    }
}

pub(super) fn insert_at_last<T: Clone>(node: &mut Node<T>, new_window: T, dir: SplitDir) {
    match node {
        Node::Leaf(window) => {
            let existing = window.clone();
            *node = Node::Internal {
                dir,
                ratio: 0.5,
                left: Box::new(Node::Leaf(existing)),
                right: Box::new(Node::Leaf(new_window)),
            };
        }
        Node::Internal { right, .. } => insert_at_last(right, new_window, dir),
    }
}

pub(super) fn remove_from_node<T>(node: Box<Node<T>>, window: &T) -> (Option<Box<Node<T>>>, bool)
where
    T: PartialEq,
{
    if matches!(node.as_ref(), Node::Leaf(w) if w == window) {
        return (None, true);
    }

    let inner = *node;
    match inner {
        Node::Leaf(_) => (Some(Box::new(inner)), false),
        Node::Internal {
            dir,
            ratio,
            left,
            right,
        } => {
            let (new_left, removed) = remove_from_node(left, window);
            if removed {
                return match new_left {
                    None => (Some(right), true),
                    Some(left) => (
                        Some(Box::new(Node::Internal {
                            dir,
                            ratio,
                            left,
                            right,
                        })),
                        true,
                    ),
                };
            }
            let left = new_left.unwrap();

            let (new_right, removed) = remove_from_node(right, window);
            if removed {
                return match new_right {
                    None => (Some(left), true),
                    Some(right) => (
                        Some(Box::new(Node::Internal {
                            dir,
                            ratio,
                            left,
                            right,
                        })),
                        true,
                    ),
                };
            }
            let right = new_right.unwrap();

            (
                Some(Box::new(Node::Internal {
                    dir,
                    ratio,
                    left,
                    right,
                })),
                false,
            )
        }
    }
}

pub(super) fn adjust_split_node<T>(
    node: &mut Node<T>,
    window: &T,
    split_dir: SplitDir,
    delta: f32,
) -> bool
where
    T: PartialEq,
{
    match node {
        Node::Leaf(w) => w == window,
        Node::Internal {
            dir,
            ratio,
            left,
            right,
        } => {
            if adjust_split_node(left, window, split_dir, delta) {
                if *dir == split_dir {
                    *ratio = (*ratio + delta).clamp(0.1, 0.9);
                }
                true
            } else if adjust_split_node(right, window, split_dir, delta) {
                if *dir == split_dir {
                    *ratio = (*ratio - delta).clamp(0.1, 0.9);
                }
                true
            } else {
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use smithay::utils::Rectangle;

    use super::{
        collect_windows, insert_at_last, insert_next_to, remove_from_node, split_rect, Node,
        SplitDir,
    };

    fn assert_positive_sizes(
        left: Rectangle<i32, super::Logical>,
        right: Rectangle<i32, super::Logical>,
    ) {
        assert!(left.size.w >= 1);
        assert!(left.size.h >= 1);
        assert!(right.size.w >= 1);
        assert!(right.size.h >= 1);
    }

    #[test]
    fn split_rect_horizontal_applies_gap_and_ratio() {
        let rect = Rectangle::new((10, 20).into(), (100, 50).into());

        let (left, right) = split_rect(rect, SplitDir::Horizontal, 0.5, 4);

        assert_eq!(left.loc.x, 10);
        assert_eq!(left.loc.y, 20);
        assert_eq!(left.size.w, 48);
        assert_eq!(left.size.h, 50);
        assert_eq!(right.loc.x, 62);
        assert_eq!(right.loc.y, 20);
        assert_eq!(right.size.w, 48);
        assert_eq!(right.size.h, 50);
    }

    #[test]
    fn split_rect_vertical_applies_gap_and_ratio() {
        let rect = Rectangle::new((5, 7).into(), (60, 80).into());

        let (top, bottom) = split_rect(rect, SplitDir::Vertical, 0.5, 4);

        assert_eq!(top.loc.x, 5);
        assert_eq!(top.loc.y, 7);
        assert_eq!(top.size.w, 60);
        assert_eq!(top.size.h, 38);
        assert_eq!(bottom.loc.x, 5);
        assert_eq!(bottom.loc.y, 49);
        assert_eq!(bottom.size.w, 60);
        assert_eq!(bottom.size.h, 38);
    }

    #[test]
    fn split_rect_horizontal_clamps_extreme_ratios() {
        let rect = Rectangle::new((0, 0).into(), (100, 20).into());

        let (left_min, right_min) = split_rect(rect, SplitDir::Horizontal, -2.0, 4);
        assert_eq!(left_min.size.w, 1);
        assert_eq!(right_min.loc.x, 3);
        assert_eq!(right_min.size.w, 97);

        let (left_max, right_max) = split_rect(rect, SplitDir::Horizontal, 2.0, 4);
        assert_eq!(left_max.size.w, 97);
        assert_eq!(right_max.loc.x, 101);
        assert_eq!(right_max.size.w, 1);
    }

    #[test]
    fn split_rect_vertical_clamps_extreme_ratios() {
        let rect = Rectangle::new((0, 0).into(), (20, 100).into());

        let (top_min, bottom_min) = split_rect(rect, SplitDir::Vertical, -2.0, 4);
        assert_eq!(top_min.size.h, 1);
        assert_eq!(bottom_min.loc.y, 3);
        assert_eq!(bottom_min.size.h, 97);

        let (top_max, bottom_max) = split_rect(rect, SplitDir::Vertical, 2.0, 4);
        assert_eq!(top_max.size.h, 97);
        assert_eq!(bottom_max.loc.y, 101);
        assert_eq!(bottom_max.size.h, 1);
    }

    #[test]
    fn split_rect_horizontal_width_zero_keeps_sizes_positive() {
        let rect = Rectangle::new((0, 0).into(), (0, 10).into());
        let (left, right) = split_rect(rect, SplitDir::Horizontal, 0.5, 4);
        assert_positive_sizes(left, right);
    }

    #[test]
    fn split_rect_horizontal_width_one_keeps_sizes_positive() {
        let rect = Rectangle::new((0, 0).into(), (1, 10).into());
        let (left, right) = split_rect(rect, SplitDir::Horizontal, 0.5, 4);
        assert_positive_sizes(left, right);
    }

    #[test]
    fn split_rect_vertical_height_zero_keeps_sizes_positive() {
        let rect = Rectangle::new((0, 0).into(), (10, 0).into());
        let (top, bottom) = split_rect(rect, SplitDir::Vertical, 0.5, 4);
        assert_positive_sizes(top, bottom);
    }

    #[test]
    fn split_rect_vertical_height_one_keeps_sizes_positive() {
        let rect = Rectangle::new((0, 0).into(), (10, 1).into());
        let (top, bottom) = split_rect(rect, SplitDir::Vertical, 0.5, 4);
        assert_positive_sizes(top, bottom);
    }

    #[test]
    fn insert_at_last_keeps_deterministic_in_order() {
        let mut node = Node::Leaf(1_u32);
        insert_at_last(&mut node, 2, SplitDir::Horizontal);
        insert_at_last(&mut node, 3, SplitDir::Vertical);

        let mut out = Vec::new();
        collect_windows(&node, &mut out);
        assert_eq!(out, vec![1, 2, 3]);
    }

    #[test]
    fn insert_next_to_inserts_beside_focused_leaf() {
        let mut node = Node::Leaf(1_u32);
        insert_at_last(&mut node, 2, SplitDir::Horizontal);
        insert_at_last(&mut node, 3, SplitDir::Vertical);

        let inserted = insert_next_to(&mut node, &2, 9, SplitDir::Horizontal);
        assert!(inserted);

        let mut out = Vec::new();
        collect_windows(&node, &mut out);
        assert_eq!(out, vec![1, 2, 9, 3]);
    }

    #[test]
    fn remove_from_node_collapses_parent_when_child_removed() {
        let node = Box::new(Node::Internal {
            dir: SplitDir::Horizontal,
            ratio: 0.5,
            left: Box::new(Node::Leaf(1_u32)),
            right: Box::new(Node::Leaf(2_u32)),
        });

        let (new_root, removed) = remove_from_node(node, &1);
        assert!(removed);
        let Some(new_root) = new_root else {
            panic!("expected collapsed surviving child");
        };
        match *new_root {
            Node::Leaf(id) => assert_eq!(id, 2),
            Node::Internal { .. } => panic!("expected collapse to surviving leaf"),
        }
    }
}
