struct NetworkProfileRow {
    index: usize,
    name: Box<str>,
    type_label: Box<str>,
    active: bool,
    accent: Color,
    row_width: i32,
}

impl Widget for NetworkProfileRow {
    fn id(&self) -> Option<&'static str> {
        if self.active {
            None
        } else {
            NETWORK_PROFILE_IDS.get(self.index).copied()
        }
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(NETWORK_PROFILE_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = if self.active {
            theme.palette.surface
        } else {
            match state {
                WidgetState::Idle => theme.palette.surface,
                WidgetState::Hovered => Interaction::DEFAULT.hover(theme.palette.surface),
                WidgetState::Pressed => Interaction::DEFAULT.pressed(theme.palette.surface),
            }
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        if self.active {
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
        paint_text(
            canvas,
            &fit_text(&self.name, 36),
            area.x + 18,
            area.y + 24,
            13.5,
            theme.palette.text,
        );
        paint_text(
            canvas,
            &self.type_label,
            area.x + 18,
            area.y + 44,
            11.5,
            theme.palette.text_dim,
        );
        if self.active {
            paint_text(
                canvas,
                "VERBUNDEN",
                area.x + area.width - 104,
                area.y + 24,
                10.5,
                self.accent,
            );
        } else if state != WidgetState::Idle {
            paint_text(
                canvas,
                "Verbinden",
                area.x + area.width - 96,
                area.y + 24,
                10.5,
                theme.palette.text_dim,
            );
        }
    }
}

/// A clickable scanned Wi-Fi row. The in-use network shows a "VERBUNDEN" badge
/// and is inert; others are clickable to connect. Secured networks show a lock
/// glyph and the signal strength; clicking a secured unknown network opens the
/// password prompt (handled in the dispatch).
struct WifiRow {
    index: usize,
    ssid: Box<str>,
    signal: u8,
    secured: bool,
    in_use: bool,
    accent: Color,
    row_width: i32,
}

impl Widget for WifiRow {
    fn id(&self) -> Option<&'static str> {
        if self.in_use {
            None
        } else {
            WIFI_NETWORK_IDS.get(self.index).copied()
        }
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(NETWORK_PROFILE_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = if self.in_use {
            theme.palette.surface
        } else {
            match state {
                WidgetState::Idle => theme.palette.surface,
                WidgetState::Hovered => Interaction::DEFAULT.hover(theme.palette.surface),
                WidgetState::Pressed => Interaction::DEFAULT.pressed(theme.palette.surface),
            }
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        if self.in_use {
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
        paint_text(
            canvas,
            &fit_text(&self.ssid, 32),
            area.x + 18,
            area.y + 24,
            13.5,
            theme.palette.text,
        );
        // Sub-line: security + signal strength (text marker, not emoji — the
        // shell font has no emoji glyphs).
        let lock = if self.secured { "gesichert" } else { "offen" };
        paint_text(
            canvas,
            &format!("{} · Signal {}%", lock, self.signal),
            area.x + 18,
            area.y + 44,
            11.5,
            theme.palette.text_dim,
        );
        if self.in_use {
            paint_text(
                canvas,
                "VERBUNDEN",
                area.x + area.width - 104,
                area.y + 24,
                10.5,
                self.accent,
            );
        } else if state != WidgetState::Idle {
            paint_text(
                canvas,
                "Verbinden",
                area.x + area.width - 96,
                area.y + 24,
                10.5,
                theme.palette.text_dim,
            );
        }
    }
}

/// A clickable Bluetooth device row. A connected device shows a "VERBUNDEN"
/// badge and is inert; a paired-but-disconnected device shows "Gekoppelt" and
/// connects on click; an unknown device shows its address and pairs on click.
struct BluetoothDeviceRow {
    index: usize,
    name: Box<str>,
    address: Box<str>,
    paired: bool,
    connected: bool,
    accent: Color,
    row_width: i32,
}

impl Widget for BluetoothDeviceRow {
    fn id(&self) -> Option<&'static str> {
        if self.connected {
            None
        } else {
            BT_DEVICE_IDS.get(self.index).copied()
        }
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(NETWORK_PROFILE_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = if self.connected {
            theme.palette.surface
        } else {
            match state {
                WidgetState::Idle => theme.palette.surface,
                WidgetState::Hovered => Interaction::DEFAULT.hover(theme.palette.surface),
                WidgetState::Pressed => Interaction::DEFAULT.pressed(theme.palette.surface),
            }
        };
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        if self.connected {
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
        let title = if self.name.is_empty() {
            self.address.clone()
        } else {
            self.name.clone()
        };
        paint_text(
            canvas,
            &fit_text(&title, 32),
            area.x + 18,
            area.y + 24,
            13.5,
            theme.palette.text,
        );
        let sub = if self.paired { "gekoppelt" } else { "neu" };
        paint_text(
            canvas,
            &format!("{} · {}", sub, self.address),
            area.x + 18,
            area.y + 44,
            11.5,
            theme.palette.text_dim,
        );
        if self.connected {
            paint_text(
                canvas,
                "VERBUNDEN",
                area.x + area.width - 104,
                area.y + 24,
                10.5,
                self.accent,
            );
        } else if state != WidgetState::Idle {
            let hint = if self.paired { "Verbinden" } else { "Koppeln" };
            paint_text(
                canvas,
                hint,
                area.x + area.width - 96,
                area.y + 24,
                10.5,
                theme.palette.text_dim,
            );
        }
    }
}

struct PrinterRow {
    printer: PrinterInfo,
    row_width: i32,
    accent: Color,
}

impl Widget for PrinterRow {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(PRINTER_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, theme.palette.surface);
        }
        if self.printer.is_default {
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

        let title = fit_text(&self.printer.name, 42);
        paint_text(
            canvas,
            &title,
            area.x + 18,
            area.y + 26,
            13.5,
            theme.palette.text,
        );
        if self.printer.is_default {
            paint_text(
                canvas,
                "DEFAULT",
                area.x + area.width - 88,
                area.y + 26,
                10.5,
                self.accent,
            );
        }

        let accepting = match self.printer.accepting {
            Some(true) => "accepting",
            Some(false) => "not accepting",
            None => "accepting unknown",
        };
        let state = if self.printer.enabled {
            "enabled"
        } else {
            "disabled"
        };
        let detail = format!(
            "{} / {} / {} jobs / {}",
            state,
            accepting,
            self.printer.job_count,
            fit_text(&self.printer.status, 48)
        );
        paint_text(
            canvas,
            &detail,
            area.x + 18,
            area.y + 52,
            12.0,
            theme.palette.text_dim,
        );
    }
}
