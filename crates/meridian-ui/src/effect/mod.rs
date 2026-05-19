//! Visual effect building blocks.
//!
//! `rounded_rect_path` is a setup/resize helper and may allocate internally.
//! Cache the resulting `Path` if it is reused across frames.
//! `paint_border` is render-loop code and must remain allocation-free.

mod border;
mod radius;

pub use border::paint_border;
pub use radius::rounded_rect_path;
