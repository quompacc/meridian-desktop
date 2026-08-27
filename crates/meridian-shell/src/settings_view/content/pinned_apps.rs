fn build_pinned_apps_content(ctx: &SettingsContentContext<'_>) -> Box<dyn Widget> {
    let row_w = settings_group_inner_width(ctx.content_w);
    let body = if ctx.pinned_adding {
        // ── Add-app sub-view ──
        let pinned_programs: std::collections::HashSet<&str> =
            ctx.pinned_apps.iter().map(|p| p.program.as_str()).collect();
        let mut addable: Vec<&DesktopApp> = ctx
            .all_apps
            .iter()
            .filter(|a| !pinned_programs.contains(a.program.as_str()))
            .collect();
        addable.sort_by(|a, b| a.name.cmp(&b.name));

        let back_btn = Box::new(Button::with_id(
            "pinned-add-close",
            "← Zurück",
            ctx.pal.accent,
            row_w,
            36,
        )) as Box<dyn Widget>;

        let rows: Vec<Box<dyn Widget>> = addable
            .iter()
            .take(PINNED_ADD_IDS.len())
            .enumerate()
            .map(|(i, app)| {
                let row_icon = app
                    .icon_name
                    .as_deref()
                    .and_then(|n| ctx.icon_cache.lookup(n, 24))
                    .and_then(icon_image_to_pixmap);
                Box::new(AddAppRow {
                    index: i,
                    name: app.name.as_str().into(),
                    accent: ctx.pal.text,
                    row_width: row_w,
                    icon: row_icon,
                }) as Box<dyn Widget>
            })
            .collect();

        Box::new(Container::column(
            4,
            vec![back_btn, Box::new(Container::column(2, rows))],
        )) as Box<dyn Widget>
    } else {
        // ── Normal pinned list ──
        let count = ctx.pinned_apps.len().min(PINNED_MAX);

        let add_btn = Box::new(Button::with_id(
            "pinned-add-open",
            "+ Anwendung hinzufügen",
            ctx.pal.accent,
            row_w,
            36,
        )) as Box<dyn Widget>;

        if count == 0 {
            let placeholder = Box::new(SettingsPlaceholder {
                width: row_w,
                text: "Keine angehefteten Anwendungen.",
            }) as Box<dyn Widget>;
            Box::new(Container::column(4, vec![add_btn, placeholder])) as Box<dyn Widget>
        } else {
            let rows: Vec<Box<dyn Widget>> = ctx
                .pinned_apps
                .iter()
                .take(PINNED_MAX)
                .enumerate()
                .map(|(i, app)| {
                    let label_w = row_w - PINNED_BTN_W * 3;
                    let is_first = i == 0;
                    let is_last = i + 1 == count;
                    let up_color = if is_first {
                        ctx.pal.text_dim
                    } else {
                        ctx.pal.accent
                    };
                    let dn_color = if is_last {
                        ctx.pal.text_dim
                    } else {
                        ctx.pal.accent
                    };
                    let app_icon = app
                        .icon_name
                        .as_deref()
                        .and_then(|n| ctx.icon_cache.lookup(n, 24))
                        .and_then(icon_image_to_pixmap);
                    let label = Box::new(PinnedAppLabel {
                        label: app.label.clone().into(),
                        program: app.program.clone().into(),
                        width: label_w,
                        icon: app_icon,
                    }) as Box<dyn Widget>;
                    let btn_up = Box::new(Button::with_id(
                        PINNED_UP_IDS[i],
                        "↑",
                        up_color,
                        PINNED_BTN_W,
                        PINNED_ROW_H,
                    )) as Box<dyn Widget>;
                    let btn_dn = Box::new(Button::with_id(
                        PINNED_DN_IDS[i],
                        "↓",
                        dn_color,
                        PINNED_BTN_W,
                        PINNED_ROW_H,
                    )) as Box<dyn Widget>;
                    let btn_rm = Box::new(Button::with_id(
                        PINNED_RM_IDS[i],
                        "×",
                        ctx.pal.error,
                        PINNED_BTN_W,
                        PINNED_ROW_H,
                    )) as Box<dyn Widget>;
                    Box::new(Container::row(0, vec![label, btn_up, btn_dn, btn_rm]))
                        as Box<dyn Widget>
                })
                .collect();

            Box::new(Container::column(
                4,
                vec![add_btn, Box::new(Container::column(2, rows))],
            )) as Box<dyn Widget>
        }
    };
    build_settings_group_page(
        ctx.content_w,
        ctx.content_h,
        "Angeheftete Anwendungen",
        "Reihenfolge und Auswahl der Anwendungen im Panel.",
        body,
    )
}
