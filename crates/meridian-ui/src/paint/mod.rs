//! Painting and layout bridge.
//!
//! `compute_layout` runs in setup/resize phases and may allocate.
//! `render` runs in the frame path and must not allocate heap memory.

mod layout;
mod render;

pub use layout::{compute_layout, LayoutNode, LayoutTree};
pub use render::{render, render_idle, RenderError};

/// Pixel-space rectangle used during painting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Pixel-space viewport size used as root layout constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelSize {
    pub width: u32,
    pub height: u32,
}
