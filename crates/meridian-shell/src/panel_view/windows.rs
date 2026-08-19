// ── PanelWindowChip ─────────────────────────────────────────────────────────

#[cfg(test)]

struct PanelWindowChip {
    window_id: Box<str>,
    title: Box<str>,
    focused: bool,
    minimized: bool,
    width: i32,
}

#[cfg(test)]
impl Widget for PanelWindowChip {
    fn id(&self) -> Option<&'static str> {
        None
    }

    fn focus_window_id(&self) -> Option<&str> {
        Some(&self.window_id)
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length(20.0),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let base_bg = if self.focused {
            theme
                .palette
                .border
                .lerp(Color::rgb(0xFF, 0xFF, 0xFF), 0.15)
        } else if self.minimized {
            theme.palette.surface.lerp(Color::rgb(0, 0, 0), 0.25)
        } else {
            theme.palette.surface
        };

        let bg = match state {
            WidgetState::Idle => base_bg,
            WidgetState::Hovered => Interaction::DEFAULT.hover(base_bg),
            WidgetState::Pressed => Interaction::DEFAULT.pressed(base_bg),
        };

        if let Some(ref path) = rounded_rect_path(area, CHIP_HL_RADIUS) {
            paint_fill(canvas, path, bg);
        }

        let text_color = if self.focused {
            theme.palette.accent
        } else if self.minimized {
            theme.palette.text_dim
        } else {
            theme.palette.text
        };

        paint_text(
            canvas,
            &self.title,
            area.x + 4,
            area.y + area.height / 2 + 3,
            FONT_SIZE,
            text_color,
        );

        if self.focused {
            let indicator_rect = Rect {
                x: area.x,
                y: area.y + area.height - 2,
                width: area.width,
                height: 2,
            };
            if let Some(ref path) = rounded_rect_path(indicator_rect, 0) {
                paint_fill(canvas, path, theme.palette.accent);
            }
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────
