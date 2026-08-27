fn fit_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

struct DisplayOutputRow {
    output_id: u32,
    name: Box<str>,
    workspace: usize,
    primary: bool,
    focused: bool,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    scale_millis: u32,
    transform: Option<Box<str>>,
    refresh_millihz: Option<i32>,
    mode_count: usize,
    row_width: i32,
    accent: Color,
}

impl Widget for DisplayOutputRow {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(SETTINGS_CHROME.display_identity_height as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        let bg = if self.focused {
            Interaction::DEFAULT.selection(
                theme.palette.surface,
                self.accent,
                Interaction::SELECTION_FOCUSED,
            )
        } else {
            theme.palette.surface
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }

        let monitor_color = if self.focused {
            self.accent
        } else {
            theme.palette.text_dim
        };
        let screen = Rect {
            x: area.x + 16,
            y: area.y + 18,
            width: 78,
            height: 48,
        };
        if let Some(path) = rounded_rect_path(screen, 4) {
            paint_fill(canvas, &path, monitor_color);
        }
        let inner = Rect {
            x: screen.x + 4,
            y: screen.y + 4,
            width: screen.width - 8,
            height: screen.height - 8,
        };
        if let Some(path) = rounded_rect_path(inner, 2) {
            paint_fill(canvas, &path, theme.palette.background);
        }
        let preview = Rect {
            x: inner.x + 6,
            y: inner.y + 6,
            width: inner.width - 12,
            height: inner.height - 12,
        };
        if let Some(path) = rounded_rect_path(preview, 1) {
            paint_fill(canvas, &path, bg.lerp(monitor_color, MONITOR_PREVIEW_MIX));
        }
        let stand = Rect {
            x: screen.x + 34,
            y: screen.y + screen.height,
            width: 10,
            height: 10,
        };
        if let Some(path) = rounded_rect_path(stand, 1) {
            paint_fill(canvas, &path, monitor_color);
        }
        let foot = Rect {
            x: screen.x + 24,
            y: screen.y + screen.height + 9,
            width: 30,
            height: 4,
        };
        if let Some(path) = rounded_rect_path(foot, 2) {
            paint_fill(canvas, &path, monitor_color);
        }

        paint_text(
            canvas,
            &self.name,
            area.x + 112,
            area.y + 26,
            14.0,
            if self.focused {
                self.accent
            } else {
                theme.palette.text
            },
        );

        let badges = display_badge_text(self.focused, self.primary);
        paint_text(
            canvas,
            &badges,
            area.x + 112,
            area.y + 45,
            11.0,
            if self.focused {
                self.accent
            } else {
                theme.palette.text_dim
            },
        );

        let refresh = self
            .refresh_millihz
            .map(|millihz| format!("{:.2} Hz", millihz as f32 / 1000.0))
            .unwrap_or_else(|| "refresh n/a".to_string());
        let transform = self.transform.as_deref().unwrap_or("transform n/a");
        let mode = if self.width > 0 && self.height > 0 {
            format!("{} x {} @ {}", self.width, self.height, refresh)
        } else {
            "geometry n/a".to_string()
        };
        paint_text(
            canvas,
            &mode,
            area.x + 112,
            area.y + 65,
            12.0,
            theme.palette.text,
        );

        let details = if self.width > 0 && self.height > 0 {
            format!(
                "pos {},{} · scale {:.2} · {} · workspace {} · id {} · {} modes",
                self.x,
                self.y,
                self.scale_millis as f32 / 1000.0,
                transform,
                self.workspace.clamp(1, 9),
                self.output_id,
                self.mode_count
            )
        } else {
            format!(
                "scale {:.2} · {} · workspace {} · id {} · {} modes",
                self.scale_millis as f32 / 1000.0,
                transform,
                self.workspace.clamp(1, 9),
                self.output_id,
                self.mode_count
            )
        };
        paint_text(
            canvas,
            &details,
            area.x + 112,
            area.y + 86,
            10.0,
            theme.palette.text_dim,
        );
    }
}

fn display_badge_text(focused: bool, primary: bool) -> String {
    match (focused, primary) {
        (true, true) => "AKTIV  ·  PRIMÄR".to_string(),
        (true, false) => "AKTIV".to_string(),
        (false, true) => "PRIMÄR".to_string(),
        (false, false) => "VERFÜGBAR".to_string(),
    }
}

fn display_mode_label(mode: &OutputModeState) -> String {
    let refresh = mode
        .refresh_millihz
        .map(|millihz| format!("{:.2} Hz", millihz as f32 / 1000.0))
        .unwrap_or_else(|| "Hz n/a".to_string());
    let suffix = if mode.preferred { " pref" } else { "" };
    format!("{} x {} @ {}{}", mode.width, mode.height, refresh, suffix)
}

fn selected_display_mode(output: &OutputWorkspaceState) -> Option<&OutputModeState> {
    output
        .modes
        .iter()
        .find(|mode| mode.current)
        .or_else(|| {
            output.modes.iter().find(|mode| {
                mode.width == output.width
                    && mode.height == output.height
                    && mode.refresh_millihz == output.refresh_millihz
            })
        })
        .or_else(|| output.modes.first())
}
