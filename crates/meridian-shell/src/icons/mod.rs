mod cache;
mod loader;
pub mod svg;
mod theme_index;

pub use cache::IconCache;

#[derive(Debug, Clone)]
pub struct IconImage {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

pub fn lookup_default_theme() -> &'static str {
    "Adwaita"
}

#[cfg(test)]
mod tests {
    use super::lookup_default_theme;

    #[test]
    fn default_theme_is_adwaita() {
        assert_eq!(lookup_default_theme(), "Adwaita");
    }
}
