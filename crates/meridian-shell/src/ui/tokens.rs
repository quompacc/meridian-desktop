use meridian_config::{Color, ThemeConfig};
use meridian_ui::style::{
    Color as UiColor, Palette as UiPalette, Radius as UiRadius, Spacing as UiSpacing,
    Theme as UiTheme,
};

pub const ACCENT_FOREGROUND: Color = Color::rgb(0x1a, 0x1b, 0x26);
pub const GLASS_FOREGROUND: Color = Color::rgb(0x05, 0x08, 0x0c);
pub const GLASS_FOREGROUND_DIM: Color = Color::rgb(0x1e, 0x28, 0x34);
pub const GLASS_BORDER: Color = Color::rgba(0x05, 0x08, 0x0c, 110);

pub(crate) fn color_from_config(color: Color) -> UiColor {
    UiColor::rgba(color.r, color.g, color.b, color.a)
}

pub(crate) fn palette_from_config(config: &ThemeConfig) -> UiPalette {
    let colors = &config.colors;
    UiPalette {
        background: color_from_config(colors.background),
        surface: color_from_config(colors.surface),
        surface_alt: color_from_config(colors.surface_alt),
        accent: color_from_config(colors.accent),
        accent_alt: color_from_config(colors.accent_alt),
        text: color_from_config(colors.text),
        text_dim: color_from_config(colors.text_dim),
        border: color_from_config(colors.border),
        error: color_from_config(colors.error),
        warning: color_from_config(colors.warning),
        success: color_from_config(colors.success),
    }
}

pub(crate) fn glass_palette_from_config(config: &ThemeConfig) -> UiPalette {
    let mut palette = palette_from_config(config);
    palette.text = color_from_config(GLASS_FOREGROUND);
    palette.text_dim = color_from_config(GLASS_FOREGROUND_DIM);
    palette.border = color_from_config(GLASS_BORDER);
    palette
}

pub(crate) fn theme_from_config(config: &ThemeConfig) -> UiTheme {
    UiTheme {
        palette: palette_from_config(config),
        spacing: UiSpacing::DEFAULT,
        radius: UiRadius::METRO,
    }
}

pub(crate) fn glass_theme_from_config(config: &ThemeConfig) -> UiTheme {
    UiTheme {
        palette: glass_palette_from_config(config),
        spacing: UiSpacing::DEFAULT,
        radius: UiRadius::METRO,
    }
}
