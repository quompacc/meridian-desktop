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

#[cfg(test)]
mod tests {
    use super::SplitDir;

    #[test]
    fn other_flips_horizontal_to_vertical() {
        assert_eq!(SplitDir::Horizontal.other(), SplitDir::Vertical);
    }

    #[test]
    fn other_flips_vertical_to_horizontal() {
        assert_eq!(SplitDir::Vertical.other(), SplitDir::Horizontal);
    }
}
