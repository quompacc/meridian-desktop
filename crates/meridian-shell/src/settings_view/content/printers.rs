fn build_printers_content(ctx: &SettingsContentContext<'_>) -> Box<dyn Widget> {
    let row_w = settings_group_inner_width(ctx.content_w);
    let mut rows: Vec<Box<dyn Widget>> = vec![Box::new(PrinterSummaryCard {
        snapshot: ctx.printer_snapshot.clone(),
        row_width: row_w,
        accent: ctx.pal.accent,
    }) as Box<dyn Widget>];

    if ctx.printer_snapshot.service != PrinterServiceState::Running {
        rows.push(Box::new(SettingsPlaceholder {
            width: row_w,
            text: printer_service_message(ctx.printer_snapshot.service),
        }));
    } else if ctx.printer_snapshot.printers.is_empty() {
        rows.push(Box::new(SettingsPlaceholder {
            width: row_w,
            text: "No printers configured yet",
        }));
    } else {
        for printer in ctx.printer_snapshot.printers.iter().take(PRINTER_MAX) {
            rows.push(Box::new(PrinterRow {
                printer: printer.clone(),
                row_width: row_w,
                accent: ctx.pal.accent,
            }));
        }
    }

    build_settings_group_page(
        ctx.content_w,
        ctx.content_h,
        "Drucker",
        "Druckdienste, Warteschlangen und konfigurierte Geräte.",
        Box::new(Container::column(4, rows)),
    )
}
