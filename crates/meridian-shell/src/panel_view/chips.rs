struct PanelDivider;

impl Widget for PanelDivider {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(DIVIDER_W as f32),
                height: ui_length(CHIP_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        let pal = theme.palette;
        let col = Color::rgba(pal.text.r, pal.text.g, pal.text.b, SEGMENT_DIVIDER_OPACITY);
        let line_height = theme.spacing.md * 2 + ACCENT_LINE_H;
        let line = Rect {
            x: area.x + DIVIDER_W / 2,
            y: area.y + (area.height - line_height) / 2,
            width: 1,
            height: line_height,
        };
        if let Some(path) = rounded_rect_path(line, 0) {
            paint_fill(canvas, &path, col);
        }
    }
}

struct PanelChip {
    id: &'static str,
    label: Box<str>,
    icon: Option<Pixmap>,
    width: i32,
    active: bool,
}

impl PanelChip {
    fn new(
        id: &'static str,
        label: Box<str>,
        icon: Option<Pixmap>,
        width: i32,
        active: bool,
    ) -> Self {
        Self {
            id,
            label,
            icon,
            width,
            active,
        }
    }
}

impl Widget for PanelChip {
    fn id(&self) -> Option<&'static str> {
        Some(self.id)
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length(CHIP_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let highlight = if self.active {
            Some(Interaction::DEFAULT.accent_idle(theme.palette.accent))
        } else {
            match state {
                WidgetState::Idle => None,
                WidgetState::Hovered => Some(Interaction::DEFAULT.neutral_hover),
                WidgetState::Pressed => Some(Interaction::DEFAULT.neutral_pressed),
            }
        };
        if let Some(color) = highlight {
            if let Some(ref path) = rounded_rect_path(area, CHIP_HL_RADIUS) {
                paint_fill(canvas, path, color);
            }
        }

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
    }
}

struct PanelWorkspaceChip {
    active: u8,
    total: u8,
}

impl Widget for PanelWorkspaceChip {
    fn id(&self) -> Option<&'static str> {
        Some("panel-workspace")
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(WS_W as f32),
                height: ui_length(CHIP_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        paint_panel_control_background(area, canvas, state);
        let label = format!("{}", self.active);
        let suffix = format!("/ {}", self.total.max(1));
        let (label_w, _) = measure_text(&label, FONT_SIZE);
        let (suffix_w, _) = measure_text(&suffix, CAPTION_SIZE);
        let dot_diameter = theme.spacing.sm;
        let content_w = dot_diameter + theme.spacing.xs + label_w + theme.spacing.xs + suffix_w;
        let mut x = area.x + (area.width - content_w) / 2;
        draw_circle(
            canvas,
            (x + dot_diameter / 2) as f32,
            (area.y + area.height / 2) as f32,
            dot_diameter as f32 / 2.0,
            theme.palette.accent,
        );
        x += dot_diameter + theme.spacing.xs;
        let baseline = area.y + area.height / 2 + theme.spacing.xs;
        paint_text(canvas, &label, x, baseline, FONT_SIZE, theme.palette.text);
        paint_text(
            canvas,
            &suffix,
            x + label_w + theme.spacing.xs,
            baseline,
            CAPTION_SIZE,
            theme.palette.text_dim,
        );
    }
}

struct PanelClockChip {
    value: Box<str>,
}

impl Widget for PanelClockChip {
    fn id(&self) -> Option<&'static str> {
        Some("panel-clock")
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(CLOCK_W as f32),
                height: ui_length(CHIP_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        paint_panel_control_background(area, canvas, state);
        let (time, date) = self.value.split_once("  ").unwrap_or((&self.value, ""));
        let (time_w, _) = measure_text(time, FONT_SIZE);
        paint_text(
            canvas,
            time,
            area.x + (area.width - time_w) / 2,
            area.y + FONT_SIZE as i32,
            FONT_SIZE,
            theme.palette.text,
        );
        if !date.is_empty() {
            let compact_date = date.get(..5).unwrap_or(date);
            let (date_w, _) = measure_text(compact_date, CAPTION_SIZE);
            paint_text(
                canvas,
                compact_date,
                area.x + (area.width - date_w) / 2,
                area.y + area.height - theme.spacing.xs,
                CAPTION_SIZE,
                theme.palette.text_dim,
            );
        }
    }
}

fn paint_panel_control_background(
    area: Rect,
    canvas: &mut PixmapMut<'_>,
    state: WidgetState,
) {
    let color = match state {
        WidgetState::Idle => None,
        WidgetState::Hovered => Some(Interaction::DEFAULT.neutral_hover),
        WidgetState::Pressed => Some(Interaction::DEFAULT.neutral_pressed),
    };
    if let Some(color) = color {
        if let Some(path) = rounded_rect_path(area, CHIP_HL_RADIUS) {
            paint_fill(canvas, &path, color);
        }
    }
}

// ── PanelPinnedChip ─────────────────────────────────────────────────────────
