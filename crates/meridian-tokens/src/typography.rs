//! Shared desktop typography scale in logical pixels.
//!
//! Renderers may compose these roles, but must not invent local font sizes or
//! weights. Light and dark themes intentionally share the exact same scale.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Typography {
    pub caption_size: u16,
    pub body_size: u16,
    pub title_size: u16,
    pub display_size: u16,
    pub regular_weight: u16,
    pub medium_weight: u16,
    pub strong_weight: u16,
}

impl Typography {
    pub const DEFAULT: Typography = Typography {
        caption_size: 12,
        body_size: 14,
        title_size: 18,
        display_size: 24,
        regular_weight: 400,
        medium_weight: 500,
        strong_weight: 600,
    };
}

impl Default for Typography {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_and_weights_are_ordered() {
        let typography = Typography::DEFAULT;
        assert!(typography.caption_size < typography.body_size);
        assert!(typography.body_size < typography.title_size);
        assert!(typography.title_size < typography.display_size);
        assert!(typography.regular_weight < typography.medium_weight);
        assert!(typography.medium_weight < typography.strong_weight);
    }
}
