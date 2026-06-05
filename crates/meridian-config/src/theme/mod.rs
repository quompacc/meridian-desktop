mod manager;
mod types;

pub use manager::{Theme, ThemeManager};
pub use types::{
    Color, Cursor, Decorations, Fonts, Icons, SurfaceTreatment, ThemeColors, ThemeConfig,
    ThemeError, ThemeSurface, Wallpaper, WallpaperMode,
};
