fn build_system_overview_content(ctx: &SettingsContentContext<'_>) -> Box<dyn Widget> {
    let row_w = ctx.content_w as i32;
    let rows: Vec<Box<dyn Widget>> = ctx
        .system_info
        .rows()
        .iter()
        .map(|(label, value)| {
            Box::new(SystemInfoRow {
                label: (*label).into(),
                value: (*value).into(),
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
