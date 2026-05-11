use std::cell::RefCell;

use meridian_config::{Color, ThemeConfig};

use crate::{Painter, Rect, TextRenderer};

use super::tokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind {
    Background,
    Surface,
    Accent,
    Border,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractiveState {
    Default,
    Selected,
}

pub fn active_accent_foreground() -> Color {
    tokens::ACCENT_FOREGROUND
}

pub fn surface_color(theme: &ThemeConfig, kind: SurfaceKind) -> Color {
    match kind {
        SurfaceKind::Background => theme.colors.background,
        SurfaceKind::Surface => theme.colors.surface,
        SurfaceKind::Accent => theme.colors.accent,
        SurfaceKind::Border => theme.colors.border,
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

pub fn draw_workspace_button(
    painter: &mut Painter<'_>,
    rect: Rect,
    theme: &ThemeConfig,
    is_active: bool,
    is_occupied: bool,
) -> Color {
    if is_active {
        fill_surface_with_radius(
            painter,
            rect,
            theme,
            SurfaceKind::Accent,
            tokens::panel::BUTTON_RADIUS,
        );
        active_accent_foreground()
    } else if is_occupied {
        fill_surface_with_radius(
            painter,
            rect,
            theme,
            SurfaceKind::Border,
            tokens::panel::BUTTON_RADIUS,
        );
        theme.colors.text
    } else {
        fill_surface_with_radius(
            painter,
            rect,
            theme,
            SurfaceKind::Background,
            tokens::panel::BUTTON_RADIUS,
        );
        theme.colors.text
    }
}

pub fn draw_panel_button(
    painter: &mut Painter<'_>,
    rect: Rect,
    theme: &ThemeConfig,
    state: InteractiveState,
) -> Color {
    match state {
        InteractiveState::Default => {
            fill_surface_with_radius(
                painter,
                rect,
                theme,
                SurfaceKind::Background,
                tokens::panel::BUTTON_RADIUS,
            );
            theme.colors.text
        }
        InteractiveState::Selected => {
            fill_surface_with_radius(
                painter,
                rect,
                theme,
                SurfaceKind::Accent,
                tokens::panel::BUTTON_RADIUS,
            );
            active_accent_foreground()
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
                SurfaceKind::Accent,
                tokens::launcher::SIDEBAR_ITEM_RADIUS,
            );
            active_accent_foreground()
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
                SurfaceKind::Surface,
                tokens::launcher::LIST_ROW_RADIUS,
            );
            theme.colors.text
        }
        InteractiveState::Selected => {
            fill_surface_with_radius(
                painter,
                rect,
                theme,
                SurfaceKind::Accent,
                tokens::launcher::LIST_ROW_RADIUS,
            );
            if with_selected_marker {
                painter.rect(
                    Rect {
                        x: rect.x + 2,
                        y: rect.y + 2,
                        w: 3,
                        h: rect.h - 4,
                    },
                    theme.colors.text,
                );
            }
            active_accent_foreground()
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
        assert_eq!(active_accent_foreground(), Color::rgb(0x1e, 0x1e, 0x2e));
    }

    #[test]
    fn surface_color_returns_expected_theme_color() {
        let theme = ThemeConfig::default();
        assert_eq!(
            surface_color(&theme, SurfaceKind::Background),
            theme.colors.background
        );
        assert_eq!(
            surface_color(&theme, SurfaceKind::Surface),
            theme.colors.surface
        );
        assert_eq!(
            surface_color(&theme, SurfaceKind::Accent),
            theme.colors.accent
        );
        assert_eq!(
            surface_color(&theme, SurfaceKind::Border),
            theme.colors.border
        );
    }

    #[test]
    fn interactive_state_selected_is_distinct_from_default() {
        assert_ne!(InteractiveState::Selected, InteractiveState::Default);
    }
}
