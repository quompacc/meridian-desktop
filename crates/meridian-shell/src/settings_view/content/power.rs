fn build_power_content(ctx: &SettingsContentContext<'_>) -> Box<dyn Widget> {
    let row_w = settings_group_inner_width(ctx.content_w);
    let btn = |id: &'static str, label: &'static str, color| {
        Box::new(Button::with_id(id, label, color, row_w, 44)) as Box<dyn Widget>
    };
    // Idle screen-blank timeout — adjustable: one chip per option, the
    // active timeout accented. Sits above the power-action buttons.
    let idle_chips: Vec<Box<dyn Widget>> = IDLE_TIMEOUT_OPTIONS
        .iter()
        .map(|(secs, id, label)| {
            let accent = if *secs == ctx.idle_timeout_secs {
                ctx.pal.accent
            } else {
                ctx.pal.surface
            };
            Box::new(Button::with_id(id, label, accent, 80, 32)) as Box<dyn Widget>
        })
        .collect();
    build_settings_group_page(
        ctx.content_w,
        ctx.content_h,
        "Leerlauf und Sitzung",
        "Bildschirmverhalten und sichere Sitzungsaktionen.",
        Box::new(Container::column(
            8,
            vec![
                Box::new(SidebarSectionLabel {
                    text: "BILDSCHIRM-LEERLAUF",
                    width: row_w,
                    pad_top: 0,
                }) as Box<dyn Widget>,
                Box::new(Container::row(8, idle_chips)) as Box<dyn Widget>,
                Box::new(SidebarSectionLabel {
                    text: "SITZUNG",
                    width: row_w,
                    pad_top: 8,
                }) as Box<dyn Widget>,
                btn("power-sleep", "Bereitschaft", ctx.pal.accent),
                btn("power-lock", "Sperren", ctx.pal.accent),
                btn("power-logout", "Abmelden", ctx.pal.accent),
                btn("power-restart", "Neu starten", ctx.pal.error),
                btn("power-off", "Ausschalten", ctx.pal.error),
            ],
        )),
    )
}
