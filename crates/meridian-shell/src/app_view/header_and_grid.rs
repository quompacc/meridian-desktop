fn draw_header(
    pm: &mut PixmapMut<'_>,
    width: u32,
    search_query: &str,
    settings_hovered: bool,
    _icon_cache: &IconCache,
    pal: &meridian_ui::style::Palette,
) {
    fill_rect(
        pm,
        Rect {
            x: 0,
            y: 0,
            width: width as i32,
            height: CP_HEADER_H,
        },
        with_alpha(pal.surface, LAUNCHER_BAND_ALPHA),
    );

    let search_w = (width as i32 - 78).max(120);
    fill_round_rect(
        pm,
        Rect {
            x: 14,
            y: 10,
            width: search_w,
            height: 32,
        },
        with_alpha(
            Interaction::DEFAULT.hover(pal.surface),
            LAUNCHER_SEARCH_FIELD_ALPHA,
        ),
        10,
    );

    let text_x = 20i32;
    let text_baseline = CP_HEADER_H - (CP_HEADER_H - 14) / 2 - 2;
    if search_query.is_empty() {
        paint_text(pm, "Apps suchen...", text_x, text_baseline, 13.0, pal.text);
    } else {
        paint_text(pm, search_query, text_x, text_baseline, 13.0, pal.text);
    }

    let sx = cp_settings_btn_x(width);
    let sy = cp_hdr_icon_y();
    let icon_col = if settings_hovered {
        pal.text
    } else {
        pal.text_dim
    };
    draw_settings_symbol(pm, sx + CP_HDR_ICON_W / 2, sy + CP_HDR_ICON_H / 2, icon_col);
}

fn draw_bento_strip(
    pm: &mut PixmapMut<'_>,
    _width: u32,
    pinned_apps: &[PinnedApp],
    icon_cache: &IconCache,
    hovered_idx: Option<usize>,
    pal: &meridian_ui::style::Palette,
) {
    section_label(pm, "ANGEHEFTET", CP_BENTO_TOP, pal);

    let n = pinned_apps.len().min(CP_MAX_BENTO);
    if n == 0 {
        return;
    }
    let strip_x = cp_bento_tile_x(n);
    let tile_y = CP_BENTO_TOP + CP_SECTION_LABEL_H + CP_SECTION_PAD;

    for (i, app) in pinned_apps.iter().take(CP_MAX_BENTO).enumerate() {
        let tx = strip_x + i as i32 * (CP_BENTO_TILE_W + CP_BENTO_TILE_GAP);

        let bg = if hovered_idx == Some(i) {
            with_alpha(
                Interaction::DEFAULT.hover(pal.surface),
                LAUNCHER_HOVER_ALPHA,
            )
        } else {
            with_alpha(pal.surface, LAUNCHER_CELL_ALPHA)
        };
        fill_round_rect(
            pm,
            Rect {
                x: tx,
                y: tile_y,
                width: CP_BENTO_TILE_W,
                height: CP_BENTO_TILE_H,
            },
            bg,
            LAUNCHER_TILE_RADIUS,
        );
        // bottom accent line
        fill_rect(
            pm,
            Rect {
                x: tx,
                y: tile_y + CP_BENTO_TILE_H - 1,
                width: CP_BENTO_TILE_W,
                height: 1,
            },
            with_alpha(pal.accent, LAUNCHER_BENTO_ACCENT_ALPHA),
        );

        // icon – try large then small sizes
        if let Some(name) = app.icon_name.as_deref() {
            for &sz in &[48u32, 32, 24] {
                if let Some(img) = icon_cache.lookup(name, sz) {
                    if let Some(pix) = icon_image_to_pixmap(img) {
                        let pw = pix.width() as i32;
                        let ph = pix.height() as i32;
                        let ix = tx + (CP_BENTO_TILE_W - pw) / 2;
                        let icon_center = tile_y + (CP_BENTO_TILE_H * 55) / 100;
                        let iy = icon_center - ph / 2;
                        pm.draw_pixmap(
                            ix,
                            iy,
                            pix.as_ref(),
                            &PixmapPaint::default(),
                            Transform::identity(),
                            None,
                        );
                        break;
                    }
                }
            }
        }

        // label
        let max_w = CP_BENTO_TILE_W - 6;
        let label = truncate_to_fit(&app.label, max_w, 11.0);
        let lx = tx + 3;
        let ly = tile_y + CP_BENTO_TILE_H - 5;
        paint_text(pm, &label, lx, ly, 11.0, pal.text);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_app_grid(
    pm: &mut PixmapMut<'_>,
    width: u32,
    height: u32,
    apps: &[DesktopApp],
    search_query: &str,
    scroll_y: i32,
    selected_idx: Option<usize>,
    icon_cache: &IconCache,
    hidden_execs: &HashSet<String>,
    hovered_idx: Option<usize>,
    pal: &meridian_ui::style::Palette,
) {
    let content_y = CP_APPS_TOP + CP_SECTION_LABEL_H + CP_SECTION_PAD;
    let grid_h = (height as i32 - content_y - CP_FOOTER_H - 1).max(0) as u32;
    let Some(mut grid_pix) = Pixmap::new(width, grid_h) else {
        return;
    };
    grid_pix.fill(to_tiny_skia_color(Color::rgba(0, 0, 0, 0)));
    {
        let mut gpm = grid_pix.as_mut();
        let filtered = collect_palette_apps(apps, search_query, icon_cache, hidden_execs);
        let n_rows = filtered.len().div_ceil(CP_APP_COLS);
        let content_h = n_rows as i32 * CP_APP_ROW_H;

        for (global_idx, app) in filtered.iter().enumerate() {
            let row = global_idx / CP_APP_COLS;
            let col = global_idx % CP_APP_COLS;
            let row_y = row as i32 * CP_APP_ROW_H - scroll_y;
            if row_y + CP_APP_ROW_H <= 0 {
                continue;
            }
            if row_y >= grid_h as i32 {
                break;
            }

            let card_x = CP_GUTTER + col as i32 * (CP_CARD_W + CP_COL_GAP);
            let is_sel = selected_idx == Some(global_idx);
            let is_hov = hovered_idx == Some(global_idx);

            if is_sel || is_hov {
                let bg = if is_sel {
                    with_alpha(
                        pal.surface
                            .lerp(pal.accent, meridian_tokens::Interaction::SELECTION_SELECTED),
                        LAUNCHER_SELECTED_ALPHA,
                    )
                } else {
                    with_alpha(pal.surface, LAUNCHER_HOVER_ALPHA)
                };
                fill_round_rect(
                    &mut gpm,
                    Rect {
                        x: card_x,
                        y: row_y + 3,
                        width: CP_CARD_W,
                        height: CP_APP_ROW_H - 6,
                    },
                    bg,
                    LAUNCHER_TILE_RADIUS,
                );
                if is_sel {
                    fill_rect(
                        &mut gpm,
                        Rect {
                            x: card_x,
                            y: row_y + 7,
                            width: 2,
                            height: CP_APP_ROW_H - 14,
                        },
                        pal.accent,
                    );
                }
            }

            draw_app_row_content(&mut gpm, app, card_x + 10, row_y, icon_cache, pal);
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

#[allow(clippy::too_many_arguments)]
fn draw_search_results(
    pm: &mut PixmapMut<'_>,
    width: u32,
    height: u32,
    apps: &[DesktopApp],
    search_query: &str,
    scroll_y: i32,
    selected_idx: Option<usize>,
    icon_cache: &IconCache,
    hidden_execs: &HashSet<String>,
    hovered_idx: Option<usize>,
    pal: &meridian_ui::style::Palette,
) {
    let content_y = CP_HEADER_H + CP_DIVIDER_H;
    let list_h = (height as i32 - content_y - CP_FOOTER_H - 1).max(0) as u32;
    let Some(mut list_pix) = Pixmap::new(width, list_h) else {
        return;
    };
    list_pix.fill(to_tiny_skia_color(Color::rgba(0, 0, 0, 0)));
    {
        let mut lpm = list_pix.as_mut();
        let filtered = collect_palette_apps(apps, search_query, icon_cache, hidden_execs);
        let content_h = filtered.len() as i32 * CP_APP_ROW_H;

        for (idx, app) in filtered.iter().enumerate() {
            let row_y = idx as i32 * CP_APP_ROW_H - scroll_y;
            if row_y + CP_APP_ROW_H <= 0 {
                continue;
            }
            if row_y >= list_h as i32 {
                break;
            }

            let is_sel = selected_idx == Some(idx);
            let is_hov = hovered_idx == Some(idx);

            if is_sel || is_hov {
                let bg = if is_sel {
                    with_alpha(
                        pal.surface
                            .lerp(pal.accent, meridian_tokens::Interaction::SELECTION_SELECTED),
                        LAUNCHER_SELECTED_ALPHA,
                    )
                } else {
                    with_alpha(pal.surface, LAUNCHER_HOVER_ALPHA)
                };
                fill_round_rect(
                    &mut lpm,
                    Rect {
                        x: CP_GUTTER,
                        y: row_y + 3,
                        width: width as i32 - 2 * CP_GUTTER,
                        height: CP_APP_ROW_H - 6,
                    },
                    bg,
                    LAUNCHER_TILE_RADIUS,
                );
                if is_sel {
                    fill_rect(
                        &mut lpm,
                        Rect {
                            x: CP_GUTTER,
                            y: row_y + 7,
                            width: 3,
                            height: CP_APP_ROW_H - 14,
                        },
                        pal.accent,
                    );
                }
            }

            draw_app_row_content(&mut lpm, app, 16, row_y, icon_cache, pal);
        }

        if content_h > list_h as i32 {
            draw_scrollbar(&mut lpm, width, list_h, content_h, scroll_y, pal);
        }
    }
    pm.draw_pixmap(
        0,
        content_y,
        list_pix.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}
