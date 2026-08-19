struct DisplayModeComboButton {
    index: usize,
    label: Box<str>,
    expanded: bool,
    enabled: bool,
    accent: Color,
}

impl Widget for DisplayModeComboButton {
    fn id(&self) -> Option<&'static str> {
        if self.enabled {
            DISPLAY_MODE_TOGGLE_IDS.get(self.index).copied()
        } else {
            None
        }
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(DISPLAY_MODE_COMBO_W as f32),
                height: ui_length(DISPLAY_CARD_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let base = if self.expanded {
            Interaction::DEFAULT.selection(
                theme.palette.surface_alt,
                self.accent,
                Interaction::SELECTION_EXPANDED,
            )
        } else if self.enabled {
            theme.palette.surface_alt
        } else {
            theme.palette.surface
        };
        let bg = match state {
            WidgetState::Idle => base,
            WidgetState::Hovered if self.enabled => Interaction::DEFAULT.hover(base),
            WidgetState::Pressed if self.enabled => Interaction::DEFAULT.pressed(base),
            _ => base,
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        let color = if self.enabled {
            theme.palette.text
        } else {
            theme.palette.text_dim
        };
        paint_text(
            canvas,
            "Mode",
            area.x + 14,
            area.y + 28,
            11.0,
            theme.palette.text_dim,
        );
        paint_text(
            canvas,
            &fit_text(&self.label, 23),
            area.x + 14,
            area.y + 58,
            11.0,
            color,
        );
        paint_text(
            canvas,
            if self.expanded { "^" } else { "v" },
            area.x + area.width - 22,
            area.y + 58,
            13.0,
            if self.enabled {
                self.accent
            } else {
                theme.palette.text_dim
            },
        );
    }
}

struct DisplayModeOptionRow {
    output_index: usize,
    mode_index: usize,
    label: Box<str>,
    selected: bool,
    row_width: i32,
    accent: Color,
}

impl Widget for DisplayModeOptionRow {
    fn id(&self) -> Option<&'static str> {
        DISPLAY_MODE_OPTION_IDS
            .get(self.output_index)
            .and_then(|ids| ids.get(self.mode_index))
            .copied()
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(DISPLAY_MODE_OPTION_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let base = if self.selected {
            Interaction::DEFAULT.selection(
                theme.palette.surface_alt,
                self.accent,
                Interaction::SELECTION_SELECTED,
            )
        } else {
            theme.palette.surface_alt
        };
        let bg = match state {
            WidgetState::Idle => base,
            WidgetState::Hovered => Interaction::DEFAULT.hover(base),
            WidgetState::Pressed => Interaction::DEFAULT.pressed(base),
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        paint_text(
            canvas,
            if self.selected { "*" } else { "" },
            area.x + 14,
            area.y + 23,
            12.0,
            self.accent,
        );
        paint_text(
            canvas,
            &fit_text(&self.label, 58),
            area.x + 34,
            area.y + 23,
            12.0,
            if self.selected {
                self.accent
            } else {
                theme.palette.text
            },
        );
    }
}

struct DisplayPrimaryButton {
    index: usize,
    active: bool,
    accent: Color,
}

impl Widget for DisplayPrimaryButton {
    fn id(&self) -> Option<&'static str> {
        if self.active {
            None
        } else {
            DISPLAY_PRIMARY_IDS.get(self.index).copied()
        }
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(DISPLAY_PRIMARY_BTN_W as f32),
                height: ui_length(DISPLAY_CARD_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let base = if self.active {
            Interaction::DEFAULT.selection(
                theme.palette.surface_alt,
                self.accent,
                Interaction::SELECTION_ACTIVE,
            )
        } else {
            theme.palette.surface_alt
        };
        let bg = match state {
            WidgetState::Idle => base,
            WidgetState::Hovered => Interaction::DEFAULT.hover(base),
            WidgetState::Pressed => Interaction::DEFAULT.pressed(base),
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }

        let dot = Rect {
            x: area.x + area.width / 2 - 5,
            y: area.y + 18,
            width: 10,
            height: 10,
        };
        if let Some(path) = rounded_rect_path(dot, 5) {
            paint_fill(
                canvas,
                &path,
                if self.active {
                    self.accent
                } else {
                    theme.palette.text_dim
                },
            );
        }
        let color = if self.active {
            self.accent
        } else {
            theme.palette.text
        };
        paint_text(
            canvas,
            if self.active { "Primary" } else { "Make" },
            area.x + 18,
            area.y + 55,
            12.0,
            color,
        );
        paint_text(
            canvas,
            if self.active { "active" } else { "Primary" },
            area.x + 18,
            area.y + 73,
            12.0,
            color,
        );
    }
}

/// A compact "cycle" button for a per-output setting (scale or rotation). Shows
/// a caption line and the current value; clicking advances to the next value
/// (handled in the dispatch). `id` selects which action fires.
struct DisplayCycleButton {
    id: &'static str,
    caption: &'static str,
    value: Box<str>,
    accent: Color,
}

impl Widget for DisplayCycleButton {
    fn id(&self) -> Option<&'static str> {
        Some(self.id)
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(DISPLAY_PRIMARY_BTN_W as f32),
                height: ui_length(DISPLAY_CARD_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let base = theme.palette.surface_alt;
        let bg = match state {
            WidgetState::Idle => base,
            WidgetState::Hovered => Interaction::DEFAULT.hover(base),
            WidgetState::Pressed => Interaction::DEFAULT.pressed(base),
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        paint_text(
            canvas,
            self.caption,
            area.x + 14,
            area.y + 40,
            11.0,
            theme.palette.text_dim,
        );
        paint_text(
            canvas,
            &self.value,
            area.x + 14,
            area.y + 64,
            14.0,
            self.accent,
        );
    }
}

struct AddAppRow {
    index: usize,
    name: Box<str>,
    accent: Color,
    row_width: i32,
    icon: Option<Pixmap>,
}

impl Widget for AddAppRow {
    fn id(&self) -> Option<&'static str> {
        PINNED_ADD_IDS.get(self.index).copied()
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(PINNED_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = match state {
            WidgetState::Idle => theme.palette.surface_alt,
            WidgetState::Hovered => Interaction::DEFAULT.hover(theme.palette.surface_alt),
            WidgetState::Pressed => Interaction::DEFAULT.pressed(theme.palette.surface_alt),
        };
        if let Some(path) = rounded_rect_path(area, 0) {
            paint_fill(canvas, &path, bg);
        }
        let text_x = if let Some(ref icon) = self.icon {
            let iw = icon.width() as i32;
            let ih = icon.height() as i32;
            let ix = area.x + 6;
            let iy = area.y + (area.height - ih) / 2;
            canvas.draw_pixmap(
                ix,
                iy,
                icon.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
            area.x + 6 + iw + 6
        } else {
            area.x + 10
        };
        paint_text(
            canvas,
            &self.name,
            text_x,
            area.y + area.height - 14,
            13.0,
            self.accent,
        );
    }
}

struct Divider {
    width: i32,
    color: Color,
}

impl Widget for Divider {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length(DIVIDER_HEIGHT as f32),
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
