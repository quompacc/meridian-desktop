use meridian_config::{Color, ThemeConfig, ThemeSurface};
use meridian_ui::style::{
    Color as UiColor, Palette as UiPalette, Radius as UiRadius, Spacing as UiSpacing,
    Theme as UiTheme,
};

pub(crate) fn color_from_config(color: Color) -> UiColor {
    UiColor::rgba(color.r, color.g, color.b, color.a)
}

pub(crate) fn color_with_alpha(color: Color, alpha: u8) -> Color {
    Color::rgba(color.r, color.g, color.b, alpha)
}

pub(crate) fn glass_foreground_from_config(config: &ThemeConfig) -> Color {
    config.colors.text
}

pub(crate) fn glass_dim_from_config(config: &ThemeConfig) -> Color {
    config.colors.text_dim
}

pub(crate) fn glass_border_from_config(config: &ThemeConfig) -> Color {
    let treatment = config.decorations.surface_treatment(ThemeSurface::Popup);
    color_with_alpha(config.colors.border, treatment.frame_alpha)
}

pub(crate) fn accent_foreground_from_config(config: &ThemeConfig) -> Color {
    // One central formula + named contrast pair (Palette::TEXT_ON_LIGHT/DARK).
    meridian_tokens::contrast_text(config.colors.accent)
}

pub(crate) fn surface_radius_from_config(config: &ThemeConfig, surface: ThemeSurface) -> i32 {
    config.decorations.surface_radius(surface).round() as i32
}

fn radius_scale_from_config(config: &ThemeConfig) -> UiRadius {
    let card = surface_radius_from_config(config, ThemeSurface::Popup);
    let control = surface_radius_from_config(config, ThemeSurface::Control);
    UiRadius {
        none: 0,
        sm: (control * 3 / 4).max(0),
        md: control.max(0),
        lg: surface_radius_from_config(config, ThemeSurface::Panel).max(0),
        xl: card.max(0),
    }
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
    palette.border = color_from_config(glass_border_from_config(config));
    palette
}

pub(crate) fn theme_from_config(config: &ThemeConfig) -> UiTheme {
    UiTheme {
        palette: palette_from_config(config),
        spacing: UiSpacing::DEFAULT,
        radius: radius_scale_from_config(config),
    }
}

pub(crate) fn glass_theme_from_config(config: &ThemeConfig) -> UiTheme {
    UiTheme {
        palette: glass_palette_from_config(config),
        spacing: UiSpacing::DEFAULT,
        radius: radius_scale_from_config(config),
    }
}
