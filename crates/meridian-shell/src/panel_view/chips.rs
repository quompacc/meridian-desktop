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
        let line = Rect {
            x: area.x + DIVIDER_W / 2,
            y: area.y + 6,
            width: 1,
            height: (area.height - 12).max(1),
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
        // The launcher chip is special: no rectangular chip chrome,
        // just the compass rose centred in the panel so the icon
        // visually sits proud of the panel line (Win8-style start-button
        // pivot). Skip the bg fill + accent strip and let the icon
        // speak for itself.
        let is_launcher = self.id == "panel-launcher";
        // Reconstruct the bar top from the chip rect so absolute placements
        // survive the panel top-shadow margin (chip is centred in the bar).
        let bar_top = area.y - (PANEL_H - area.height) / 2;

        if is_launcher {
            let halo_color = match state {
                WidgetState::Idle => None,
                WidgetState::Hovered => Some(Interaction::DEFAULT.hover(theme.palette.accent)),
                WidgetState::Pressed => Some(Interaction::DEFAULT.pressed(theme.palette.accent)),
            };
            if let Some(mut color) = halo_color {
                color.a = match state {
                    WidgetState::Hovered => 74,
                    WidgetState::Pressed => 96,
                    WidgetState::Idle => 0,
                };
                let halo = Rect {
                    x: area.x + 1,
                    y: bar_top + 2,
                    width: area.width - 2,
                    height: PANEL_H - 4,
                };
                if let Some(ref path) = rounded_rect_path(halo, 8) {
                    paint_fill(canvas, path, color);
                }
            }
        } else {
            // Idle chips draw no frame so the frosted glass shows through —
            // only the icon floats. Active/hover/pressed get a soft, rounded,
            // translucent highlight that keeps the glass visible.
            let hl: Option<Color> = if self.active {
                Some(Interaction::DEFAULT.accent_idle(theme.palette.accent))
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
        }

        if let Some(ref icon) = self.icon {
            let iw = icon.width() as i32;
            let ih = icon.height() as i32;
            let x = area.x + (area.width - iw) / 2;
            // Launcher: vertical-centre against the whole panel so an
            // oversized rose extends slightly above/below the chip's
            // own rectangle, not just within it.
            let y = if is_launcher {
                bar_top + (PANEL_H - ih) / 2 + if state == WidgetState::Pressed { 1 } else { 0 }
            } else {
                area.y + (area.height - ACCENT_LINE_H - ih) / 2
            };
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

        if !is_launcher {
            // accent line bottom
            let line = Rect {
                x: area.x,
                y: area.y + area.height - ACCENT_LINE_H,
                width: area.width,
                height: ACCENT_LINE_H,
            };
            if let Some(ref path) = rounded_rect_path(line, 0) {
                paint_fill(canvas, path, theme.palette.accent);
            }
        }
    }
}

// ── PanelPinnedChip ─────────────────────────────────────────────────────────
