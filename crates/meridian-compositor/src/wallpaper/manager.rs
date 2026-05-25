use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::PathBuf,
};

use image::RgbaImage;
use meridian_compass_render::{CompassPainter, Fonts, FrameOpts, Style};
use meridian_config::{Theme, WallpaperMode};
use tiny_skia::Pixmap;
use tracing::{info, warn};

use super::compose;

/// Transient login->desktop intro: the compass is rendered live and zooms out
/// from the login end-size down to the wallpaper size, after which we fall back
/// to the static wallpaper image. It lives in the wallpaper manager so it rides
/// the existing wallpaper texture upload path instead of needing its own layer.
struct Intro {
    painter: CompassPainter<'static>,
    radius_factor: f32,
    frame: u64,
    pixmap: Option<Pixmap>,
}

pub struct WallpaperManager {
    path: Option<PathBuf>,
    pub mode: WallpaperMode,
    image: Option<RgbaImage>,
    intro: Option<Intro>,
}

impl WallpaperManager {
    pub fn new() -> Self {
        Self {
            path: None,
            mode: WallpaperMode::Fill,
            image: None,
            intro: None,
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

    /// Arm the zoom-out intro with the given compass style (midnight or chart).
    /// `start_radius` is the login end-size; the caller drives it down to the
    /// wallpaper size via [`set_intro_radius`]. No-op if the font build fails.
    pub fn begin_intro(&mut self, style: Style, start_radius: f32) {
        match CompassPainter::new(Fonts::quompacc()) {
            Ok(painter) => {
                self.intro = Some(Intro {
                    painter: painter.with_style(style),
                    radius_factor: start_radius,
                    frame: 0,
                    pixmap: None,
                });
            }
            Err(err) => warn!("compass intro disabled: {err}"),
        }
    }

    pub fn intro_active(&self) -> bool {
        self.intro.is_some()
    }

    pub fn set_intro_radius(&mut self, radius_factor: f32) {
        if let Some(intro) = self.intro.as_mut() {
            intro.radius_factor = radius_factor;
            intro.frame = intro.frame.wrapping_add(1);
        }
    }

    /// End the intro and fall back to the static wallpaper image.
    pub fn end_intro(&mut self) {
        self.intro = None;
        self.image = None;
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
        if let Some(intro) = self.intro.as_mut() {
            let resize = intro
                .pixmap
                .as_ref()
                .map(|pm| pm.width() != out_w || pm.height() != out_h)
                .unwrap_or(true);
            if resize {
                intro.pixmap = Pixmap::new(out_w, out_h);
            }
            if let Some(pm) = intro.pixmap.as_mut() {
                intro.painter.style_mut().radius_factor = intro.radius_factor;
                let mut canvas = pm.as_mut();
                intro.painter.render(
                    &mut canvas,
                    out_w as f32,
                    out_h as f32,
                    6.0,
                    &FrameOpts {
                        force_needle_north: true,
                        watermark_alpha: 22,
                        ..Default::default()
                    },
                );
                // tiny-skia stores opaque premultiplied RGBA, byte-compatible
                // with the Abgr8888 import the GPU cache uses for the static
                // wallpaper (alpha is 255 everywhere here).
                return pm.data().to_vec();
            }
        }
        self.ensure_loaded();
        compose::compose_for_size(self.image.as_ref(), self.mode, out_w, out_h)
    }

    pub fn source_key(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        if let Some(intro) = &self.intro {
            // Bump per frame so the GPU cache re-uploads the animating compass.
            "intro".hash(&mut hasher);
            intro.frame.hash(&mut hasher);
        } else {
            self.path.hash(&mut hasher);
            (self.mode as u8).hash(&mut hasher);
        }
        hasher.finish()
    }
}

impl Default for WallpaperManager {
    fn default() -> Self {
        Self::new()
    }
}
