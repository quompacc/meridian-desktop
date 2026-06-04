//! The embedded UI typeface, owned here once so every desktop component
//! (shell, ui widgets, lock screen, polkit agent) references a single copy
//! instead of embedding its own. See the design-tokens audit (2026-06-04).
//!
//! Login and the boot splash deliberately use their own brand fonts
//! (DejaVu + Italianno, via meridian-compass-render) and are not affected.

/// Adwaita Sans Regular — the Meridian desktop UI typeface.
pub const ADWAITA_SANS_REGULAR: &[u8] = include_bytes!("../assets/fonts/AdwaitaSans-Regular.ttf");
