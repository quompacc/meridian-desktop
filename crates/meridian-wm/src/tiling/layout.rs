use smithay::{
    desktop::Window,
    utils::{Logical, Rectangle},
};

use super::{
    tree::{
        adjust_split_node, collect_rects, collect_windows, insert_at_last, insert_next_to,
        remove_from_node, Node,
    },
    SplitDir,
};

pub struct TilingLayout {
    root: Option<Box<Node<Window>>>,
    pub next_split: SplitDir,
}

impl TilingLayout {
    pub fn new() -> Self {
        Self {
            root: None,
            next_split: SplitDir::Horizontal,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub fn windows(&self) -> Vec<Window> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            collect_windows(root, &mut out);
        }
        out
    }

    pub fn add(&mut self, window: Window, focused: Option<&Window>) {
        let dir = self.next_split;
        match &mut self.root {
            None => {
                self.root = Some(Box::new(Node::Leaf(window)));
                return;
            }
            Some(root) => {
                let inserted = focused
                    .is_some_and(|focused| insert_next_to(root, focused, window.clone(), dir));
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

    pub fn adjust_split(&mut self, window: &Window, dir: SplitDir, delta: f32) -> bool {
        match &mut self.root {
            None => false,
            Some(root) => adjust_split_node(root, window, dir, delta),
        }
    }
}

impl Default for TilingLayout {
    fn default() -> Self {
        Self::new()
    }
}
