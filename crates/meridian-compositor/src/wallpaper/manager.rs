use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::PathBuf,
};

use image::RgbaImage;
use meridian_config::{Theme, WallpaperMode};
use tracing::{info, warn};

use super::compose;

pub struct WallpaperManager {
    path: Option<PathBuf>,
    pub mode: WallpaperMode,
    image: Option<RgbaImage>,
}

impl WallpaperManager {
    pub fn new() -> Self {
        Self {
            path: None,
            mode: WallpaperMode::Fill,
            image: None,
        }
    }

    pub fn apply_theme(&mut self, theme: &Theme) {
        let new_path = theme.wallpaper_path();
        let new_mode = theme
            .config
            .wallpaper
            .as_ref()
            .map(|wallpaper| wallpaper.mode)
            .unwrap_or_default();
        if new_path != self.path || new_mode != self.mode {
            self.path = new_path;
            self.mode = new_mode;
            self.image = None;
        }
    }

    fn ensure_loaded(&mut self) {
        if self.image.is_some() {
            return;
        }
        let dyn_img = match &self.path {
            Some(path) => match image::open(path) {
                Ok(img) => {
                    info!("Wallpaper loaded from {:?}", path);
                    img
                }
                Err(err) => {
                    warn!(
                        "Cannot open wallpaper {:?}: {err} — using built-in default",
                        path
                    );
                    compose::load_default_image()
                }
            },
            None => compose::load_default_image(),
        };
        self.image = Some(dyn_img.into_rgba8());
    }

    pub fn compose_for_size(&mut self, out_w: u32, out_h: u32) -> Vec<u8> {
        self.ensure_loaded();
        compose::compose_for_size(self.image.as_ref(), self.mode, out_w, out_h)
    }

    pub fn source_key(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.path.hash(&mut hasher);
        (self.mode as u8).hash(&mut hasher);
        hasher.finish()
    }
}

impl Default for WallpaperManager {
    fn default() -> Self {
        Self::new()
    }
}
