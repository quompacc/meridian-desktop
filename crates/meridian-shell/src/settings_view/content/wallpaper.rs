fn build_wallpaper_content(ctx: &SettingsContentContext<'_>) -> Box<dyn Widget> {
    let row_w = settings_group_inner_width(ctx.content_w);
    let body_h = settings_group_body_height(ctx.content_h);
    let mode_chips: Vec<Box<dyn Widget>> = [
        ("wallpaper-mode-fill", "Fill", WallpaperMode::Fill),
        ("wallpaper-mode-fit", "Fit", WallpaperMode::Fit),
        ("wallpaper-mode-center", "Center", WallpaperMode::Center),
        ("wallpaper-mode-tile", "Tile", WallpaperMode::Tile),
    ]
    .iter()
    .map(|(id, label, mode)| {
        let accent = if *mode == ctx.wallpaper_mode {
            ctx.pal.accent
        } else {
            ctx.pal.surface
        };
        Box::new(Button::with_id(id, label, accent, 80, 32)) as Box<dyn Widget>
    })
    .collect();
    let mode_bar = Container::centered_viewport(
        row_w as u32,
        WALLPAPER_MODE_BAR_H,
        vec![Box::new(Container::row(8, mode_chips)) as Box<dyn Widget>],
    );
    let list_h = body_h.saturating_sub(WALLPAPER_MODE_BAR_H);
    let max_visible = ((list_h + 2) / (WALLPAPER_ROW_H as u32 + 2))
        .min(WALLPAPER_WIDGET_IDS.len() as u32) as usize;
    let mut rows: Vec<Box<dyn Widget>> = Vec::new();
    rows.push(Box::new(WallpaperBrowseRow {
        row_width: row_w,
        accent: ctx.pal.accent,
    }));
    let entry_slots = max_visible.saturating_sub(1);
    if ctx.available_wallpapers.is_empty() {
        rows.push(Box::new(SettingsPlaceholder {
            width: row_w,
            text: "No wallpapers found in /usr/share/wallpapers or ~/Pictures",
        }));
    } else {
        // Filter by query only when a wallpaper name matches; keep the
        // original index so thumbnails and click ids stay aligned.
        let name_hit = !ctx.query.is_empty()
            && ctx
                .available_wallpapers
                .iter()
                .any(|w| w.display_name.to_lowercase().contains(ctx.query));
        let mut shown = 0usize;
        for (i, entry) in ctx.available_wallpapers.iter().enumerate() {
            if name_hit && !entry.display_name.to_lowercase().contains(ctx.query) {
                continue;
            }
            if shown >= entry_slots {
                break;
            }
            let thumbnail = ctx.wallpaper_thumbnails.get(i).and_then(|t| t.clone());
            rows.push(Box::new(WallpaperRow {
                index: i,
                display_name: entry.display_name.as_str().into(),
                thumbnail,
                is_selected: ctx.current_wallpaper == Some(entry.apply_path.as_str()),
                accent: ctx.pal.accent,
                row_width: row_w,
            }));
            shown += 1;
        }
    }
    build_settings_group_page(
        ctx.content_w,
        ctx.content_h,
        "Hintergrund",
        "Das Bild bleibt der visuelle Mittelpunkt des Desktops.",
        Box::new(Container::column(
            2,
            vec![
                Box::new(mode_bar) as Box<dyn Widget>,
                Box::new(Container::column(2, rows)) as Box<dyn Widget>,
            ],
        )),
    )
}
