fn build_cursor_content(ctx: &SettingsContentContext<'_>) -> Box<dyn Widget> {
    let row_w = ctx.content_w as i32;
    let mut rows: Vec<Box<dyn Widget>> = Vec::new();

    // Size is adjustable: one chip per option, the active size accented.
    rows.push(Box::new(SidebarSectionLabel {
        text: "GRÖSSE",
        width: row_w,
        pad_top: 0,
    }));
    let size_chips: Vec<Box<dyn Widget>> = crate::cursor::CURSOR_SIZE_OPTIONS
        .iter()
        .map(|(px, id, label)| {
            let accent = if *px == ctx.cursor_size {
                ctx.pal.accent
            } else {
                ctx.pal.surface
            };
            Box::new(Button::with_id(id, label, accent, 72, 32)) as Box<dyn Widget>
        })
        .collect();
    rows.push(Box::new(Container::row(8, size_chips)));

    // Theme is selectable: one row per installed cursor theme, the
    // active one accented. The list is bounded to the id-array length.
    rows.push(Box::new(SidebarSectionLabel {
        text: "THEME",
        width: row_w,
        pad_top: 12,
    }));
    if ctx.available_cursor_themes.is_empty() {
        rows.push(Box::new(SettingsPlaceholder {
            width: row_w,
            text: "No cursor themes found under /usr/share/icons or ~/.icons",
        }));
    } else {
        for (i, name) in ctx
            .available_cursor_themes
            .iter()
            .take(crate::cursor::CURSOR_THEME_WIDGET_IDS.len())
            .enumerate()
        {
            rows.push(Box::new(CursorThemeRow {
                index: i,
                name: name.as_str().into(),
                is_selected: name == ctx.current_cursor_theme,
                accent: ctx.pal.accent,
                row_width: row_w,
            }));
        }
    }
    Box::new(Container::top_viewport(
        ctx.content_w,
        ctx.content_h,
        14,
        16,
        8,
        vec![Box::new(Container::column(8, rows)) as Box<dyn Widget>],
    ))
}
