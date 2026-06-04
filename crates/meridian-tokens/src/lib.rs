#![deny(unsafe_code)]
//! Meridian design tokens — the single source of truth for the visual
//! language. Pure, `Copy`, dependency-light primitives that both the theme
//! deserialization layer (`meridian-config`) and the render layer
//! (`meridian-ui`) build on, so a value is defined once and never hand-synced.
//!
//! Phase 1 hosts the colour primitive and the canonical palette. Later phases
//! move the radius scale, elevation/shadow, interaction states, glass and
//! typography tokens here too (see the design-tokens refactor plan).

pub mod color;

pub use color::{Color, Palette};
