fn build_system_overview_content(ctx: &SettingsContentContext<'_>) -> Box<dyn Widget> {
    let row_w = settings_group_inner_width(ctx.content_w);
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
    build_settings_group_page(
        ctx.content_w,
        ctx.content_h,
        "Geräteinformationen",
        "Grundlegende Daten dieser Meridian-Installation.",
        Box::new(Container::column(4, rows)),
    )
}
