struct PanelPinnedChip {
    idx: usize,
    label: Box<str>,
    icon: Option<Pixmap>,
    program: Box<str>,
    args: Vec<String>,
    window_count: usize,
    has_focused: bool,
}

impl Widget for PanelPinnedChip {
    fn id(&self) -> Option<&'static str> {
        None
    }

    fn pinned_app_idx(&self) -> Option<usize> {
        Some(self.idx)
    }

    fn launch_info(&self) -> Option<(&str, &[String])> {
        Some((&self.program, &self.args))
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(PINNED_W as f32),
                height: ui_length(CHIP_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        // Background — subtle highlight when this app has the focused window
        // No opaque idle frame — the icon floats on the glass. The focused
        // app gets a subtle translucent accent cushion; hover/press a soft
        // rounded highlight.
        let hl: Option<Color> = if self.has_focused {
            match state {
                WidgetState::Idle => Some(Interaction::DEFAULT.accent_idle(theme.palette.accent)),
                WidgetState::Hovered => {
                    Some(Interaction::DEFAULT.accent_hover(theme.palette.accent))
                }
                WidgetState::Pressed => Some(Interaction::DEFAULT.neutral_pressed),
            }
        } else {
            match state {
                WidgetState::Idle => None,
                WidgetState::Hovered => Some(Interaction::DEFAULT.neutral_hover),
                WidgetState::Pressed => Some(Interaction::DEFAULT.neutral_pressed),
            }
        };
        if let Some(color) = hl {
            if let Some(ref path) = rounded_rect_path(area, CHIP_HL_RADIUS) {
                paint_fill(canvas, path, color);
            }
        }

        // Icon stays optically centred; the running indicator occupies only
        // the bottom edge and does not push the application mark upward.
        if let Some(ref icon) = self.icon {
            let iw = icon.width() as i32;
            let ih = icon.height() as i32;
            let x = area.x + (area.width - iw) / 2;
            let y = area.y + (area.height - ih) / 2;
            canvas.draw_pixmap(
                x,
                y,
                icon.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        } else {
            let (text_w, _) = measure_text(&self.label, FONT_SIZE);
            let tx = area.x + (area.width - text_w) / 2;
            let ty = area.y + area.height / 2 + theme.spacing.xs;
            paint_text(canvas, &self.label, tx, ty, FONT_SIZE, theme.palette.text);
        }

        if self.window_count > 0 {
            let line_w = if self.has_focused {
                theme.spacing.xl + theme.spacing.sm
            } else {
                theme.spacing.xl
            };
            let line = Rect {
                x: area.x + (area.width - line_w) / 2,
                y: area.y + area.height - ACCENT_LINE_H,
                width: line_w,
                height: ACCENT_LINE_H,
            };
            if let Some(path) = rounded_rect_path(line, meridian_tokens::Radius::DEFAULT.sm) {
                paint_fill(canvas, &path, theme.palette.accent);
            }
        }
    }
}
