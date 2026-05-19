//! Widget abstractions for Meridian UI.
//!
//! Contract:
//! - `paint` must stay allocation-free and side-effect free (`&self` only).
//! - `children` exposes a prebuilt tree assembled in setup/build phases.
//!   Heap allocation is allowed while building that tree, not while rendering.

pub mod base;
pub mod tile;

pub use base::{Container, Widget};
pub use tile::Tile;
