struct SettingsGroupPanel {
    width: i32,
    height: i32,
    children: Vec<Box<dyn Widget>>,
}

fn settings_group_width(content_width: u32) -> i32 {
    content_width as i32 - SETTINGS_CHROME.content_pad * 2
}

fn settings_group_inner_width(content_width: u32) -> i32 {
    settings_group_width(content_width) - SETTINGS_CHROME.group_pad * 2
}

fn settings_group_body_height(content_height: u32) -> u32 {
    content_height.saturating_sub(
        (SETTINGS_CHROME.group_pad * 2
            + SETTINGS_CHROME.group_heading_height
            + SETTINGS_CHROME.group_gap) as u32,
    )
}

fn build_settings_group_page(
    content_width: u32,
    content_height: u32,
    title: &'static str,
    description: &'static str,
    body: Box<dyn Widget>,
) -> Box<dyn Widget> {
    let group_width = settings_group_width(content_width);
    let inner_width = settings_group_inner_width(content_width);
    let group = Box::new(SettingsGroupPanel {
        width: group_width,
        height: 0,
        children: vec![
            Box::new(SettingsGroupHeading {
                width: inner_width,
                title,
                description,
            }) as Box<dyn Widget>,
            body,
        ],
    }) as Box<dyn Widget>;
    Box::new(Container::top_viewport(
        content_width,
        content_height,
        0,
        SETTINGS_CHROME.content_pad,
        0,
        vec![group],
    ))
}

impl Widget for SettingsGroupPanel {
    fn style(&self) -> WidgetStyle {
        let mut style = WidgetStyle {
            flex_direction: FlexDirection::Column,
            gap: UiSize {
                width: ui_length(0.0),
                height: ui_length(SETTINGS_CHROME.group_gap as f32),
            },
            padding: TaffyRect {
                left: ui_length(SETTINGS_CHROME.group_pad as f32),
                right: ui_length(SETTINGS_CHROME.group_pad as f32),
                top: ui_length(SETTINGS_CHROME.group_pad as f32),
                bottom: ui_length(SETTINGS_CHROME.group_pad as f32),
            },
            ..Default::default()
        };
        style.size.width = ui_length(self.width as f32);
        if self.height > 0 {
            style.size.height = ui_length(self.height as f32);
        }
        style
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        if let Some(path) = rounded_rect_path(area, theme.radius.lg) {
            paint_fill(canvas, &path, theme.palette.surface_alt);
            paint_border(canvas, &path, theme.palette.border, 1.0);
        }
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }
}

struct SettingsGroupHeading {
    width: i32,
    title: &'static str,
    description: &'static str,
}

impl Widget for SettingsGroupHeading {
    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length(SETTINGS_CHROME.group_heading_height as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, _state: WidgetState) {
        paint_text(
            canvas,
            self.title,
            area.x,
            area.y + Typography::DEFAULT.title_size as i32,
            Typography::DEFAULT.title_size as f32,
            theme.palette.text,
        );
        paint_text(
            canvas,
            self.description,
            area.x,
            area.y + SETTINGS_CHROME.group_heading_height,
            Typography::DEFAULT.caption_size as f32,
            theme.palette.text_dim,
        );
    }
}

struct ThemeOption {
    index: usize,
    name: Box<str>,
    preview_is_light: bool,
    is_selected: bool,
    accent: Color,
    width: i32,
}

impl Widget for ThemeOption {
    fn id(&self) -> Option<&'static str> {
        THEME_WIDGET_IDS.get(self.index).copied()
    }

    fn style(&self) -> WidgetStyle {
        WidgetStyle {
            size: UiSize {
                width: ui_length(self.width as f32),
                height: ui_length(SETTINGS_CHROME.theme_option_height as f32),
            },
            ..Default::default()
        }
    }

    fn paint(&self, area: Rect, canvas: &mut PixmapMut<'_>, theme: &Theme, state: WidgetState) {
        let base = if self.is_selected {
            Interaction::DEFAULT.selected_tint(theme.palette.surface)
        } else {
            theme.palette.surface
        };
        let bg = match state {
            WidgetState::Idle => base,
            WidgetState::Hovered => Interaction::DEFAULT.hover(base),
            WidgetState::Pressed => Interaction::DEFAULT.pressed(base),
        };
        if let Some(path) = rounded_rect_path(area, theme.radius.md) {
            paint_fill(canvas, &path, bg);
            paint_border(
                canvas,
                &path,
                if self.is_selected { self.accent } else { theme.palette.border },
                1.0,
            );
        }

        let preview = if self.preview_is_light {
            meridian_tokens::Palette::LIGHT
        } else {
            meridian_tokens::Palette::DARK
        };
        let preview_area = Rect {
            x: area.x + SETTINGS_CHROME.group_pad,
            y: area.y + (area.height - SETTINGS_CHROME.theme_preview_height) / 2,
            width: SETTINGS_CHROME.theme_preview_width,
            height: SETTINGS_CHROME.theme_preview_height,
        };
        if let Some(path) = rounded_rect_path(preview_area, theme.radius.sm) {
            paint_fill(canvas, &path, preview.background);
            paint_border(canvas, &path, preview.border, 1.0);
        }
        let nav = Rect {
            x: preview_area.x,
            y: preview_area.y,
            width: SETTINGS_CHROME.group_gap,
            height: preview_area.height,
        };
        if let Some(path) = rounded_rect_path(nav, theme.radius.sm) {
            paint_fill(canvas, &path, preview.surface_alt);
        }
        let header = Rect {
            x: preview_area.x + SETTINGS_CHROME.group_gap,
            y: preview_area.y,
            width: preview_area.width - SETTINGS_CHROME.group_gap,
            height: SETTINGS_CHROME.group_gap,
        };
        if let Some(path) = rounded_rect_path(header, 0) {
            paint_fill(canvas, &path, preview.surface_alt);
        }
        paint_text(
            canvas,
            &self.name,
            preview_area.x + preview_area.width + SETTINGS_CHROME.group_gap,
            area.y + (area.height + Typography::DEFAULT.body_size as i32) / 2,
            Typography::DEFAULT.body_size as f32,
            theme.palette.text,
        );
    }
}
