#![deny(unsafe_code)]
//! Meridian design tokens — the single source of truth for the visual
//! language. Pure, `Copy`, dependency-light primitives that both the theme
//! deserialization layer (`meridian-config`) and the render layer
//! (`meridian-ui`) build on, so a value is defined once and never hand-synced.
//!
//! Phase 1 hosts the colour primitive and the canonical palette; phase 2 the
//! corner-radius scale; phase 3 interaction-state tokens; phase 4 the
//! per-surface elevation (drop-shadow) scale; phase 6 the shared embedded UI
//! font. Later phases move typography sizing here too (refactor plan).

pub mod chrome;
pub mod color;
pub mod elevation;
pub mod font;
pub mod interaction;
pub mod radius;
pub mod typography;

pub use chrome::{Launcher, Mask, Panel, QuickSettings, Scrollbar, WindowChrome};
pub use color::{contrast_text, relative_luminance, Color, Palette};
pub use elevation::Elevation;
pub use interaction::Interaction;
pub use radius::Radius;
pub use typography::Typography;
