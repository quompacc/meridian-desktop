//! Corner-radius scale, in logical pixels.
//!
//! `DEFAULT` is the live scale the shell rounds with; `METRO` is the
//! all-square variant kept for the `meridian-ui` `Theme` bundle (whose
//! `radius` field no consumer reads for rounding yet — see the design-tokens
//! audit 2026-06-04). Both are `Copy` so they pass through the render loop by
//! value.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Radius {
    pub none: i32,
    pub sm: i32,
    pub md: i32,
    pub lg: i32,
    pub xl: i32,
}

impl Radius {
    /// The live rounding scale. The shell's element radii derive from these
    /// steps, so changing a corner radius is a one-line edit here.
    ///
    /// Step assignments (Phase 2, mapped 1:1 from the former scattered
    /// constants so the rendered image is unchanged):
    /// - `sm` (6):  small tiles / generic roundish rects, workspace tiles
    /// - `md` (8):  panel chip highlight, launcher tiles
    /// - `lg` (12): panel island
    /// - `xl` (14): popup cards
    pub const DEFAULT: Radius = Radius {
        none: 0,
        sm: 6,
        md: 8,
        lg: 12,
        xl: 14,
    };

    /// All-square scale (Metro). Retained for the UI `Theme` default bundle.
    pub const METRO: Radius = Radius {
        none: 0,
        sm: 0,
        md: 0,
        lg: 0,
        xl: 0,
    };
}

impl Default for Radius {
    fn default() -> Self {
        Self::DEFAULT
    }
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
        assert_eq!(r.xl, 0);
    }

    #[test]
    fn default_scale_matches_phase1_values() {
        // Guards the 1:1 mapping from the former shell constants. Changing
        // these intentionally is fine — but it WILL move the rendered radii,
        // so update with eyes open.
        let r = Radius::DEFAULT;
        assert_eq!(r.sm, 6);
        assert_eq!(r.md, 8);
        assert_eq!(r.lg, 12);
        assert_eq!(r.xl, 14);
    }

    #[test]
    fn scales_are_non_decreasing() {
        for r in [Radius::DEFAULT, Radius::METRO] {
            assert!(r.none <= r.sm);
            assert!(r.sm <= r.md);
            assert!(r.md <= r.lg);
            assert!(r.lg <= r.xl);
        }
    }
}
