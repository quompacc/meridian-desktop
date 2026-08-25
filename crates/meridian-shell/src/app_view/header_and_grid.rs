fn draw_header(
    pm: &mut PixmapMut<'_>,
    width: u32,
    search_query: &str,
    _settings_hovered: bool,
    _icon_cache: &IconCache,
    pal: &meridian_ui::style::Palette,
) {
    fill_rect(
        pm,
        Rect { x: 0, y: 0, width: width as i32, height: CP_HEADER_H },
        with_alpha(pal.surface, LAUNCHER_BAND_ALPHA),
    );

    let search = Rect {
        x: LAUNCHER_LAYOUT.outer_pad,
        y: LAUNCHER_LAYOUT.outer_pad,
        width: width as i32 - LAUNCHER_LAYOUT.outer_pad * 2,
        height: LAUNCHER_LAYOUT.search_height,
    };
    fill_round_rect(
        pm,
        search,
        with_alpha(pal.surface_alt, LAUNCHER_SEARCH_FIELD_ALPHA),
        meridian_tokens::Radius::DEFAULT.md,
    );
    draw_round_border(
        pm,
        search,
        with_alpha(pal.accent, LAUNCHER_SEARCH_FOCUS_ALPHA),
        meridian_tokens::Radius::DEFAULT.md,
    );

    let icon_x = search.x + LAUNCHER_LAYOUT.outer_pad;
    let icon_y = search.y + search.height / 2;
    draw_search_symbol(pm, icon_x, icon_y, pal.text_dim);
    let baseline = search.y
        + (search.height + i32::from(meridian_tokens::Typography::DEFAULT.body_size)) / 2;
    paint_text(
        pm,
        if search_query.is_empty() { "Anwendungen suchen" } else { search_query },
        icon_x + 24,
        baseline,
        f32::from(meridian_tokens::Typography::DEFAULT.body_size),
        if search_query.is_empty() { pal.text_dim } else { pal.text },
    );

    let hint_rect = Rect {
        x: search.x + search.width - 56,
        y: search.y + 8,
        width: 48,
        height: search.height - 16,
    };
    draw_round_border(pm, hint_rect, pal.border, meridian_tokens::Radius::DEFAULT.sm);
    paint_text(
        pm,
        "Strg K",
        hint_rect.x + 8,
        hint_rect.y + 18,
        f32::from(meridian_tokens::Typography::DEFAULT.caption_size),
        pal.text_dim,
    );
}

fn draw_bento_strip(
    pm: &mut PixmapMut<'_>,
    _width: u32,
    apps: &[DesktopApp],
    hidden_execs: &HashSet<String>,
    pinned_apps: &[PinnedApp],
    active_category: LauncherCategory,
    hovered_idx: Option<usize>,
    pal: &meridian_ui::style::Palette,
) {
    let body_h = LAUNCHER_LAYOUT.height - CP_HEADER_H - CP_FOOTER_H;
    fill_rect(
        pm,
        Rect {
            x: 0,
            y: CP_BENTO_TOP,
            width: LAUNCHER_LAYOUT.sidebar_width,
            height: body_h,
        },
        with_alpha(pal.surface_alt, LAUNCHER_SEARCH_FIELD_ALPHA),
    );
    fill_rect(
        pm,
        Rect {
            x: LAUNCHER_LAYOUT.sidebar_width - 1,
            y: CP_BENTO_TOP,
            width: 1,
            height: body_h,
        },
        divider_col(pal),
    );
    section_label(pm, "BIBLIOTHEK", CP_BENTO_TOP, pal);

    let tile_x = CP_SECTION_PAD;
    let tile_y = CP_BENTO_TOP + CP_SECTION_LABEL_H;
    for (i, category) in LauncherCategory::ALL.iter().copied().enumerate() {
        let ty = tile_y + i as i32 * (CP_BENTO_TILE_H + CP_BENTO_TILE_GAP);
        let active = category == active_category;
        if hovered_idx == Some(i) || active {
            fill_round_rect(
                pm,
                Rect { x: tile_x, y: ty, width: CP_BENTO_TILE_W, height: CP_BENTO_TILE_H },
                if active {
                    with_alpha(pal.accent, LAUNCHER_SELECTED_ALPHA)
                } else {
                    with_alpha(Interaction::DEFAULT.hover(pal.surface), LAUNCHER_HOVER_ALPHA)
                },
                LAUNCHER_TILE_RADIUS,
            );
        }
        let count = collect_palette_apps(apps, "", hidden_execs, category, pinned_apps).len();
        let label = truncate_to_fit(
            category.label(),
            CP_BENTO_TILE_W - 64,
            f32::from(meridian_tokens::Typography::DEFAULT.body_size),
        );
        paint_text(
            pm,
            &label,
            tile_x + LAUNCHER_LAYOUT.outer_pad,
            ty + 30,
            f32::from(meridian_tokens::Typography::DEFAULT.body_size),
            if active { pal.text } else { pal.text_dim },
        );
        paint_text(
            pm,
            &count.to_string(),
            tile_x + CP_BENTO_TILE_W - 24,
            ty + 30,
            f32::from(meridian_tokens::Typography::DEFAULT.caption_size),
            pal.text_dim,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_app_grid(
    pm: &mut PixmapMut<'_>,
    width: u32,
    height: u32,
    apps: &[DesktopApp],
    pinned_apps: &[PinnedApp],
    category: LauncherCategory,
    search_query: &str,
    scroll_y: i32,
    selected_idx: Option<usize>,
    icon_cache: &IconCache,
    hidden_execs: &HashSet<String>,
    hovered_idx: Option<usize>,
    pal: &meridian_ui::style::Palette,
) {
    let filtered = collect_palette_apps(apps, search_query, hidden_execs, category, pinned_apps);
    let heading_y = CP_APPS_TOP + LAUNCHER_LAYOUT.content_pad;
    paint_text(
        pm,
        if search_query.is_empty() { category.label() } else { "Suchergebnisse" },
        CP_GUTTER,
        heading_y + 25,
        f32::from(meridian_tokens::Typography::DEFAULT.display_size),
        pal.text,
    );
    paint_text(
        pm,
        &format!("{} Anwendungen", filtered.len()),
        CP_GUTTER,
        heading_y + 48,
        f32::from(meridian_tokens::Typography::DEFAULT.caption_size),
        pal.text_dim,
    );

    let content_y = CP_APPS_TOP + LAUNCHER_LAYOUT.content_pad + LAUNCHER_LAYOUT.app_heading_height;
    let grid_h = (height as i32 - content_y - CP_FOOTER_H - LAUNCHER_LAYOUT.content_pad)
        .max(0) as u32;
    let Some(mut grid_pix) = Pixmap::new(width, grid_h) else { return };
    grid_pix.fill(to_tiny_skia_color(Color::rgba(0, 0, 0, 0)));
    {
        let mut gpm = grid_pix.as_mut();
        let n_rows = filtered.len().div_ceil(CP_APP_COLS);
        let content_h = n_rows as i32 * CP_APP_ROW_H - CP_COL_GAP;
        for (global_idx, app) in filtered.iter().enumerate() {
            let row = global_idx / CP_APP_COLS;
            let col = global_idx % CP_APP_COLS;
            let row_y = row as i32 * CP_APP_ROW_H - scroll_y;
            if row_y + LAUNCHER_LAYOUT.app_card_height <= 0 { continue; }
            if row_y >= grid_h as i32 { break; }

            let card_x = CP_GUTTER + col as i32 * (CP_CARD_W + CP_COL_GAP);
            let card = Rect {
                x: card_x,
                y: row_y,
                width: CP_CARD_W,
                height: LAUNCHER_LAYOUT.app_card_height,
            };
            let is_selected = selected_idx == Some(global_idx);
            let is_hovered = hovered_idx == Some(global_idx);
            if is_selected || is_hovered {
                let background = if is_selected {
                    with_alpha(
                        pal.surface.lerp(
                            pal.accent,
                            meridian_tokens::Interaction::SELECTION_SELECTED,
                        ),
                        LAUNCHER_SELECTED_ALPHA,
                    )
                } else {
                    with_alpha(pal.surface, LAUNCHER_HOVER_ALPHA)
                };
                fill_round_rect(&mut gpm, card, background, LAUNCHER_TILE_RADIUS);
            }
            if is_selected {
                draw_round_border(&mut gpm, card, divider_col(pal), LAUNCHER_TILE_RADIUS);
                fill_round_rect(
                    &mut gpm,
                    Rect {
                        x: card.x + LAUNCHER_LAYOUT.outer_pad,
                        y: card.y + card.height - 2,
                        width: card.width - LAUNCHER_LAYOUT.outer_pad * 2,
                        height: 2,
                    },
                    pal.accent,
                    meridian_tokens::Radius::DEFAULT.sm,
                );
            }
            draw_app_row_content(&mut gpm, app, card_x + 8, row_y, icon_cache, pal);
        }
        if content_h > grid_h as i32 {
            draw_scrollbar(&mut gpm, width, grid_h, content_h, scroll_y, pal);
        }
    }
    pm.draw_pixmap(
        0,
        content_y,
        grid_pix.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}

fn draw_search_symbol(pm: &mut PixmapMut<'_>, cx: i32, cy: i32, color: Color) {
    let mut path = PathBuilder::new();
    path.push_circle(cx as f32, cy as f32, 5.0);
    path.move_to(cx as f32 + 4.0, cy as f32 + 4.0);
    path.line_to(cx as f32 + 9.0, cy as f32 + 9.0);
    if let Some(path) = path.finish() {
        let mut paint = SkPaint::default();
        paint.set_color(tiny_skia::Color::from_rgba8(color.r, color.g, color.b, color.a));
        paint.anti_alias = true;
        pm.stroke_path(
            &path,
            &paint,
            &Stroke { width: 1.5, line_cap: LineCap::Round, ..Stroke::default() },
            Transform::identity(),
            None,
        );
    }
}

fn draw_round_border(pm: &mut PixmapMut<'_>, rect: Rect, color: Color, radius: i32) {
    if let Some(path) = rounded_rect_path(rect, radius) {
        paint_border(pm, &path, color, 1.0);
    }
}
