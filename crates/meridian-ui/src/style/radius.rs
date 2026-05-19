//! Corner-radius scale. Metro defaults to zero across the board; the scale
//! is kept so widgets can later opt in to rounding without a token migration.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Radius {
    pub none: i32,
    pub sm: i32,
    pub md: i32,
    pub lg: i32,
}

impl Radius {
    pub const METRO: Radius = Radius {
        none: 0,
        sm: 0,
        md: 0,
        lg: 0,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metro_radius_is_all_zero() {
        let r = Radius::METRO;
        assert_eq!(r.none, 0);
        assert_eq!(r.sm, 0);
        assert_eq!(r.md, 0);
        assert_eq!(r.lg, 0);
    }

    #[test]
    fn metro_radius_is_non_decreasing() {
        let r = Radius::METRO;
        assert!(r.none <= r.sm);
        assert!(r.sm <= r.md);
        assert!(r.md <= r.lg);
    }
}
