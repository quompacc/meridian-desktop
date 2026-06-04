//! Corner-radius scale. The `Radius` token now lives in the shared
//! `meridian-tokens` crate; re-exported here under the historical
//! `style::Radius` path. The UI `Theme` bundle still defaults to
//! `Radius::METRO` (square); the shell rounds with `Radius::DEFAULT`.

pub use meridian_tokens::Radius;
