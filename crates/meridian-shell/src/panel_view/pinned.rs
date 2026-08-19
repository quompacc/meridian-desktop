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

        // Icon (centered, shifted up slightly to leave room for indicator)
        if let Some(ref icon) = self.icon {
            let iw = icon.width() as i32;
            let ih = icon.height() as i32;
            let x = area.x + (area.width - iw) / 2;
            let y = area.y + (area.height - ACCENT_LINE_H - ih) / 2;
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
            let ty = area.y + (area.height - ACCENT_LINE_H) / 2 + 5;
            paint_text(canvas, &self.label, tx, ty, FONT_SIZE, theme.palette.text);
        }

        // Indicator: dot or pill at the bottom of the chip
        let chip_cx = (area.x + area.width / 2) as f32;
        let indicator_cy = (area.y + area.height - 2) as f32; // 2px from chip bottom

        match self.window_count {
            0 => {
                // No running window: dim accent line (subtle, just chip chrome)
                let dim = Color::rgba(
                    theme.palette.accent.r,
                    theme.palette.accent.g,
                    theme.palette.accent.b,
                    IDLE_INDICATOR_OPACITY,
                );
                let line = Rect {
                    x: area.x + 4,
                    y: area.y + area.height - ACCENT_LINE_H,
                    width: area.width - 8,
                    height: ACCENT_LINE_H,
                };
                if let Some(ref path) = rounded_rect_path(line, 1) {
                    paint_fill(canvas, path, dim);
                }
            }
            1 => {
                // Single window: small dot
                let dot_color = if self.has_focused {
                    Color::rgba(
                        theme.palette.text.r,
                        theme.palette.text.g,
                        theme.palette.text.b,
                        FOCUSED_DOT_OPACITY,
                    )
                } else {
                    theme.palette.accent
                };
                draw_circle(canvas, chip_cx, indicator_cy, 2.5, dot_color);
            }
            n => {
                // Multiple windows: pill with count
                let dot_color = if self.has_focused {
                    theme.palette.text
                } else {
                    theme.palette.accent
                };
                let label: Box<str> = if n > 9 {
                    "9+".into()
                } else {
                    n.to_string().into()
                };
                let (text_w, _) = measure_text(&label, 9.0);
                let pill_w = (text_w + 8).max(14);
                let pill_h = 9;
                let pill_x = area.x + (area.width - pill_w) / 2;
                let pill_y = area.y + area.height - pill_h - 1;
                if let Some(ref path) = rounded_rect_path(
                    Rect {
                        x: pill_x,
                        y: pill_y,
                        width: pill_w,
                        height: pill_h,
                    },
                    4,
                ) {
                    paint_fill(canvas, path, dot_color);
                }
                let text_color = theme.palette.background;
                paint_text(
                    canvas,
                    &label,
                    pill_x + (pill_w - text_w) / 2,
                    pill_y + pill_h - 1,
                    9.0,
                    text_color,
                );
            }
        }
    }
}
