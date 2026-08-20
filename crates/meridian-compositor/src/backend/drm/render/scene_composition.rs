macro_rules! compose_output_scene {
    ($state:ident, $renderer:ident, $out:ident, $space:ident, $theme:ident, $scale:ident, $cursor_icon:ident, $cursor_image:ident, $cursor_buffer:ident, $named_cursor_cache:ident, $decoration_element_count:ident, $space_element_count:ident, $cursor_count:ident) => {
        if !$state.lock_manager.is_locked_or_pending() {
            for window in $out.scratch_windows.iter().rev() {
                let loc = match $space.element_location(window) {
                    Some(l) => l,
                    None => continue,
                };
                let geometry = window.geometry();
                let window_surface = window.wl_surface().map(|surface| surface.into_owned());
                let preview_rect = window_surface
                    .as_ref()
                    .and_then(crate::grabs::resize_grab::preview_rect);
                let visual_rect = preview_rect
                    .unwrap_or_else(|| smithay::utils::Rectangle::new(loc, geometry.size));
                let render_loc =
                    smithay::utils::Point::from((loc.x - geometry.loc.x, loc.y - geometry.loc.y));

                render_window_popup_elements(
                    $renderer,
                    window,
                    render_loc,
                    $scale,
                    &mut $out.scratch_normal,
                );

                let mut content_clip = None;
                let mut window_drop_shadow = None;
                if let Some(wl_surf) = window_surface {
                    let metrics = $state.decoration_manager.ssd_render_metrics(
                        &wl_surf,
                        visual_rect.loc,
                        visual_rect.size,
                        &$theme.decorations,
                    );
                    let window_deco_elements = $state.decoration_manager.render_elements(
                        $renderer,
                        &wl_surf,
                        metrics.frame_origin,
                        metrics.client_size,
                        &$theme.decorations,
                        &$theme.colors,
                        $scale,
                    );
                    $decoration_element_count += window_deco_elements.len();
                    for element in window_deco_elements {
                        match element {
                            crate::decoration::DecorationRenderElement::Solid(solid) => {
                                $out.scratch_normal
                                    .push(MeridianRenderElements::Decoration(solid));
                            }
                            crate::decoration::DecorationRenderElement::Icon(icon) => {
                                $out.scratch_normal
                                    .push(MeridianRenderElements::DecorationIcon(icon.into()));
                            }
                            crate::decoration::DecorationRenderElement::PixelShader(s) => {
                                $out.scratch_normal.push(MeridianRenderElements::Shadow(s));
                            }
                            crate::decoration::DecorationRenderElement::Glass(info) => {
                                $out.scratch_normal.push(MeridianRenderElements::Glass(
                                    GlassElement::pending(info, $scale),
                                ));
                            }
                            crate::decoration::DecorationRenderElement::DropShadow(s) => {
                                // Held aside and pushed after the content
                                // below, so it sits *behind* the window.
                                window_drop_shadow = Some(s);
                            }
                        }
                    }

                    // Round the client content's bottom corners to match the
                    // rounded border/titlebar (top corners sit under the
                    // titlebar, so only the bottom two need clipping).
                    if preview_rect.is_none() {
                        if let Some(r) = $state
                        .decoration_manager
                        .content_corner_radius(&wl_surf, &$theme.decorations)
                    {
                        if let Some(prog) = crate::backend::clipped_surface::clip_shader($renderer) {
                            let r8 = r.min(255) as u8;
                            content_clip =
                                Some((prog, metrics.client_rect.to_f64(), [r8, 0, r8, 0]));
                        }
                    }
                    }
                }

                let space_start = $out.scratch_normal.len();
                render_window_toplevel_elements(
                    $renderer,
                    window,
                    render_loc,
                    $scale,
                    preview_rect,
                    content_clip,
                    &mut $out.scratch_normal,
                );
                let appended_space = $out.scratch_normal.len().saturating_sub(space_start);
                $space_element_count += appended_space;
                if let Some(s) = window_drop_shadow {
                    $out.scratch_normal.push(MeridianRenderElements::Shadow(s));
                }
            }

            if let Some(pointer) = $state.seat.get_pointer() {
                let pointer_location = pointer.current_location();
                if let Some(output_geo) = $space.output_geometry(&$out.output) {
                    if output_geo.to_f64().contains(pointer_location) {
                        let cursor_pos = (pointer_location - output_geo.loc.to_f64())
                            .to_physical($scale)
                            .to_i32_round::<i32>();
                        match &$state.cursor_status {
                            CursorImageStatus::Hidden => {}
                            CursorImageStatus::Named(icon_name) => {
                                // Preserve compositor-managed resize cursors for SSD/X11 edge hit-tests.
                                if !matches!($cursor_icon, super::DrmCursorIcon::Default) {
                                    let mut cursor_loc = cursor_pos;
                                    cursor_loc.x -= $cursor_image.xhot as i32;
                                    cursor_loc.y -= $cursor_image.yhot as i32;
                                    if let Ok(element) =
                                        MemoryRenderBufferRenderElement::from_buffer(
                                            $renderer,
                                            cursor_loc.to_f64(),
                                            $cursor_buffer,
                                            None,
                                            None,
                                            None,
                                            Kind::Cursor,
                                        )
                                    {
                                        $out.scratch_cursor
                                            .push(MeridianRenderElements::Cursor(element));
                                    }
                                } else {
                                    let cursor_cfg = &$state.theme_manager.current().config.cursor;
                                    let (named_buffer, hotspot) = $named_cursor_cache
                                        .entry(icon_name.name().to_string())
                                        .or_insert_with(|| {
                                            let cursor =
                                                crate::cursor::CursorImage::load_theme_cursor_icon(
                                                    &cursor_cfg.$theme,
                                                    cursor_cfg.size,
                                                    *icon_name,
                                                );
                                            (
                                                cursor.to_memory_buffer(),
                                                smithay::utils::Point::from((
                                                    cursor.xhot as i32,
                                                    cursor.yhot as i32,
                                                )),
                                            )
                                        });
                                    let mut cursor_loc = cursor_pos;
                                    cursor_loc.x -= hotspot.x;
                                    cursor_loc.y -= hotspot.y;
                                    if let Ok(element) =
                                        MemoryRenderBufferRenderElement::from_buffer(
                                            $renderer,
                                            cursor_loc.to_f64(),
                                            named_buffer,
                                            None,
                                            None,
                                            None,
                                            Kind::Cursor,
                                        )
                                    {
                                        $out.scratch_cursor
                                            .push(MeridianRenderElements::Cursor(element));
                                    }
                                }
                            }
                            CursorImageStatus::Surface(surface) => {
                                let hotspot = with_states(surface, |states| {
                                    states
                                        .data_map
                                        .get::<CursorImageSurfaceData>()
                                        .map(|attrs| attrs.lock().unwrap().hotspot)
                                        .unwrap_or_default()
                                });
                                let hotspot =
                                    hotspot.to_f64().to_physical($scale).to_i32_round::<i32>();
                                let cursor_loc = smithay::utils::Point::from((
                                    cursor_pos.x - hotspot.x,
                                    cursor_pos.y - hotspot.y,
                                ));
                                $out.scratch_cursor
                                    .extend(render_elements_from_surface_tree::<
                                        GlesRenderer,
                                        MeridianRenderElements,
                                    >(
                                        $renderer,
                                        surface,
                                        cursor_loc,
                                        $scale,
                                        1.0,
                                        Kind::Cursor,
                                    ));
                            }
                        }
                    }
                }
            }

            collect_layer_data(
                &$out.output,
                &mut $out.scratch_lower_layer_data,
                &mut $out.scratch_upper_layer_data,
            );
            render_layer_elements(
                $renderer,
                &$out.scratch_lower_layer_data,
                $scale,
                &mut $out.scratch_lower_layer_elements,
            );
            render_layer_elements(
                $renderer,
                &$out.scratch_upper_layer_data,
                $scale,
                &mut $out.scratch_upper_layer_elements,
            );

            let layer_glass_enabled = $theme.decorations.glass && $theme.decorations.glass_blur;
            if layer_glass_enabled {
                let theme_config = &$state.theme_manager.current().config;
                // Liquid-glass panel: a blurred-scene backdrop behind the
                // translucent panel island. Pushed last in the upper-layer block so
                // it sits behind the panel surface but in front of windows. The
                // island insets/radius mirror the shell panel constants
                // (PANEL_SIDE_MARGIN=12, PANEL_TOP_SHADOW=16, PANEL_HEIGHT=42,
                // ISLAND_RADIUS=12).
                if let Some((_, pg)) = $out
                    .scratch_upper_layer_data
                    .iter()
                    .find(|(ls, _)| ls.namespace() == "meridian-panel")
                {
                    let island = smithay::utils::Rectangle::<i32, smithay::utils::Logical>::new(
                        (pg.loc.x + 12, pg.loc.y + 16).into(),
                        ((pg.size.w - 24).max(1), 42).into(),
                    );
                    let info = themed_layer_glass_info(theme_config, island, ThemeSurface::Panel);
                    $out.scratch_upper_layer_elements
                        .push(MeridianRenderElements::Glass(GlassElement::pending(
                            info, $scale,
                        )));
                }

                // Liquid-glass launcher: the shell paints a translucent rounded
                // command palette on a full-screen layer surface. Mirror the
                // shell's visual card geometry here so the card samples a live
                // blurred backdrop instead of darkening the wallpaper behind it.
                if let Some((_, lg)) = $out
                    .scratch_upper_layer_data
                    .iter()
                    .find(|(ls, _)| ls.namespace() == "meridian-launcher")
                {
                    let launcher_w = 880;
                    let launcher_h = 620;
                    let popup_bottom_margin = 2;
                    let visual_x = if lg.size.w > launcher_w {
                        lg.loc.x + 12
                    } else {
                        lg.loc.x
                    };
                    let visual_y = if lg.size.h > launcher_h {
                        lg.loc.y + lg.size.h - launcher_h - popup_bottom_margin
                    } else {
                        lg.loc.y
                    };
                    let card = smithay::utils::Rectangle::<i32, smithay::utils::Logical>::new(
                        (visual_x.max(lg.loc.x), visual_y.max(lg.loc.y)).into(),
                        (
                            launcher_w.min(lg.size.w).max(1),
                            launcher_h.min(lg.size.h).max(1),
                        )
                            .into(),
                    );
                    let info = themed_layer_glass_info(theme_config, card, ThemeSurface::Launcher);
                    $out.scratch_upper_layer_elements
                        .push(MeridianRenderElements::Glass(GlassElement::pending(
                            info, $scale,
                        )));
                }

                // Liquid-glass desktop context menu. The shell paints only the
                // border/text/hover chrome for glass themes; mirror its card
                // geometry here so the main menu and settings flyout each sample
                // their own blurred backdrop instead of sharing one large box.
                if let Some((_, menu_geo)) = $out
                    .scratch_upper_layer_data
                    .iter()
                    .find(|(ls, _)| ls.namespace() == "meridian-desktop-menu")
                {
                    const PAD: i32 = 16;
                    const MENU_W: i32 = 236;
                    const MENU_H: i32 = 193;
                    const SUBMENU_GAP: i32 = 6;
                    const SUBMENU_W: i32 = 188;
                    const SUBMENU_H: i32 = 228;
                    let card_w = (menu_geo.size.w - 2 * PAD).max(1);
                    let card_h = (menu_geo.size.h - 2 * PAD).max(1);
                    let mut push_menu_glass = |x: i32, y: i32, w: i32, h: i32| {
                        let card = smithay::utils::Rectangle::<i32, smithay::utils::Logical>::new(
                            (x, y).into(),
                            (w.max(1), h.max(1)).into(),
                        );
                        let info = themed_layer_glass_info(theme_config, card, ThemeSurface::Popup);
                        $out.scratch_upper_layer_elements
                            .push(MeridianRenderElements::Glass(GlassElement::pending(
                                info, $scale,
                            )));
                    };

                    let main_x = menu_geo.loc.x + PAD;
                    let main_y = menu_geo.loc.y + PAD;
                    if card_w > MENU_W + SUBMENU_GAP {
                        push_menu_glass(main_x, main_y, MENU_W, MENU_H.min(card_h));
                        push_menu_glass(
                            main_x + MENU_W + SUBMENU_GAP,
                            main_y,
                            SUBMENU_W.min(card_w - MENU_W - SUBMENU_GAP),
                            SUBMENU_H.min(card_h),
                        );
                    } else {
                        push_menu_glass(main_x, main_y, card_w, card_h);
                    }
                }

                for (_, popup_geo) in $out.scratch_upper_layer_data.iter().filter(|(ls, _)| {
                    matches!(
                        ls.namespace(),
                        "meridian-calendar-popup"
                            | "meridian-workspace-popup"
                            | "meridian-network-popup"
                            | "meridian-notification"
                            | "meridian-thumbnail-popup"
                    )
                }) {
                    let pad = 16;
                    let card_w = (popup_geo.size.w - 2 * pad).max(1);
                    let card_h = (popup_geo.size.h - 2 * pad).max(1);
                    let card = smithay::utils::Rectangle::<i32, smithay::utils::Logical>::new(
                        (popup_geo.loc.x + pad, popup_geo.loc.y + pad).into(),
                        (card_w, card_h).into(),
                    );
                    let info = themed_layer_glass_info(theme_config, card, ThemeSurface::Popup);
                    $out.scratch_upper_layer_elements
                        .push(MeridianRenderElements::Glass(GlassElement::pending(
                            info, $scale,
                        )));
                }
            }

            let wallpaper_elem = $out
                .wallpaper
                .as_ref()
                .map(WallpaperGpuCache::render_element);

            #[cfg(debug_assertions)]
            {
                $cursor_count = $out.scratch_cursor.len();
            }
            {
                let (
                    scratch_final,
                    scratch_cursor,
                    scratch_normal,
                    scratch_lower_layer_elements,
                    scratch_upper_layer_elements,
                ) = (
                    &mut $out.scratch_final,
                    &mut $out.scratch_cursor,
                    &mut $out.scratch_normal,
                    &mut $out.scratch_lower_layer_elements,
                    &mut $out.scratch_upper_layer_elements,
                );
                scratch_final.append(scratch_cursor);
                scratch_final.append(scratch_upper_layer_elements);
                scratch_final.append(scratch_normal);
                scratch_final.append(scratch_lower_layer_elements);
                scratch_final.extend(
                    wallpaper_elem
                        .into_iter()
                        .map(MeridianRenderElements::Wallpaper),
                );
            }
        } else {
            $out.scratch_cursor.clear();
            $out.scratch_lower_layer_data.clear();
            $out.scratch_upper_layer_data.clear();
            $out.scratch_lower_layer_elements.clear();
            $out.scratch_upper_layer_elements.clear();
            let output_name = $out.output.name();
            if let Some(lock_surface) = $state.lock_manager.surface_for_output(&output_name) {
                $out.scratch_normal
                    .extend(render_elements_from_surface_tree::<
                        GlesRenderer,
                        MeridianRenderElements,
                    >(
                        $renderer,
                        lock_surface.wl_surface(),
                        (0, 0),
                        $scale,
                        1.0,
                        Kind::Unspecified,
                    ));
            }
            $out.scratch_final.append(&mut $out.scratch_normal);
            if matches!($state.lock_manager.phase(), LockPhase::Pending) {
                let maybe_ready_locker = $state.lock_manager.record_pending_frame(&output_name);
                if let Some(locker) = maybe_ready_locker {
                    locker.lock();
                    let _ = $state.lock_manager.confirm_locked();
                    $state.refresh_lock_focus();
                    tracing::info!("session lock confirmed after cleared frames");
                }
            }
        }
    };
}
