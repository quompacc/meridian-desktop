//! Spacing scale tokens, in logical pixels.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spacing {
    pub xs: i32,
    pub sm: i32,
    pub md: i32,
    pub lg: i32,
    pub xl: i32,
}

impl Spacing {
    pub const DEFAULT: Spacing = Spacing {
        xs: 4,
        sm: 6,
        md: 8,
        lg: 10,
        xl: 12,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_spacing_is_strictly_monotonic() {
        let s = Spacing::DEFAULT;
        assert!(s.xs < s.sm);
        assert!(s.sm < s.md);
        assert!(s.md < s.lg);
        assert!(s.lg < s.xl);
    }

    #[test]
    fn default_spacing_is_positive() {
        let s = Spacing::DEFAULT;
        assert!(s.xs > 0);
    }
}
