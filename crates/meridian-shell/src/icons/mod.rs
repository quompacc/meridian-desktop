mod cache;
mod loader;
mod rcc;
pub mod svg;
mod theme_index;

pub use cache::IconCache;

#[cfg(test)]
pub(crate) use loader::IconLoader;

#[derive(Debug, Clone)]
pub struct IconImage {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

pub fn lookup_default_theme() -> &'static str {
    "breeze"
}

#[cfg(test)]
mod tests {
    use super::lookup_default_theme;

    #[test]
    fn default_theme_is_breeze() {
        assert_eq!(lookup_default_theme(), "breeze");
    }
}
