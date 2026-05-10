mod color;
mod config;
mod error;

pub use color::Color;
pub use config::{
    Cursor, Decorations, Fonts, Icons, ThemeColors, ThemeConfig, Wallpaper, WallpaperMode,
};
pub use error::ThemeError;
