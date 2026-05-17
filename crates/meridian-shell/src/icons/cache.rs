use std::collections::HashMap;

use super::{loader::IconLoader, IconImage};

enum CacheEntry {
    Found(IconImage),
    Missing,
}

pub struct IconCache {
    loader: IconLoader,
    entries: HashMap<u32, HashMap<String, CacheEntry>>,
}

impl IconCache {
    pub fn new() -> Self {
        Self {
            loader: IconLoader::new(super::lookup_default_theme()),
            entries: HashMap::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests(loader: IconLoader) -> Self {
        Self {
            loader,
            entries: HashMap::new(),
        }
    }

    pub fn warm(&mut self, names: &[&str], size: u32) {
        let entries_for_size = self.entries.entry(size).or_default();
        for name in names {
            if name.is_empty() {
                continue;
            }
            if entries_for_size.contains_key(*name) {
                continue;
            }

            let entry = match self.loader.load_icon(name, size) {
                Some(image) => CacheEntry::Found(image),
                None => CacheEntry::Missing,
            };
            entries_for_size.insert((*name).to_string(), entry);
        }
    }

    pub fn lookup(&self, name: &str, size: u32) -> Option<&IconImage> {
        let entries_for_size = self.entries.get(&size)?;
        match entries_for_size.get(name) {
            Some(CacheEntry::Found(image)) => Some(image),
            Some(CacheEntry::Missing) | None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use png::{BitDepth, ColorType, Encoder};

    use super::IconCache;
    use crate::icons::loader::IconLoader;

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(label: &str) -> Self {
            let mut path = std::env::temp_dir();
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            path.push(format!(
                "meridian-shell-cache-{label}-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_theme_index(path: &Path, directories: &[&str]) {
        let mut body = format!(
            "[Icon Theme]\nName=Theme\nInherits=\nDirectories={}\n\n",
            directories.join(",")
        );
        for directory in directories {
            let size: u32 = directory
                .split('x')
                .next()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0);
            body.push_str(&format!("[{directory}]\nType=Fixed\nSize={size}\n\n"));
        }
        fs::write(path, body).expect("write index.theme");
    }

    fn write_png(path: &Path, width: u32, height: u32, rgba: [u8; 4]) {
        let file = fs::File::create(path).expect("create png");
        let mut encoder = Encoder::new(file, width, height);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        let mut writer = encoder.write_header().expect("write header");
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..(width * height) {
            data.extend_from_slice(&rgba);
        }
        writer.write_image_data(&data).expect("write data");
    }

    fn create_loader_env() -> (TempDir, IconLoader) {
        let temp = TempDir::new("env");
        let icons_root = temp.path().join("icons");
        let theme_root = icons_root.join("Adwaita");
        let apps = theme_root.join("22x22/apps");

        fs::create_dir_all(&apps).expect("create apps dir");
        write_theme_index(&theme_root.join("index.theme"), &["22x22/apps"]);

        let loader = IconLoader::new_for_tests("Adwaita", vec![icons_root], vec![]);
        (temp, loader)
    }

    #[test]
    fn warm_is_idempotent_for_existing_key() {
        let (temp, loader) = create_loader_env();
        let png_path = temp
            .path()
            .join("icons/Adwaita/22x22/apps/utilities-terminal.png");
        write_png(&png_path, 22, 22, [255, 0, 0, 255]);

        let mut cache = IconCache::new_for_tests(loader);
        cache.warm(&["utilities-terminal"], 22);
        let first = cache
            .lookup("utilities-terminal", 22)
            .expect("cached icon")
            .bgra[0..4]
            .to_vec();

        write_png(&png_path, 22, 22, [0, 255, 0, 255]);
        cache.warm(&["utilities-terminal"], 22);
        let second = cache
            .lookup("utilities-terminal", 22)
            .expect("cached icon")
            .bgra[0..4]
            .to_vec();

        assert_eq!(first, second);
    }

    #[test]
    fn lookup_without_warm_returns_none() {
        let (_temp, loader) = create_loader_env();
        let cache = IconCache::new_for_tests(loader);
        assert!(cache.lookup("utilities-terminal", 22).is_none());
    }

    #[test]
    fn missing_icon_is_negative_cached_after_warm() {
        let (temp, loader) = create_loader_env();
        let mut cache = IconCache::new_for_tests(loader);

        cache.warm(&["firefox"], 22);
        assert!(cache.lookup("firefox", 22).is_none());

        let path = temp.path().join("icons/Adwaita/22x22/apps/firefox.png");
        write_png(&path, 22, 22, [255, 255, 255, 255]);

        assert!(cache.lookup("firefox", 22).is_none());
    }

    #[test]
    fn lookup_returns_stable_reference_without_clone() {
        let (temp, loader) = create_loader_env();
        let path = temp
            .path()
            .join("icons/Adwaita/22x22/apps/utilities-terminal.png");
        write_png(&path, 22, 22, [255, 0, 0, 255]);

        let mut cache = IconCache::new_for_tests(loader);
        cache.warm(&["utilities-terminal"], 22);

        let first = cache.lookup("utilities-terminal", 22).expect("first") as *const _;
        let second = cache.lookup("utilities-terminal", 22).expect("second") as *const _;
        assert_eq!(first, second);
    }
}
