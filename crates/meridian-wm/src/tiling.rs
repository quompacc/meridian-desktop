use smithay::{
    desktop::Window,
    utils::{Logical, Rectangle},
};

// ── Split direction ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitDir {
    Horizontal,
    Vertical,
}

impl SplitDir {
    pub fn other(self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }
}

// ── BSP Node ──────────────────────────────────────────────────────────────────

enum Node {
    Leaf(Window),
    Internal {
        dir:   SplitDir,
        ratio: f32, // fraction of space given to `left`
        left:  Box<Node>,
        right: Box<Node>,
    },
}

// ── Recursive helpers ─────────────────────────────────────────────────────────

fn collect_windows(node: &Node, out: &mut Vec<Window>) {
    match node {
        Node::Leaf(w) => out.push(w.clone()),
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
            let left_w  = (split - half).max(1);
            let right_x = rect.loc.x + split + half;
            let right_w = (rect.size.w - split - half).max(1);
            (
                Rectangle::new(rect.loc, (left_w, rect.size.h).into()),
                Rectangle::new((right_x, rect.loc.y).into(), (right_w, rect.size.h).into()),
            )
        }
        SplitDir::Vertical => {
            let split = ((rect.size.h as f32 * ratio) as i32).clamp(1, rect.size.h - 1);
            let top_h  = (split - half).max(1);
            let bot_y  = rect.loc.y + split + half;
            let bot_h  = (rect.size.h - split - half).max(1);
            (
                Rectangle::new(rect.loc, (rect.size.w, top_h).into()),
                Rectangle::new((rect.loc.x, bot_y).into(), (rect.size.w, bot_h).into()),
            )
        }
    }
}

fn collect_rects(
    node: &Node,
    rect: Rectangle<i32, Logical>,
    gap: i32,
    out: &mut Vec<(Window, Rectangle<i32, Logical>)>,
) {
    match node {
        Node::Leaf(w) => out.push((w.clone(), rect)),
        Node::Internal { dir, ratio, left, right } => {
            let (lr, rr) = split_rect(rect, *dir, *ratio, gap);
            collect_rects(left, lr, gap, out);
            collect_rects(right, rr, gap, out);
        }
    }
}

// Split the leaf matching `focused` and insert `new_window` as right child.
// Returns true if the focused window was found.
fn insert_next_to(
    node: &mut Node,
    focused: &Window,
    new_window: Window,
    dir: SplitDir,
) -> bool {
    let is_target = matches!(node, Node::Leaf(w) if w == focused);
    if is_target {
        let existing = match node { Node::Leaf(w) => w.clone(), _ => unreachable!() };
        *node = Node::Internal {
            dir,
            ratio: 0.5,
            left:  Box::new(Node::Leaf(existing)),
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

// Append to the rightmost leaf when no focused window is available.
fn insert_at_last(node: &mut Node, new_window: Window, dir: SplitDir) {
    match node {
        Node::Leaf(w) => {
            let existing = w.clone();
            *node = Node::Internal {
                dir,
                ratio: 0.5,
                left:  Box::new(Node::Leaf(existing)),
                right: Box::new(Node::Leaf(new_window)),
            };
        }
        Node::Internal { right, .. } => insert_at_last(right, new_window, dir),
    }
}

// Remove `window` from the subtree.  Returns (new_root, was_found).
fn remove_from_node(node: Box<Node>, window: &Window) -> (Option<Box<Node>>, bool) {
    if matches!(node.as_ref(), Node::Leaf(w) if w == window) {
        return (None, true);
    }
    let inner = *node; // move out of Box
    match inner {
        Node::Leaf(_) => (Some(Box::new(inner)), false),
        Node::Internal { dir, ratio, left, right } => {
            let (new_left, removed) = remove_from_node(left, window);
            if removed {
                return match new_left {
                    None    => (Some(right), true),
                    Some(l) => (Some(Box::new(Node::Internal { dir, ratio, left: l, right })), true),
                };
            }
            let left = new_left.unwrap();
            let (new_right, removed) = remove_from_node(right, window);
            if removed {
                return match new_right {
                    None    => (Some(left), true),
                    Some(r) => (Some(Box::new(Node::Internal { dir, ratio, left, right: r })), true),
                };
            }
            let right = new_right.unwrap();
            (Some(Box::new(Node::Internal { dir, ratio, left, right })), false)
        }
    }
}

// Adjust the ratio of the split node that directly contains `window`.
// `delta` > 0 means "grow window in the positive axis direction".
// Returns true if the window was found.
fn adjust_split_node(
    node: &mut Node,
    window: &Window,
    split_dir: SplitDir,
    delta: f32,
) -> bool {
    match node {
        Node::Leaf(w) => w == window,
        Node::Internal { dir, ratio, left, right } => {
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

// ── Public API ────────────────────────────────────────────────────────────────

pub struct TilingLayout {
    root:           Option<Box<Node>>,
    pub next_split: SplitDir,
}

impl TilingLayout {
    pub fn new() -> Self {
        Self { root: None, next_split: SplitDir::Horizontal }
    }

    pub fn is_empty(&self) -> bool { self.root.is_none() }

    pub fn windows(&self) -> Vec<Window> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            collect_windows(root, &mut out);
        }
        out
    }

    /// Insert a new window, splitting next to `focused` when possible.
    pub fn add(&mut self, window: Window, focused: Option<&Window>) {
        let dir = self.next_split;
        match &mut self.root {
            None => {
                self.root = Some(Box::new(Node::Leaf(window)));
                return;
            }
            Some(root) => {
                let inserted = focused
                    .map_or(false, |f| insert_next_to(root, f, window.clone(), dir));
                if !inserted {
                    insert_at_last(root, window, dir);
                }
            }
        }
        self.next_split = self.next_split.other();
    }

    pub fn remove(&mut self, window: &Window) {
        if let Some(root) = self.root.take() {
            let (new_root, _) = remove_from_node(root, window);
            self.root = new_root;
        }
    }

    /// Compute tile rectangles for all leaves.
    /// `gap` controls inter-tile spacing (half applied at screen edges).
    pub fn compute_rects(
        &self,
        screen: Rectangle<i32, Logical>,
        gap: i32,
    ) -> Vec<(Window, Rectangle<i32, Logical>)> {
        let half = gap / 2;
        let inner = Rectangle::new(
            (screen.loc.x + half, screen.loc.y + half).into(),
            ((screen.size.w - gap).max(1), (screen.size.h - gap).max(1)).into(),
        );
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            collect_rects(root, inner, gap, &mut out);
        }
        out
    }

    /// Nudge the split ratio of the node that directly contains `window`.
    /// `delta` positive = window grows in the positive direction of `dir`.
    pub fn adjust_split(&mut self, window: &Window, dir: SplitDir, delta: f32) -> bool {
        match &mut self.root {
            None => false,
            Some(root) => adjust_split_node(root, window, dir, delta),
        }
    }
}

impl Default for TilingLayout {
    fn default() -> Self { Self::new() }
}
