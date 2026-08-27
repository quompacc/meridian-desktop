fn build_users_content(ctx: &SettingsContentContext<'_>) -> Box<dyn Widget> {
    let row_w = settings_group_inner_width(ctx.content_w);
    let rows: Vec<Box<dyn Widget>> = crate::users::UserAccounts::gather()
        .rows()
        .into_iter()
        .map(|(label, value)| {
            Box::new(SystemInfoRow {
                label: label.into(),
                value: value.into(),
                row_width: row_w,
            }) as Box<dyn Widget>
        })
        .collect();
    build_settings_group_page(
        ctx.content_w,
        ctx.content_h,
        "Lokale Konten",
        "Benutzer und ihre aktive Meridian-Sitzung.",
        Box::new(Container::column(4, rows)),
    )
}
