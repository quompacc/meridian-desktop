//! The `Color` primitive (hex parse/serialize + helpers) now lives in the
//! shared `meridian-tokens` crate so the theme layer and the render layer
//! share one definition. Re-exported here so existing `theme::Color` /
//! `meridian_config::Color` paths keep resolving.

pub use meridian_tokens::Color;
