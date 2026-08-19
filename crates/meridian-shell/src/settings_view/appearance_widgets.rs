struct WallpaperRow {
    index: usize,
    display_name: Box<str>,
    thumbnail: Option<(u32, u32, Vec<u8>)>,
    is_selected: bool,
    accent: Color,
    row_width: i32,
}

impl Widget for WallpaperRow {
    fn id(&self) -> Option<&'static str> {
        WALLPAPER_WIDGET_IDS.get(self.index).copied()
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(WALLPAPER_ROW_H as f32),
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
                y: area.y + 6,
                width: 3,
                height: area.height - 12,
            };
            if let Some(path) = rounded_rect_path(strip, 1) {
                paint_fill(canvas, &path, self.accent);
            }
        }
        // Thumbnail on the left (96x54 inside 64px-tall row).
        let thumb_left = area.x + 8;
        if let Some((tw, th, ref data)) = self.thumbnail {
            let thumb_y = area.y + (area.height - th as i32) / 2;
            if let Some(pm_ref) = PixmapRef::from_bytes(data, tw, th) {
                canvas.draw_pixmap(
                    thumb_left,
                    thumb_y,
                    pm_ref,
                    &PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
            }
        } else {
            // Placeholder rectangle when thumbnail not yet loaded.
            let ph = Rect {
                x: thumb_left,
                y: area.y + 5,
                width: WALLPAPER_THUMB_W as i32,
                height: WALLPAPER_THUMB_H as i32,
            };
            if let Some(path) = rounded_rect_path(ph, 2) {
                paint_fill(canvas, &path, theme.palette.surface_alt);
            }
        }
        let text_x = area.x + 8 + WALLPAPER_THUMB_W as i32 + 8;
        let text_color = if self.is_selected {
            self.accent
        } else {
            theme.palette.text
        };
        paint_text(
            canvas,
            &self.display_name,
            text_x,
            area.y + area.height - 16,
            12.0,
            text_color,
        );
    }
}

struct WallpaperBrowseRow {
    row_width: i32,
    accent: Color,
}

impl Widget for WallpaperBrowseRow {
    fn id(&self) -> Option<&'static str> {
        Some("wallpaper-browse")
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.row_width as f32),
                height: ui_length(WALLPAPER_ROW_H as f32),
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
        if let Some(path) = rounded_rect_path(area, THEME_ROW_CORNER) {
            paint_fill(canvas, &path, bg);
        }
        // Icon area — dim filled rectangle with "..." hint.
        let icon = Rect {
            x: area.x + 8,
            y: area.y + (area.height - WALLPAPER_THUMB_H as i32) / 2,
            width: WALLPAPER_THUMB_W as i32,
            height: WALLPAPER_THUMB_H as i32,
        };
        if let Some(path) = rounded_rect_path(icon, 4) {
            paint_fill(
                canvas,
                &path,
                Interaction::DEFAULT.darken(self.accent, 0.55),
            );
        }
        paint_text(
            canvas,
            "\u{2026}",
            icon.x + icon.width / 2 - 6,
            icon.y + icon.height - 8,
            14.0,
            self.accent,
        );
        let text_x = area.x + 8 + WALLPAPER_THUMB_W as i32 + 8;
        paint_text(
            canvas,
            "Browse for image\u{2026}",
            text_x,
            area.y + area.height - 16,
            12.0,
            self.accent,
        );
    }
}

struct PinnedAppLabel {
    label: Box<str>,
    program: Box<str>,
    width: i32,
    icon: Option<Pixmap>,
}

impl Widget for PinnedAppLabel {
    fn id(&self) -> Option<&'static str> {
        None
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length(PINNED_ROW_H as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, 0) {
            paint_fill(canvas, &path, theme.palette.surface);
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
            &self.label,
            text_x,
            area.y + 16,
            13.0,
            theme.palette.text,
        );
        paint_text(
            canvas,
            &self.program,
            text_x,
            area.y + 32,
            11.0,
            theme.palette.text_dim,
        );
    }
}

struct SettingsPlaceholder {
    width: i32,
    text: &'static str,
}

impl Widget for SettingsPlaceholder {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length(60.0),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        paint_text(
            canvas,
            self.text,
            area.x + 20,
            area.y + 36,
            13.0,
            theme.palette.text_dim,
        );
    }
}
