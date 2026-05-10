mod manager;
mod types;

pub use manager::{Theme, ThemeManager};
pub use types::{
    Color, Cursor, Decorations, Fonts, Icons, ThemeColors, ThemeConfig, ThemeError, Wallpaper,
    WallpaperMode,
};
