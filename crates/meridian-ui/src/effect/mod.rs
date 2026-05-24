//! Visual effect building blocks.
//!
//! `rounded_rect_path` is a setup/resize helper and may allocate internally.
//! Cache the resulting `Path` if it is reused across frames.
//! `paint_border` is render-loop code and must remain allocation-free.
//! `paint_text` rasterizes glyphs on demand and allocates per glyph — known
//! trade-off, see text.rs.

mod border;
mod dominant_color;
mod fill;
mod metro_surface;
mod radius;
mod text;

pub use border::paint_border;
pub use dominant_color::dominant_color;
pub use fill::paint_fill;
pub use metro_surface::paint_metro_surface;
pub use radius::rounded_rect_path;
pub use text::{measure_text, paint_text, truncate_to_fit, ui_font};
