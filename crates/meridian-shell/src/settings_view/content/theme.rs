fn build_theme_content(ctx: &SettingsContentContext<'_>) -> Box<dyn Widget> {
    let row_w = ctx.content_w as i32;
    // Filter the list by the query only when a theme name actually
    // matches; a keyword-only hit (e.g. "dunkel") keeps the full list.
    let name_hit = !ctx.query.is_empty()
        && ctx
            .available_themes
            .iter()
            .take(THEME_WIDGET_IDS.len())
            .any(|t| t.to_lowercase().contains(ctx.query));
    let rows: Vec<Box<dyn Widget>> = ctx
        .available_themes
        .iter()
        .take(THEME_WIDGET_IDS.len())
        .enumerate()
        .filter(|(_, name)| !name_hit || name.to_lowercase().contains(ctx.query))
        .map(|(i, name)| {
            Box::new(ThemeRow {
                index: i,
                name: name.as_str().into(),
                is_selected: name.as_str() == ctx.current_theme,
                accent: ctx.pal.accent,
                row_width: row_w,
            }) as Box<dyn Widget>
        })
        .collect();
    Box::new(Container::top_viewport(
        ctx.content_w,
        ctx.content_h,
        14,
        16,
        4,
        vec![Box::new(Container::column(4, rows)) as Box<dyn Widget>],
    ))
}
