fn build_theme_content(ctx: &SettingsContentContext<'_>) -> Box<dyn Widget> {
    let group_w = ctx.content_w as i32 - SETTINGS_CHROME.content_pad * 2;
    let options_w = group_w - SETTINGS_CHROME.group_pad * 2;
    let option_w = (options_w - SETTINGS_CHROME.option_gap) / 2;
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
        .take(2)
        .enumerate()
        .filter(|(_, name)| !name_hit || name.to_lowercase().contains(ctx.query))
        .map(|(i, name)| {
            Box::new(ThemeOption {
                index: i,
                name: name.as_str().into(),
                preview_is_light: name.to_lowercase().contains("light")
                    || name.to_lowercase().contains("hell"),
                is_selected: name.as_str() == ctx.current_theme,
                accent: ctx.pal.accent,
                width: option_w,
            }) as Box<dyn Widget>
        })
        .collect();
    let options = Box::new(Container::row(SETTINGS_CHROME.option_gap, rows)) as Box<dyn Widget>;
    let group = Box::new(SettingsGroupPanel {
        width: group_w,
        height: SETTINGS_CHROME.appearance_group_height,
        children: vec![
            Box::new(SettingsGroupHeading {
                width: options_w,
                title: "Oberfläche",
                description: "Helle und dunkle Darstellung verwenden dieselben Proportionen und Effekte.",
            }) as Box<dyn Widget>,
            options,
        ],
    }) as Box<dyn Widget>;
    Box::new(Container::top_viewport(
        ctx.content_w,
        ctx.content_h,
        0,
        SETTINGS_CHROME.content_pad,
        0,
        vec![group],
    ))
}
