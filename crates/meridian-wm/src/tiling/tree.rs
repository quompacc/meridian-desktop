use smithay::{
    desktop::Window,
    utils::{Logical, Rectangle},
};

use super::types::SplitDir;

pub(super) enum Node {
    Leaf(Window),
    Internal {
        dir: SplitDir,
        ratio: f32,
        left: Box<Node>,
        right: Box<Node>,
    },
}

pub(super) fn collect_windows(node: &Node, out: &mut Vec<Window>) {
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
    match dir {
        SplitDir::Horizontal => {
            let split = ((rect.size.w as f32 * ratio) as i32).clamp(1, rect.size.w - 1);
            let left_w = (split - half).max(1);
            let right_x = rect.loc.x + split + half;
            let right_w = (rect.size.w - split - half).max(1);
            (
                Rectangle::new(rect.loc, (left_w, rect.size.h).into()),
                Rectangle::new((right_x, rect.loc.y).into(), (right_w, rect.size.h).into()),
            )
        }
        SplitDir::Vertical => {
            let split = ((rect.size.h as f32 * ratio) as i32).clamp(1, rect.size.h - 1);
            let top_h = (split - half).max(1);
            let bot_y = rect.loc.y + split + half;
            let bot_h = (rect.size.h - split - half).max(1);
            (
                Rectangle::new(rect.loc, (rect.size.w, top_h).into()),
                Rectangle::new((rect.loc.x, bot_y).into(), (rect.size.w, bot_h).into()),
            )
        }
    }
}

pub(super) fn collect_rects(
    node: &Node,
    rect: Rectangle<i32, Logical>,
    gap: i32,
    out: &mut Vec<(Window, Rectangle<i32, Logical>)>,
) {
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

pub(super) fn insert_next_to(
    node: &mut Node,
    focused: &Window,
    new_window: Window,
    dir: SplitDir,
) -> bool {
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

pub(super) fn insert_at_last(node: &mut Node, new_window: Window, dir: SplitDir) {
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

pub(super) fn remove_from_node(node: Box<Node>, window: &Window) -> (Option<Box<Node>>, bool) {
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

pub(super) fn adjust_split_node(
    node: &mut Node,
    window: &Window,
    split_dir: SplitDir,
    delta: f32,
) -> bool {
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
