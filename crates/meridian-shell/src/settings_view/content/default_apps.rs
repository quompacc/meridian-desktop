fn build_default_apps_content(ctx: &SettingsContentContext<'_>) -> Box<dyn Widget> {
    let row_w = settings_group_inner_width(ctx.content_w);
    let mut rows: Vec<Box<dyn Widget>> = Vec::new();
    if let Some(open_cat) = ctx.default_apps_picker_open {
        // Accordion mode: only the expanded category + its candidates
        // + a back button. Avoids needing a scrollable viewport.
        rows.push(Box::new(Button::with_id(
            "default-apps-back",
            "← Zurück zur Übersicht",
            ctx.pal.surface,
            row_w,
            32,
        )));
        rows.push(Box::new(SidebarSectionLabel {
            text: "STANDARD-APPS",
            width: row_w,
            pad_top: 8,
        }));
        if let Some(index) = ctx.default_apps_index {
            let cat_idx = crate::default_apps::DefaultAppCategory::ALL
                .iter()
                .position(|c| *c == open_cat)
                .unwrap_or(0);
            let current_id = ctx.default_apps_current.get(&open_cat).cloned();
            let current_name = current_id
                .as_deref()
                .and_then(|id| index.lookup(id))
                .map(|app| app.name.clone().into_boxed_str())
                .or_else(|| current_id.clone().map(String::into_boxed_str));
            rows.push(Box::new(DefaultAppCategoryRow {
                index: cat_idx,
                label: open_cat.label(),
                current_app_name: current_name,
                is_expanded: true,
                accent: ctx.pal.accent,
                row_width: row_w,
            }));
            let candidates = index.apps_for_mime(open_cat.representative_mime());
            if candidates.is_empty() {
                rows.push(Box::new(SettingsPlaceholder {
                    width: row_w,
                    text: "Keine installierte Anwendung kann diesen Dateityp öffnen",
                }));
            } else {
                for (app_idx, app) in candidates
                    .iter()
                    .enumerate()
                    .take(crate::settings_view::DEFAULT_APP_MAX_APPS_PER_CAT)
                {
                    let is_current = current_id.as_deref() == Some(app.desktop_id.as_str());
                    rows.push(Box::new(DefaultAppCandidateRow {
                        cat_idx,
                        app_idx,
                        name: app.name.clone().into_boxed_str(),
                        is_current,
                        accent: ctx.pal.accent,
                        row_width: row_w,
                    }));
                }
            }
        }
    } else {
        // Overview: auto-defaults button + every category row.
        rows.push(Box::new(Button::with_id(
            "default-apps-auto",
            "Sinnvolle Defaults für leere Kategorien setzen",
            ctx.pal.accent,
            row_w,
            32,
        )));
        rows.push(Box::new(SidebarSectionLabel {
            text: "STANDARD-APPS",
            width: row_w,
            pad_top: 8,
        }));
        if let Some(index) = ctx.default_apps_index {
            for (cat_idx, cat) in crate::default_apps::DefaultAppCategory::ALL
                .iter()
                .enumerate()
            {
                let current_id = ctx.default_apps_current.get(cat).cloned();
                let current_name = current_id
                    .as_deref()
                    .and_then(|id| index.lookup(id))
                    .map(|app| app.name.clone().into_boxed_str())
                    .or_else(|| current_id.clone().map(String::into_boxed_str));
                rows.push(Box::new(DefaultAppCategoryRow {
                    index: cat_idx,
                    label: cat.label(),
                    current_app_name: current_name,
                    is_expanded: false,
                    accent: ctx.pal.accent,
                    row_width: row_w,
                }));
            }
        } else {
            rows.push(Box::new(SettingsPlaceholder {
                width: row_w,
                text: "Lade installierte Anwendungen ...",
            }));
        }
    }
    build_settings_group_page(
        ctx.content_w,
        ctx.content_h,
        "Standardzuordnungen",
        "Anwendungen für häufige Dateitypen und Aufgaben auswählen.",
        Box::new(Container::column(4, rows)),
    )
}
