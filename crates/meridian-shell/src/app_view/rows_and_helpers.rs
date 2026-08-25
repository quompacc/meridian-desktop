fn draw_app_row_content(
    pm: &mut PixmapMut<'_>,
    app: &DesktopApp,
    icon_x: i32,
    row_y: i32,
    icon_cache: &IconCache,
    pal: &meridian_ui::style::Palette,
) {
    let icon_box = Rect {
        x: icon_x,
        y: row_y + (LAUNCHER_LAYOUT.app_card_height - 44) / 2,
        width: 44,
        height: 44,
    };
    fill_round_rect(
        pm,
        icon_box,
        with_alpha(pal.surface_alt, LAUNCHER_SEARCH_FIELD_ALPHA),
        meridian_tokens::Radius::DEFAULT.md,
    );
    draw_round_border(
        pm,
        icon_box,
        pal.border,
        meridian_tokens::Radius::DEFAULT.md,
    );
    if let Some(name) = app.icon_name.as_deref() {
        if let Some(img) = icon_cache.lookup(name, LAUNCHER_LAYOUT.app_icon_size as u32) {
            if let Some(pix) = icon_image_to_pixmap(img) {
                let ix = icon_box.x + (icon_box.width - pix.width() as i32) / 2;
                let iy = icon_box.y + (icon_box.height - pix.height() as i32) / 2;
                pm.draw_pixmap(
                    ix,
                    iy,
                    pix.as_ref(),
                    &PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
            }
        }
    }
    let tx = icon_box.x + icon_box.width + 12;
    let max_w = CP_CARD_W - (tx - icon_x) - 10;
    let body_size = f32::from(meridian_tokens::Typography::DEFAULT.body_size);
    let caption_size = f32::from(meridian_tokens::Typography::DEFAULT.caption_size);
    let label = truncate_to_fit(&app.name, max_w, body_size);
    paint_text(pm, &label, tx, row_y + 29, body_size, pal.text);
    let detail = truncate_to_fit(&app.program, max_w, caption_size);
    paint_text(pm, &detail, tx, row_y + 49, caption_size, pal.text_dim);
}

fn draw_power_footer(
    pm: &mut PixmapMut<'_>,
    width: u32,
    launcher_h: u32,
    settings_hovered: bool,
    hovered_idx: Option<usize>,
    armed_power: Option<(&str, f32)>,
    pal: &meridian_ui::style::Palette,
) {
    let footer_y = cp_footer_y(launcher_h);
    // divider + background
    fill_rect(
        pm,
        Rect {
            x: 0,
            y: footer_y - 1,
            width: width as i32,
            height: 1,
        },
        divider_col(pal),
    );
    fill_rect(
        pm,
        Rect {
            x: 0,
            y: footer_y,
            width: width as i32,
            height: CP_FOOTER_H,
        },
        with_alpha(pal.surface, LAUNCHER_BAND_ALPHA),
    );

    let btn_y = footer_y + (CP_FOOTER_H - CP_PWR_BTN_SIZE) / 2;

    let avatar = Rect {
        x: LAUNCHER_LAYOUT.outer_pad,
        y: btn_y,
        width: CP_PWR_BTN_SIZE,
        height: CP_PWR_BTN_SIZE,
    };
    fill_round_rect(
        pm,
        avatar,
        with_alpha(pal.surface_alt, LAUNCHER_SEARCH_FIELD_ALPHA),
        meridian_tokens::Radius::DEFAULT.xl,
    );
    draw_round_border(
        pm,
        avatar,
        pal.border,
        meridian_tokens::Radius::DEFAULT.xl,
    );
    draw_user_symbol(
        pm,
        avatar.x + avatar.width / 2,
        avatar.y + avatar.height / 2,
        pal.text_dim,
    );
    paint_text(
        pm,
        "Benutzer",
        avatar.x + avatar.width + 10,
        footer_y + 27,
        f32::from(meridian_tokens::Typography::DEFAULT.body_size),
        pal.text,
    );
    paint_text(
        pm,
        "Lokale Sitzung",
        avatar.x + avatar.width + 10,
        footer_y + 45,
        f32::from(meridian_tokens::Typography::DEFAULT.caption_size),
        pal.text_dim,
    );

    let settings_x = cp_settings_btn_x(width);
    if settings_hovered {
        fill_round_rect(
            pm,
            Rect {
                x: settings_x,
                y: btn_y,
                width: CP_HDR_ICON_W,
                height: CP_HDR_ICON_H,
            },
            with_alpha(
                Interaction::DEFAULT.hover(pal.surface),
                LAUNCHER_HOVER_ALPHA,
            ),
            meridian_tokens::Radius::DEFAULT.md,
        );
    }
    draw_settings_symbol(
        pm,
        settings_x + CP_HDR_ICON_W / 2,
        btn_y + CP_HDR_ICON_H / 2,
        if settings_hovered { pal.text } else { pal.text_dim },
    );

    #[allow(clippy::needless_range_loop)]
    for i in 0..5usize {
        let bx = CP_PWR_START_X + i as i32 * CP_PWR_BTN_STRIDE;
        let is_hov = hovered_idx == Some(i);
        let is_armed = armed_power
            .map(|(id, _)| id == POWER_IDS[i])
            .unwrap_or(false);

        if is_hov || is_armed {
            let bg = if is_armed {
                with_alpha(pal.error, LAUNCHER_POWER_ARMED_ALPHA)
            } else {
                with_alpha(
                    Interaction::DEFAULT.hover(pal.surface),
                    LAUNCHER_HOVER_ALPHA,
                )
            };
            fill_round_rect(
                pm,
                Rect {
                    x: bx,
                    y: btn_y,
                    width: CP_PWR_BTN_SIZE,
                    height: CP_PWR_BTN_SIZE,
                },
                bg,
                meridian_tokens::Radius::DEFAULT.md,
            );
        }

        let col = if is_armed {
            pal.error
        } else if is_hov {
            pal.text
        } else {
            pal.text_dim
        };

        draw_power_symbol(
            pm,
            i,
            bx + CP_PWR_BTN_SIZE / 2,
            btn_y + CP_PWR_BTN_SIZE / 2,
            col,
        );

        // arm progress bar
        if is_armed {
            if let Some((_, p)) = armed_power.filter(|(id, _)| *id == POWER_IDS[i]) {
                let bar_w = (CP_PWR_BTN_SIZE as f32 * p) as i32;
                fill_rect(
                    pm,
                    Rect {
                        x: bx,
                        y: btn_y + CP_PWR_BTN_SIZE - 1,
                        width: bar_w,
                        height: 1,
                    },
                    pal.error,
                );
            }
        }
    }
}

fn draw_user_symbol(pm: &mut PixmapMut<'_>, cx: i32, cy: i32, col: Color) {
    let mut paint = SkPaint::default();
    paint.set_color(tiny_skia::Color::from_rgba8(col.r, col.g, col.b, col.a));
    paint.anti_alias = true;
    let stroke = Stroke {
        width: 1.5,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };
    let mut path = PathBuilder::new();
    path.push_circle(cx as f32, cy as f32 - 5.0, 4.0);
    path.move_to(cx as f32 - 7.0, cy as f32 + 8.0);
    path.cubic_to(
        cx as f32 - 6.0,
        cy as f32 + 2.0,
        cx as f32 + 6.0,
        cy as f32 + 2.0,
        cx as f32 + 7.0,
        cy as f32 + 8.0,
    );
    if let Some(path) = path.finish() {
        pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

fn draw_settings_symbol(pm: &mut PixmapMut<'_>, cx: i32, cy: i32, col: meridian_ui::style::Color) {
    let ts_col = tiny_skia::Color::from_rgba8(col.r, col.g, col.b, col.a);
    let mut paint = SkPaint::default();
    paint.set_color(ts_col);
    paint.anti_alias = true;
    let stroke = Stroke {
        width: 1.5,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    };
    let fx = cx as f32;
    let fy = cy as f32;
    // Three slider lines
    for dy in [-6.0f32, 0.0, 6.0] {
        let mut pb = PathBuilder::new();
        pb.move_to(fx - 9.0, fy + dy);
        pb.line_to(fx + 9.0, fy + dy);
        if let Some(p) = pb.finish() {
            pm.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
        }
    }
    // Three knobs (filled circles) at different x positions
    for (dy, kx) in [(-6.0f32, -3.5f32), (0.0, 2.5), (6.0, -1.0)] {
        let mut pb = PathBuilder::new();
        pb.push_circle(fx + kx, fy + dy, 3.0);
        if let Some(p) = pb.finish() {
            pm.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }
}

fn arc_seg(
    pb: &mut PathBuilder,
    cx: f32,
    cy: f32,
    r: f32,
    start_deg: f32,
    end_deg: f32,
    first_move: bool,
) {
    let n = ((end_deg - start_deg).abs() / 5.0).ceil() as usize + 2;
    for i in 0..=n {
        let t = (start_deg + (end_deg - start_deg) * i as f32 / n as f32).to_radians();
        let x = cx + r * t.cos();
        let y = cy + r * t.sin();
        if i == 0 && first_move {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
}

fn draw_power_symbol(
    pm: &mut PixmapMut<'_>,
    idx: usize,
    cx: i32,
    cy: i32,
    col: meridian_ui::style::Color,
) {
    let fx = cx as f32;
    let fy = cy as f32;
    let ts_col = tiny_skia::Color::from_rgba8(col.r, col.g, col.b, col.a);
    let mut paint = SkPaint::default();
    paint.set_color(ts_col);
    paint.anti_alias = true;
    let stroke = Stroke {
        width: 1.5,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    };
    let do_stroke = |pm: &mut PixmapMut<'_>, pb: PathBuilder| {
        if let Some(p) = pb.finish() {
            pm.stroke_path(&p, &paint, &stroke, Transform::identity(), None);
        }
    };
    match idx {
        0 => {
            // Lock — shackle arc + body rect + keyhole dot
            let mut pb = PathBuilder::new();
            pb.move_to(fx - 4.0, fy - 2.0);
            pb.line_to(fx - 4.0, fy - 6.0);
            arc_seg(&mut pb, fx, fy - 6.0, 4.0, 180.0, 360.0, false);
            pb.line_to(fx + 4.0, fy - 2.0);
            do_stroke(pm, pb);
            let mut pb = PathBuilder::new();
            pb.move_to(fx - 6.0, fy - 2.0);
            pb.line_to(fx + 6.0, fy - 2.0);
            pb.line_to(fx + 6.0, fy + 6.0);
            pb.line_to(fx - 6.0, fy + 6.0);
            pb.close();
            do_stroke(pm, pb);
            let mut pb = PathBuilder::new();
            pb.push_circle(fx, fy + 2.0, 2.0);
            do_stroke(pm, pb);
        }
        1 => {
            // Logout — door bar + right arrow
            let mut pb = PathBuilder::new();
            pb.move_to(fx - 7.0, fy - 7.0);
            pb.line_to(fx - 7.0, fy + 7.0);
            pb.move_to(fx - 2.0, fy);
            pb.line_to(fx + 7.0, fy);
            pb.move_to(fx + 3.0, fy - 4.0);
            pb.line_to(fx + 7.0, fy);
            pb.line_to(fx + 3.0, fy + 4.0);
            do_stroke(pm, pb);
        }
        2 => {
            // Sleep — crescent moon via EvenOdd (big circle minus offset smaller circle)
            let mut pb = PathBuilder::new();
            pb.push_circle(fx - 1.0, fy, 7.5);
            pb.push_circle(fx + 2.5, fy - 1.0, 6.0);
            if let Some(p) = pb.finish() {
                let mut fp = SkPaint::default();
                fp.set_color(ts_col);
                fp.anti_alias = true;
                pm.fill_path(&p, &fp, FillRule::EvenOdd, Transform::identity(), None);
            }
        }
        3 => {
            // Restart — 270° arc with arrowhead
            let r = 7.0f32;
            let mut pb = PathBuilder::new();
            arc_seg(&mut pb, fx, fy, r, -30.0, -30.0 + 270.0, true);
            do_stroke(pm, pb);
            // Arrowhead at end of arc (240°): tip at (fx-3.5, fy-6.06)
            let end_rad = 240.0f32.to_radians();
            let ex = fx + r * end_rad.cos();
            let ey = fy + r * end_rad.sin();
            let al = 4.5f32;
            // Arms rotated ±150° from CW tangent at 240° = (-0.866, 0.5)
            let mut pb = PathBuilder::new();
            pb.move_to(ex + 0.5 * al, ey - 0.866 * al);
            pb.line_to(ex, ey);
            pb.line_to(ex + 1.0 * al, ey);
            do_stroke(pm, pb);
        }
        _ => {
            // Power off — 300° circle arc + vertical line through top gap
            let r = 7.0f32;
            let mut pb = PathBuilder::new();
            arc_seg(&mut pb, fx, fy, r, -60.0, -60.0 + 300.0, true);
            do_stroke(pm, pb);
            let mut pb = PathBuilder::new();
            pb.move_to(fx, fy - 3.0);
            pb.line_to(fx, fy - 9.0);
            do_stroke(pm, pb);
        }
    }
}

fn draw_scrollbar(
    pm: &mut PixmapMut<'_>,
    width: u32,
    view_h: u32,
    content_h: i32,
    scroll_y: i32,
    pal: &meridian_ui::style::Palette,
) {
    let track_x = width as i32 - 6;
    let track_h = view_h as i32 - 8;
    if track_h <= 0 {
        return;
    }
    let thumb_h = ((track_h * view_h as i32) / content_h).max(20).min(track_h);
    let max_scroll = (content_h - view_h as i32).max(1);
    let thumb_y = 4 + scroll_y * (track_h - thumb_h) / max_scroll;
    let track_col = with_alpha(pal.text, LAUNCHER_SCROLLBAR_TRACK_ALPHA);
    let thumb_col = with_alpha(pal.accent, LAUNCHER_SCROLLBAR_THUMB_ALPHA);
    fill_rect(
        pm,
        Rect {
            x: track_x,
            y: 4,
            width: 4,
            height: track_h,
        },
        track_col,
    );
    fill_rect(
        pm,
        Rect {
            x: track_x,
            y: thumb_y,
            width: 4,
            height: thumb_h,
        },
        thumb_col,
    );
}

fn section_label(pm: &mut PixmapMut<'_>, label: &str, y: i32, pal: &meridian_ui::style::Palette) {
    paint_text(
        pm,
        label,
        CP_SECTION_PAD,
        y + CP_SECTION_LABEL_H - 6,
        10.0,
        pal.text,
    );
}

fn divider(pm: &mut PixmapMut<'_>, width: u32, y: i32, pal: &meridian_ui::style::Palette) {
    fill_rect(
        pm,
        Rect {
            x: 0,
            y,
            width: width as i32,
            height: CP_DIVIDER_H,
        },
        divider_col(pal),
    );
}

fn divider_col(pal: &meridian_ui::style::Palette) -> Color {
    with_alpha(pal.accent, LAUNCHER_DIVIDER_ALPHA)
}

fn fill_rect(pm: &mut PixmapMut<'_>, rect: Rect, color: Color) {
    if let Some(path) = rounded_rect_path(rect, 0) {
        paint_fill(pm, &path, color);
    }
}

fn fill_round_rect(pm: &mut PixmapMut<'_>, rect: Rect, color: Color, radius: i32) {
    if let Some(path) = rounded_rect_path(rect, radius) {
        paint_fill(pm, &path, color);
    }
}

fn with_alpha(color: Color, alpha: u8) -> Color {
    Color::rgba(color.r, color.g, color.b, alpha)
}

fn blit_rgba_to_argb(src: &[u8], dst: &mut [u8]) {
    if src.len() != dst.len() || !src.len().is_multiple_of(4) {
        return;
    }
    for (rgba, argb) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        argb[0] = rgba[2];
        argb[1] = rgba[1];
        argb[2] = rgba[0];
        argb[3] = rgba[3];
    }
}

fn to_tiny_skia_color(color: Color) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(color.r, color.g, color.b, color.a)
}
