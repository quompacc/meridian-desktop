mod hit_test;

pub use hit_test::hit_test;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WidgetState {
    #[default]
    Idle,
    Hovered,
    Pressed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointerPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    PointerEnter {
        position: PointerPosition,
    },
    PointerLeave,
    PointerMove {
        position: PointerPosition,
    },
    PointerPress {
        position: PointerPosition,
        button: PointerButton,
    },
    PointerRelease {
        position: PointerPosition,
        button: PointerButton,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetPath {
    indices: Vec<usize>,
}

impl WidgetPath {
    pub fn empty() -> Self {
        Self {
            indices: Vec::new(),
        }
    }

    pub fn from_vec(indices: Vec<usize>) -> Self {
        Self { indices }
    }

    pub fn iter(&self) -> impl Iterator<Item = &usize> {
        self.indices.iter()
    }

    pub fn len(&self) -> usize {
        self.indices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn as_slice(&self) -> &[usize] {
        &self.indices
    }
}
