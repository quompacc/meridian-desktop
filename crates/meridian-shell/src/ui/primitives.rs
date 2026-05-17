use std::cell::RefCell;

use meridian_config::{Color, ThemeConfig};

use crate::{Painter, Rect, TextRenderer};

use super::tokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind {
    Background,
    Surface,
    Accent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractiveState {
    Default,
    Selected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveIndicatorEdge {
    Top,
    Bottom,
    Left,
}

pub fn active_accent_foreground() -> Color {
    tokens::ACCENT_FOREGROUND
}

pub fn surface_color(theme: &ThemeConfig, kind: SurfaceKind) -> Color {
    match kind {
        SurfaceKind::Background => theme.colors.surface_alt,
        SurfaceKind::Surface => theme.colors.surface,
        SurfaceKind::Accent => theme.colors.accent,
    }
}

pub fn fill_surface_with_radius(
    painter: &mut Painter<'_>,
    rect: Rect,
    theme: &ThemeConfig,
    kind: SurfaceKind,
    radius: i32,
) {
    painter.roundish_rect_with_radius(rect, surface_color(theme, kind), radius);
}

pub fn subtle_border(painter: &mut Painter<'_>, rect: Rect, theme: &ThemeConfig) {
    painter.stroke_rect(rect, theme.colors.border);
}

pub fn draw_active_indicator(
    painter: &mut Painter<'_>,
    rect: Rect,
    edge: ActiveIndicatorEdge,
    theme: &ThemeConfig,
) {
    const THICKNESS: i32 = 2;
    let bar = match edge {
        ActiveIndicatorEdge::Top => Rect {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: THICKNESS.min(rect.h),
        },
        ActiveIndicatorEdge::Bottom => Rect {
            x: rect.x,
            y: rect.y + rect.h - THICKNESS.min(rect.h),
            w: rect.w,
            h: THICKNESS.min(rect.h),
        },
        ActiveIndicatorEdge::Left => Rect {
            x: rect.x,
            y: rect.y,
            w: THICKNESS.min(rect.w),
            h: rect.h,
        },
    };
    painter.rect(bar, theme.colors.accent);
}

pub fn draw_section_separator(
    painter: &mut Painter<'_>,
    x: i32,
    y: i32,
    height: i32,
    theme: &ThemeConfig,
) {
    let line = Rect {
        x,
        y: y + 4,
        w: 1,
        h: (height - 8).max(0),
    };
    painter.rect(line, theme.colors.border);
}

pub fn draw_panel_button(
    painter: &mut Painter<'_>,
    rect: Rect,
    theme: &ThemeConfig,
    state: InteractiveState,
    is_hovered: bool,
) -> Color {
    let default_bg = if is_hovered {
        theme.colors.border
    } else {
        theme.colors.surface
    };
    match state {
        InteractiveState::Default => {
            painter.roundish_rect_with_radius(rect, default_bg, tokens::panel::BUTTON_RADIUS);
            theme.colors.text
        }
        InteractiveState::Selected => {
            painter.roundish_rect_with_radius(rect, default_bg, tokens::panel::BUTTON_RADIUS);
            draw_active_indicator(painter, rect, ActiveIndicatorEdge::Bottom, theme);
            theme.colors.text
        }
    }
}

pub fn draw_sidebar_item(
    painter: &mut Painter<'_>,
    rect: Rect,
    theme: &ThemeConfig,
    state: InteractiveState,
) -> Color {
    match state {
        InteractiveState::Default => theme.colors.border,
        InteractiveState::Selected => {
            fill_surface_with_radius(
                painter,
                rect,
                theme,
                SurfaceKind::Surface,
                tokens::launcher::SIDEBAR_ITEM_RADIUS,
            );
            draw_active_indicator(painter, rect, ActiveIndicatorEdge::Left, theme);
            theme.colors.text
        }
    }
}

pub fn draw_list_item(
    painter: &mut Painter<'_>,
    rect: Rect,
    theme: &ThemeConfig,
    state: InteractiveState,
    with_selected_marker: bool,
) -> Color {
    match state {
        InteractiveState::Default => {
            fill_surface_with_radius(
                painter,
                rect,
                theme,
                SurfaceKind::Background,
                tokens::launcher::LIST_ROW_RADIUS,
            );
            theme.colors.text
        }
        InteractiveState::Selected => {
            fill_surface_with_radius(
                painter,
                rect,
                theme,
                SurfaceKind::Surface,
                tokens::launcher::LIST_ROW_RADIUS,
            );
            draw_active_indicator(painter, rect, ActiveIndicatorEdge::Left, theme);
            if with_selected_marker {
                draw_active_indicator(painter, rect, ActiveIndicatorEdge::Left, theme);
            }
            theme.colors.text
        }
    }
}

pub fn draw_initial_badge(
    painter: &mut Painter<'_>,
    font: &RefCell<Option<TextRenderer>>,
    rect: Rect,
    initial: &str,
    theme: &ThemeConfig,
    state: InteractiveState,
) {
    let (bg, fg) = match state {
        InteractiveState::Default => (theme.colors.border, theme.colors.text),
        InteractiveState::Selected => (active_accent_foreground(), theme.colors.accent),
    };
    painter.roundish_rect_with_radius(rect, bg, tokens::badge::RADIUS);
    painter.text_clipped(font, initial, rect.x + 5, rect.y + 14, rect.w - 6, fg);
}

#[cfg(test)]
mod tests {
    use meridian_config::{Color, ThemeConfig};

    use super::{active_accent_foreground, surface_color, InteractiveState, SurfaceKind};

    #[test]
    fn accent_foreground_matches_token_value() {
        assert_eq!(active_accent_foreground(), Color::rgb(0x1a, 0x1b, 0x26));
    }

    #[test]
    fn surface_color_returns_expected_theme_color() {
        let theme = ThemeConfig::default();
        assert_eq!(
            surface_color(&theme, SurfaceKind::Background),
            theme.colors.surface_alt
        );
        assert_eq!(
            surface_color(&theme, SurfaceKind::Surface),
            theme.colors.surface
        );
        assert_eq!(
            surface_color(&theme, SurfaceKind::Accent),
            theme.colors.accent
        );
    }

    #[test]
    fn interactive_state_selected_is_distinct_from_default() {
        assert_ne!(InteractiveState::Selected, InteractiveState::Default);
    }
}
