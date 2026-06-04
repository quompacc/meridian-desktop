#![deny(unsafe_code)]
//! Meridian design tokens — the single source of truth for the visual
//! language. Pure, `Copy`, dependency-light primitives that both the theme
//! deserialization layer (`meridian-config`) and the render layer
//! (`meridian-ui`) build on, so a value is defined once and never hand-synced.
//!
//! Phase 1 hosts the colour primitive and the canonical palette; phase 2 adds
//! the corner-radius scale; phase 3 adds interaction-state tokens. Later
//! phases move elevation/shadow, glass and typography here too (design-tokens
//! refactor plan).

pub mod color;
pub mod interaction;
pub mod radius;

pub use color::{Color, Palette};
pub use interaction::Interaction;
pub use radius::Radius;
