fn with_alpha(color: Color, alpha: u8) -> Color {
    Color::rgba(color.r, color.g, color.b, alpha)
}

fn settings_glass_theme_from_config(config: &ThemeConfig) -> Theme {
    let mut theme = glass_theme_from_config(config);
    theme.palette.background = with_alpha(theme.palette.background, 0);
    theme.palette.surface = with_alpha(theme.palette.surface, 0);
    theme.palette.surface_alt = with_alpha(theme.palette.surface_alt, 0);
    theme.palette.text_dim = theme.palette.text;
    theme
}

/// Header bar: paints the surface background and lays out the back button +
/// search field as a centred row. Matches the command palette header height.
struct SettingsHeaderBar {
    width: i32,
    children: Vec<Box<dyn Widget>>,
}

impl Widget for SettingsHeaderBar {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Center),
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length(HEADER_HEIGHT as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, 0) {
            paint_fill(canvas, &path, theme.palette.surface);
        }
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }
}

/// Clickable back arrow that returns to the command palette. Shares the
/// "show-tile-view" id so it dispatches ToggleSettings like the footer button.
struct SettingsBackButton;

impl Widget for SettingsBackButton {
    fn id(&self) -> Option<&'static str> {
        Some("show-tile-view")
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(SETTINGS_BACK_W as f32),
                height: ui_length(HEADER_HEIGHT as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let hot = matches!(state, WidgetState::Hovered | WidgetState::Pressed);
        if hot {
            let bg = Interaction::DEFAULT.hover(theme.palette.surface);
            if let Some(path) = rounded_rect_path(area, 0) {
                paint_fill(canvas, &path, bg);
            }
        }
        let col = if hot {
            theme.palette.text
        } else {
            theme.palette.accent
        };
        let baseline = area.y + (area.height + 16) / 2;
        paint_text(canvas, "\u{2190}", area.x + 18, baseline, 17.0, col);
    }
}

/// Search field filling the rest of the header. Type-to-filter; the actual
/// query lives in MeridianShell.settings_search and is fed in via `query`.
struct SettingsSearchField {
    width: i32,
    query: Box<str>,
}

impl Widget for SettingsSearchField {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length(HEADER_HEIGHT as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        let pal = theme.palette;
        let baseline = area.y + (area.height + 13) / 2;
        let text_x = area.x + 4;
        if self.query.is_empty() {
            paint_text(
                canvas,
                "Einstellungen durchsuchen…",
                text_x,
                baseline,
                13.0,
                pal.text,
            );
        } else {
            paint_text(canvas, &self.query, text_x, baseline, 13.0, pal.text);
        }
    }
}

/// Full-height sidebar panel: paints a distinct surface_alt background and
/// stacks its children (section labels + category rows) from the top.
struct SidebarPanel {
    width: i32,
    height: i32,
    bg: Color,
    children: Vec<Box<dyn Widget>>,
}

impl Widget for SidebarPanel {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            flex_direction: FlexDirection::Column,
            justify_content: Some(JustifyContent::FlexStart),
            align_items: Some(AlignItems::Stretch),
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length(self.height as f32),
            },
            padding: TaffyRect {
                left: ui_length(0.0),
                right: ui_length(0.0),
                top: ui_length(10.0),
                bottom: ui_length(0.0),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, _theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, 0) {
            paint_fill(canvas, &path, self.bg);
        }
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }
}

/// Small dim all-caps group label inside the sidebar (e.g. "DARSTELLUNG").
struct SidebarSectionLabel {
    text: &'static str,
    width: i32,
    pad_top: i32,
}

impl Widget for SidebarSectionLabel {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length((self.pad_top + 18) as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        paint_text(
            canvas,
            self.text,
            area.x + 16,
            area.y + area.height - 6,
            10.0,
            theme.palette.text,
        );
    }
}

struct SettingsSidebarRow {
    cat: SettingsCategory,
    is_selected: bool,
    accent: Color,
    row_width: i32,
}

impl Widget for SettingsSidebarRow {
    fn id(&self) -> Option<&'static str> {
        Some(self.cat.chip_id())
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(SIDEBAR_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        // Sidebar sits on a surface_alt panel; idle rows blend with it, the
        // selected row gets a subtle accent tint + left bar.
        let base = theme.palette.surface_alt;
        let bg = match state {
            WidgetState::Idle if self.is_selected => {
                Interaction::DEFAULT.selection(base, self.accent, Interaction::SELECTION_HOVER)
            }
            WidgetState::Idle => base,
            WidgetState::Hovered => Interaction::DEFAULT.hover(base),
            WidgetState::Pressed => Interaction::DEFAULT.pressed(base),
        };
        if let Some(path) = rounded_rect_path(area, 0) {
            paint_fill(canvas, &path, bg);
        }
        if self.is_selected {
            let strip = Rect {
                x: area.x,
                y: area.y + 8,
                width: 3,
                height: area.height - 16,
            };
            if let Some(path) = rounded_rect_path(strip, 1) {
                paint_fill(canvas, &path, self.accent);
            }
        }
        let text_color = if self.is_selected {
            self.accent
        } else {
            theme.palette.text
        };
        paint_text(
            canvas,
            self.cat.label(),
            area.x + 14,
            area.y + area.height - 14,
            12.5,
            text_color,
        );
    }
}

struct VerticalDivider {
    height: i32,
    color: Color,
}

impl Widget for VerticalDivider {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(1.0),
                height: ui_length(self.height as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, _theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, 0) {
            paint_fill(canvas, &path, self.color);
        }
    }
}

struct ThemeRow {
    index: usize,
    name: Box<str>,
    is_selected: bool,
    accent: Color,
    row_width: i32,
}

impl Widget for ThemeRow {
    fn id(&self) -> Option<&'static str> {
        THEME_WIDGET_IDS.get(self.index).copied()
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(THEME_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = match state {
            WidgetState::Idle => {
                if self.is_selected {
                    Interaction::DEFAULT.selected_tint(theme.palette.surface)
                } else {
                    theme.palette.surface
                }
            }
            WidgetState::Hovered => Interaction::DEFAULT.hover(theme.palette.surface),
            WidgetState::Pressed => Interaction::DEFAULT.pressed(theme.palette.surface),
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        if self.is_selected {
            let strip = Rect {
                x: area.x + 4,
                y: area.y + 8,
                width: 3,
                height: area.height - 16,
            };
            if let Some(path) = rounded_rect_path(strip, 1) {
                paint_fill(canvas, &path, self.accent);
            }
        }
        let text_color = if self.is_selected {
            self.accent
        } else {
            theme.palette.text
        };
        paint_text(
            canvas,
            &self.name,
            area.x + 16,
            area.y + area.height - 14,
            13.0,
            text_color,
        );
    }
}

/// A selectable cursor-theme row. Visually identical to `ThemeRow`; differs
/// only in which static id array it indexes, so clicks route to the cursor
/// theme action rather than the colour-theme one.
struct CursorThemeRow {
    index: usize,
    name: Box<str>,
    is_selected: bool,
    accent: Color,
    row_width: i32,
}

impl Widget for CursorThemeRow {
    fn id(&self) -> Option<&'static str> {
        crate::cursor::CURSOR_THEME_WIDGET_IDS
            .get(self.index)
            .copied()
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(THEME_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = match state {
            WidgetState::Idle => {
                if self.is_selected {
                    Interaction::DEFAULT.selected_tint(theme.palette.surface)
                } else {
                    theme.palette.surface
                }
            }
            WidgetState::Hovered => Interaction::DEFAULT.hover(theme.palette.surface),
            WidgetState::Pressed => Interaction::DEFAULT.pressed(theme.palette.surface),
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        if self.is_selected {
            let strip = Rect {
                x: area.x + 4,
                y: area.y + 8,
                width: 3,
                height: area.height - 16,
            };
            if let Some(path) = rounded_rect_path(strip, 1) {
                paint_fill(canvas, &path, self.accent);
            }
        }
        let text_color = if self.is_selected {
            self.accent
        } else {
            theme.palette.text
        };
        paint_text(
            canvas,
            &self.name,
            area.x + 16,
            area.y + area.height - 14,
            13.0,
            text_color,
        );
    }
}

const WALLPAPER_MODE_BAR_H: u32 = 52;
const WALLPAPER_ROW_H: i32 = 64;
const WALLPAPER_THUMB_W: u32 = 96;
const WALLPAPER_THUMB_H: u32 = 54;
