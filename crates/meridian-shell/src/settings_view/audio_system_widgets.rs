struct SoundSummaryCard {
    snapshot: AudioSnapshot,
    row_width: i32,
    accent: Color,
}

impl Widget for SoundSummaryCard {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(SOUND_SUMMARY_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, theme.palette.surface);
        }
        let service_text = match self.snapshot.service {
            AudioServiceState::Running => "PIPEWIRE RUNNING",
            AudioServiceState::Unavailable => "AUDIO UNAVAILABLE",
        };
        let status_color = match self.snapshot.service {
            AudioServiceState::Running => self.accent,
            AudioServiceState::Unavailable => theme.palette.error,
        };
        paint_text(
            canvas,
            "Sound",
            area.x + 18,
            area.y + 30,
            14.0,
            theme.palette.text,
        );
        paint_text(
            canvas,
            service_text,
            area.x + 18,
            area.y + 52,
            10.5,
            status_color,
        );

        let output_text = self
            .snapshot
            .default_output
            .as_ref()
            .map(|device| format!("Output: {}", fit_text(&device.name, 40)))
            .unwrap_or_else(|| "Output: none".to_string());
        let count_text = format!(
            "{} outputs / {} inputs",
            self.snapshot.outputs.len(),
            self.snapshot.inputs.len()
        );
        paint_text(
            canvas,
            &output_text,
            area.x + 18,
            area.y + 76,
            12.0,
            theme.palette.text_dim,
        );
        paint_text(
            canvas,
            &count_text,
            area.x + area.width - 170,
            area.y + 76,
            12.0,
            theme.palette.text_dim,
        );
    }
}

struct SoundDeviceRow {
    label: &'static str,
    device: AudioDevice,
    /// Position in the snapshot's outputs/inputs list, used to pick the static
    /// click id from `ids`.
    index: usize,
    /// Id array for this row's kind (AUDIO_OUTPUT_IDS / AUDIO_INPUT_IDS).
    ids: &'static [&'static str],
    row_width: i32,
    accent: Color,
}

impl Widget for SoundDeviceRow {
    fn id(&self) -> Option<&'static str> {
        // The already-default device is not clickable (nothing to change).
        if self.device.is_default {
            None
        } else {
            self.ids.get(self.index).copied()
        }
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(SOUND_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        // Non-default rows react to hover/press so the "click to make default"
        // affordance is visible; the default row stays inert.
        let bg = if self.device.is_default {
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
        if self.device.is_default {
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

        let title = format!("{} {}", self.label, fit_text(&self.device.name, 42));
        paint_text(
            canvas,
            &title,
            area.x + 18,
            area.y + 26,
            13.5,
            theme.palette.text,
        );
        // Right-aligned status: DEFAULT badge, or a click hint on hover.
        if self.device.is_default {
            paint_text(
                canvas,
                "DEFAULT",
                area.x + area.width - 88,
                area.y + 26,
                10.5,
                self.accent,
            );
        } else if state != WidgetState::Idle {
            paint_text(
                canvas,
                "Auswählen",
                area.x + area.width - 96,
                area.y + 26,
                10.5,
                theme.palette.text_dim,
            );
        }

        let volume = self
            .device
            .volume_percent
            .map(|value| format!("{}%", value))
            .unwrap_or_else(|| "volume unknown".to_string());
        let detail = if self.device.muted {
            format!("id {} / {} / muted", self.device.id, volume)
        } else {
            format!("id {} / {}", self.device.id, volume)
        };
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

struct PrinterSummaryCard {
    snapshot: PrinterSnapshot,
    row_width: i32,
    accent: Color,
}

impl Widget for PrinterSummaryCard {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(PRINTER_SUMMARY_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, theme.palette.surface);
        }

        let service_text = match self.snapshot.service {
            PrinterServiceState::Running => "CUPS RUNNING",
            PrinterServiceState::Stopped => "CUPS STOPPED",
            PrinterServiceState::Unavailable => "CUPS UNAVAILABLE",
        };
        let status_color = match self.snapshot.service {
            PrinterServiceState::Running => self.accent,
            PrinterServiceState::Stopped | PrinterServiceState::Unavailable => theme.palette.error,
        };
        paint_text(
            canvas,
            "Printers",
            area.x + 18,
            area.y + 30,
            14.0,
            theme.palette.text,
        );
        paint_text(
            canvas,
            service_text,
            area.x + 18,
            area.y + 52,
            10.5,
            status_color,
        );

        let default_text = self
            .snapshot
            .default_printer
            .as_deref()
            .map(|name| format!("Default: {}", fit_text(name, 44)))
            .unwrap_or_else(|| "Default: none".to_string());
        let queue_text = format!(
            "{} configured / {} queued",
            self.snapshot.printers.len(),
            self.snapshot.job_count
        );
        paint_text(
            canvas,
            &default_text,
            area.x + 18,
            area.y + 76,
            12.0,
            theme.palette.text_dim,
        );
        paint_text(
            canvas,
            &queue_text,
            area.x + area.width - 190,
            area.y + 76,
            12.0,
            theme.palette.text_dim,
        );
    }
}

const SYSINFO_ROW_H: i32 = 36;

struct SystemInfoRow {
    label: Box<str>,
    value: Box<str>,
    row_width: i32,
}

impl Widget for SystemInfoRow {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(SYSINFO_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, theme.palette.surface);
        }
        paint_text(
            canvas,
            &self.label,
            area.x + 16,
            area.y + 23,
            12.5,
            theme.palette.text_dim,
        );
        let value = fit_text(&self.value, 48);
        paint_text(
            canvas,
            &value,
            area.x + 150,
            area.y + 23,
            12.5,
            theme.palette.text,
        );
    }
}

const NETWORK_PROFILE_ROW_H: i32 = 56;

// A clickable saved-network row. The active profile shows a "VERBUNDEN" badge
// and is inert (`id()` -> None); others are clickable to activate and react to
// hover/press, mirroring `SoundDeviceRow`.
// ─── DefaultApps page widgets ────────────────────────────────────────────────

const DEFAULT_APP_ROW_H: i32 = 42;
const DEFAULT_APP_CANDIDATE_ROW_H: i32 = 32;
const DEFAULT_APP_ROW_CORNER: i32 = 10;
const DEFAULT_APP_MAX_CATS: usize = 12;
const DEFAULT_APP_MAX_APPS_PER_CAT: usize = 24;

/// Stable widget ids for the category rows (the row click toggles the
/// candidate-list expander). Lazily leaked once so the `'static` lifetime
/// the Widget trait wants is satisfied without const-string gymnastics.
fn default_apps_pick_id(idx: usize) -> Option<&'static str> {
    use std::sync::OnceLock;
    static IDS: OnceLock<Vec<&'static str>> = OnceLock::new();
    let ids = IDS.get_or_init(|| {
        (0..DEFAULT_APP_MAX_CATS)
            .map(|i| Box::leak(format!("default-apps-pick-{}", i).into_boxed_str()) as &'static str)
            .collect()
    });
    ids.get(idx).copied()
}

/// Stable widget ids for the candidate rows within each category's picker.
fn default_apps_set_id(cat_idx: usize, app_idx: usize) -> Option<&'static str> {
    use std::sync::OnceLock;
    static IDS: OnceLock<Vec<Vec<&'static str>>> = OnceLock::new();
    let ids = IDS.get_or_init(|| {
        (0..DEFAULT_APP_MAX_CATS)
            .map(|c| {
                (0..DEFAULT_APP_MAX_APPS_PER_CAT)
                    .map(|a| {
                        Box::leak(format!("default-apps-set-{}-{}", c, a).into_boxed_str())
                            as &'static str
                    })
                    .collect()
            })
            .collect()
    });
    ids.get(cat_idx).and_then(|row| row.get(app_idx)).copied()
}

/// A row in the Standard-Apps page: category label on the left, current
/// default's display name on the right, with an inline arrow hinting at
/// the click-to-expand behaviour. Clicking the row toggles the candidate
/// list below.
struct DefaultAppCategoryRow {
    index: usize,
    label: &'static str,
    current_app_name: Option<Box<str>>,
    is_expanded: bool,
    accent: Color,
    row_width: i32,
}

impl Widget for DefaultAppCategoryRow {
    fn id(&self) -> Option<&'static str> {
        default_apps_pick_id(self.index)
    }
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(DEFAULT_APP_ROW_H as f32),
            },
            ..Default::default()
        }
    }
    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let bg = match state {
            WidgetState::Idle => theme.palette.surface,
            WidgetState::Hovered => Interaction::DEFAULT.hover(theme.palette.surface),
            WidgetState::Pressed => Interaction::DEFAULT.pressed(theme.palette.surface),
        };
        if let Some(path) = rounded_rect_path(area, DEFAULT_APP_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        if self.is_expanded {
            let strip = Rect {
                x: area.x,
                y: area.y + 10,
                width: 3,
                height: area.height - 20,
            };
            if let Some(path) = rounded_rect_path(strip, 1) {
                paint_fill(canvas, &path, self.accent);
            }
        }
        paint_text(
            canvas,
            self.label,
            area.x + 18,
            area.y + 24,
            13.5,
            theme.palette.text,
        );
        let current = self.current_app_name.as_deref().unwrap_or("Nicht gesetzt");
        let current_color = if self.current_app_name.is_some() {
            theme.palette.text_dim
        } else {
            theme
                .palette
                .text_dim
                .lerp(theme.palette.error, NOT_SET_ERROR_TINT)
        };
        paint_text(
            canvas,
            &fit_text(current, 28),
            area.x + 18,
            area.y + 33,
            10.5,
            current_color,
        );
        let arrow = if self.is_expanded { "▴" } else { "▾" };
        paint_text(
            canvas,
            arrow,
            area.x + area.width - 26,
            area.y + 26,
            14.0,
            theme.palette.text_dim,
        );
    }
}

/// A single candidate app inside an expanded category picker.
struct DefaultAppCandidateRow {
    cat_idx: usize,
    app_idx: usize,
    name: Box<str>,
    is_current: bool,
    accent: Color,
    row_width: i32,
}

impl Widget for DefaultAppCandidateRow {
    fn id(&self) -> Option<&'static str> {
        default_apps_set_id(self.cat_idx, self.app_idx)
    }
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(DEFAULT_APP_CANDIDATE_ROW_H as f32),
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
        if let Some(path) = rounded_rect_path(area, 6) {
            paint_fill(canvas, &path, bg);
        }
        paint_text(
            canvas,
            &fit_text(&self.name, 44),
            area.x + 32,
            area.y + 21,
            12.0,
            theme.palette.text,
        );
        if self.is_current {
            paint_text(
                canvas,
                "AKTUELL",
                area.x + area.width - 78,
                area.y + 21,
                10.5,
                self.accent,
            );
        }
    }
}
