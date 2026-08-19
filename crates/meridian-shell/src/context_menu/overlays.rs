/// Render the context menu as an overlay onto the existing BGRA `canvas`.
/// `draw_background` is disabled by the desktop glass menu, which gets its
/// border/shadow from `popup_card` so the popup chrome has one source.
#[allow(clippy::too_many_arguments)]
fn draw_overlay_with_background(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    state: &ContextMenuState,
    items: &[(&str, ContextMenuAction)],
    icons: &[MenuIcon],
    submenu_arrows: &[bool],
    theme_config: &ThemeConfig,
    draw_background: bool,
) {
    let n = items.len();
    let mw = MENU_WIDTH as u32;
    let mh = menu_height(n) as u32;
    let Some(mut pm) = Pixmap::new(mw, mh) else {
        return;
    };
    let pal = menu_palette_from_config(theme_config);

    // Background
    let bg_rect = Rect {
        x: 0,
        y: 0,
        width: mw as i32,
        height: mh as i32,
    };
    let Some(bg_path) = rounded_rect_path(bg_rect, menu_radius(theme_config)) else {
        return;
    };
    if draw_background {
        paint_menu_background(&mut pm.as_mut(), &bg_path, theme_config, pal);
    }

    // Separator before last item
    let sep_before = n.saturating_sub(1);
    let sep_y = VPAD + sep_before as i32 * ITEM_H;
    let sep_rect = Rect {
        x: 8,
        y: sep_y,
        width: mw as i32 - 16,
        height: 1,
    };
    if let Some(sep_path) = rounded_rect_path(sep_rect, 0) {
        paint_fill(&mut pm.as_mut(), &sep_path, pal.border);
    }

    // Items
    for (i, (label, _)) in items.iter().enumerate() {
        let extra = if i >= sep_before { 1 } else { 0 };
        let item_top = VPAD + i as i32 * ITEM_H + extra;
        let item_rect = Rect {
            x: 2,
            y: item_top,
            width: mw as i32 - 4,
            height: ITEM_H,
        };

        if state.hover_idx == Some(i) {
            if let Some(p) = rounded_rect_path(item_rect, meridian_tokens::Radius::DEFAULT.sm) {
                let hover_fill = if is_glass_menu(theme_config) {
                    theme_accent_idle(theme_config)
                } else {
                    pal.surface_alt.lerp(
                        pal.accent,
                        meridian_tokens::Interaction::DEFAULT.hover_lighten,
                    )
                };
                paint_fill(&mut pm.as_mut(), &p, hover_fill);
            }
            let marker = Rect {
                x: item_rect.x + 1,
                y: item_top + 7,
                width: 3,
                height: ITEM_H - 14,
            };
            if let Some(mp) = rounded_rect_path(marker, 1) {
                let marker_fill = if is_glass_menu(theme_config) {
                    theme_accent_hover(theme_config)
                } else {
                    pal.accent
                };
                paint_fill(&mut pm.as_mut(), &mp, marker_fill);
            }
        }

        let text_x = if let Some(icon) = icons.get(i).copied() {
            let iy = item_top as f32 + (ITEM_H as f32 - ICON_SZ) / 2.0;
            draw_menu_icon(
                &mut pm.as_mut(),
                PADDING_X as f32,
                iy,
                ICON_SZ,
                icon,
                pal.text,
            );
            PADDING_X + ICON_SZ as i32 + ICON_GAP
        } else {
            PADDING_X
        };
        let text_y = item_top + ITEM_H - 10;
        paint_text(&mut pm.as_mut(), label, text_x, text_y, FONT_SIZE, pal.text);

        // Right-pointing triangle for items that open a submenu
        if submenu_arrows.get(i).copied().unwrap_or(false) {
            draw_submenu_arrow_indicator(&mut pm.as_mut(), mw as i32, item_top, pal.text);
        }
    }

    blit_over(
        canvas,
        canvas_w as i32,
        canvas_h as i32,
        &pm,
        state.x,
        state.y,
    );
}

/// Render the desktop context menu (main + optional settings flyout) onto BGRA `canvas`.
pub(crate) fn draw_desktop_overlay(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    state: &DesktopContextMenuState,
    items: &[(&str, DesktopContextMenuAction)],
    theme_config: &ThemeConfig,
) {
    let icons: Vec<MenuIcon> = items.iter().map(|(_, a)| icon_for_desktop(*a)).collect();
    // When the flyout is open, keep the Settings item highlighted in the main menu.
    let effective_hover = if state.submenu_open {
        Some(SETTINGS_ITEM_IDX)
    } else {
        state.hover_idx
    };
    let local_state = ContextMenuState {
        x: state.x,
        y: state.y,
        app_name: "Desktop".into(),
        exec: "".into(),
        is_terminal: false,
        is_pinned: false,
        running_window_id: None,
        hover_idx: effective_hover,
    };
    let app_items: Vec<(&str, ContextMenuAction)> = items
        .iter()
        .map(|(label, _)| (*label, ContextMenuAction::Launch))
        .collect();
    // submenu_arrows: mark the Settings item with a right-pointing triangle
    let mut submenu_arrows = vec![false; app_items.len()];
    if SETTINGS_ITEM_IDX < submenu_arrows.len() {
        submenu_arrows[SETTINGS_ITEM_IDX] = true;
    }
    let draw_panel_background = !is_glass_menu(theme_config);
    draw_overlay_with_background(
        canvas,
        canvas_w,
        canvas_h,
        &local_state,
        &app_items,
        &icons,
        &submenu_arrows,
        theme_config,
        draw_panel_background,
    );

    if state.submenu_open {
        draw_submenu_overlay(
            canvas,
            canvas_w,
            canvas_h,
            state,
            theme_config,
            draw_panel_background,
        );
    }
}

/// Render the settings flyout panel to the right of the main menu.
fn draw_submenu_overlay(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    state: &DesktopContextMenuState,
    theme_config: &ThemeConfig,
    draw_background: bool,
) {
    let items = submenu_items();
    let n = items.len();
    let mw = SUBMENU_WIDTH as u32;
    let mh = submenu_height() as u32;
    let Some(mut pm) = Pixmap::new(mw, mh) else {
        return;
    };
    let pal = menu_palette_from_config(theme_config);

    let bg_rect = Rect {
        x: 0,
        y: 0,
        width: mw as i32,
        height: mh as i32,
    };
    let Some(bg_path) = rounded_rect_path(bg_rect, menu_radius(theme_config)) else {
        return;
    };
    if draw_background {
        paint_menu_background(&mut pm.as_mut(), &bg_path, theme_config, pal);
    }

    for (i, (label, _)) in items.iter().enumerate() {
        let item_top = VPAD + i as i32 * ITEM_H;
        let item_rect = Rect {
            x: 2,
            y: item_top,
            width: mw as i32 - 4,
            height: ITEM_H,
        };

        if state.submenu_hover_idx == Some(i) {
            if let Some(p) = rounded_rect_path(item_rect, meridian_tokens::Radius::DEFAULT.sm) {
                let hover_fill = if is_glass_menu(theme_config) {
                    theme_accent_idle(theme_config)
                } else {
                    pal.surface_alt.lerp(
                        pal.accent,
                        meridian_tokens::Interaction::DEFAULT.hover_lighten,
                    )
                };
                paint_fill(&mut pm.as_mut(), &p, hover_fill);
            }
            let marker = Rect {
                x: item_rect.x + 1,
                y: item_top + 7,
                width: 3,
                height: ITEM_H - 14,
            };
            if let Some(mp) = rounded_rect_path(marker, 1) {
                let marker_fill = if is_glass_menu(theme_config) {
                    theme_accent_hover(theme_config)
                } else {
                    pal.accent
                };
                paint_fill(&mut pm.as_mut(), &mp, marker_fill);
            }
        }

        let text_y = item_top + ITEM_H - 10;
        paint_text(
            &mut pm.as_mut(),
            label,
            PADDING_X,
            text_y,
            FONT_SIZE,
            pal.text,
        );
    }

    blit_over(
        canvas,
        canvas_w as i32,
        canvas_h as i32,
        &pm,
        state.x + MENU_WIDTH + SUBMENU_GAP,
        state.y,
    );
    let _ = n; // used via items.iter().enumerate()
}

/// Alpha-composite a tiny_skia Pixmap (premultiplied RGBA) over a Wayland BGRA canvas.
fn blit_over(canvas: &mut [u8], cw: i32, ch: i32, pm: &Pixmap, dx: i32, dy: i32) {
    let pw = pm.width() as i32;
    let ph = pm.height() as i32;
    let src = pm.pixels();
    for my in 0..ph {
        let cy = dy + my;
        if cy < 0 || cy >= ch {
            continue;
        }
        for mx in 0..pw {
            let cx = dx + mx;
            if cx < 0 || cx >= cw {
                continue;
            }
            let pi = (my * pw + mx) as usize;
            if pi >= src.len() {
                continue;
            }
            let px = src[pi];
            let a = px.alpha();
            if a == 0 {
                continue;
            }
            let ci = ((cy * cw + cx) * 4) as usize;
            if ci + 3 >= canvas.len() {
                continue;
            }
            // premultiplied src over straight-ish dst (BGRA). Preserve
            // alpha via SRC_OVER instead of forcing 255 — hardcoding 255
            // blew out the AA ring on every rounded corner, making them
            // look angular.
            let rp = px.red() as u32;
            let gp = px.green() as u32;
            let bp = px.blue() as u32;
            let inv = 255 - a as u32;
            let db = canvas[ci] as u32;
            let dg = canvas[ci + 1] as u32;
            let dr = canvas[ci + 2] as u32;
            let da = canvas[ci + 3] as u32;
            canvas[ci] = (bp + db * inv / 255).min(255) as u8;
            canvas[ci + 1] = (gp + dg * inv / 255).min(255) as u8;
            canvas[ci + 2] = (rp + dr * inv / 255).min(255) as u8;
            canvas[ci + 3] = (a as u32 + da * inv / 255).min(255) as u8;
        }
    }
}
