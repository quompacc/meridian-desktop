pub mod config;
pub mod keybind;
pub mod theme;

pub use config::MeridianConfig;
pub use keybind::{Action, Keybind, KeybindConfig, Modifiers, SplitDir};
pub use theme::{
    Color, Cursor, Decorations, Fonts, Icons, Theme, ThemeColors, ThemeConfig, ThemeError,
    ThemeManager, Wallpaper, WallpaperMode,
};
