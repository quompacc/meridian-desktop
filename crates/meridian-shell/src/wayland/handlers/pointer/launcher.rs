macro_rules! handle_launcher_pointer {
    ($shell:ident, $qh:ident, $event:ident) => {{
            if $shell.pointer_surface == SurfaceKind::Launcher {
                // Translate to content-buffer coordinates first.
                // In fullscreen mode the launcher surface covers the full screen but
                // draw_overlay / hit tests operate on the fixed LAUNCHER_WxH content
                // buffer, so we need coords relative to that buffer, not the surface.
                // A click outside the visual area is discarded early.
                let local_pos = if $shell.launcher_is_fullscreen {
                    let (px, py) = $event.position;
                    let vx = $shell.launcher_visual_x as f64;
                    let vy = $shell.launcher_visual_y as f64;
                    let (lw, lh) = (crate::LAUNCHER_WIDTH as f64, crate::LAUNCHER_HEIGHT as f64);
                    if px < vx || px >= vx + lw || py < vy || py >= vy + lh {
                        if let PointerEventKind::Press { button: 0x110, .. } = $event.kind {
                            $shell.close_launcher_after_launch($qh, RepaintReason::Pointer);
                        }
                        continue;
                    }
                    (px - vx, py - vy)
                } else {
                    $event.position
                };

                // ── Step 0: Context-menu left-click — before the widget tree so clicking
                //    a menu item does not also fire the underlying tile.
                if let PointerEventKind::Press { button: 0x110, .. } = $event.kind {
                    if let Some(cm) = $shell.context_menu.take() {
                        let items = context_menu::item_list(
                            cm.is_terminal,
                            cm.is_pinned,
                            cm.running_window_id.is_some(),
                        );
                        let n = items.len();
                        if let Some(idx) = context_menu::hit_item(&cm, n, local_pos.0, local_pos.1)
                        {
                            let action = items[idx].1;
                            $shell.handle_context_menu_action($qh, action, &cm);
                            $shell.draw_launcher($qh, RepaintReason::Pointer);
                            continue;
                        }
                        // Click outside menu: dismiss and fall through to widget tree.
                        $shell.draw_launcher($qh, RepaintReason::Pointer);
                    }
                }

                // ── Step 2: Context-menu hover tracking.
                if let PointerEventKind::Motion { .. } = $event.kind {
                    if let Some(ref mut cm) = $shell.context_menu {
                        let items = context_menu::item_list(
                            cm.is_terminal,
                            cm.is_pinned,
                            cm.running_window_id.is_some(),
                        );
                        let n = items.len();
                        let new_hover = context_menu::hit_item(cm, n, local_pos.0, local_pos.1);
                        if new_hover != cm.hover_idx {
                            cm.hover_idx = new_hover;
                            $shell.draw_launcher($qh, RepaintReason::Pointer);
                        }
                    }
                }

                // ── Step 2b: Command-palette hover tracking.
                if let PointerEventKind::Motion { .. } = $event.kind {
                    if !$shell.launcher_settings_open {
                        let search_active = !$shell.search_query.is_empty();
                        let new_bento = if !search_active {
                            crate::app_view::hit_bento_tile(
                                local_pos.0 as i32,
                                local_pos.1 as i32,
                                crate::launcher::LauncherCategory::ALL.len(),
                            )
                        } else {
                            None
                        };
                        let new_app = {
                            let hit = crate::app_view::hit_app_row(
                                local_pos.0 as i32,
                                local_pos.1 as i32,
                                $shell.app_view_scroll_y,
                                crate::LAUNCHER_HEIGHT,
                                search_active,
                            );
                            let filtered = crate::app_view::collect_palette_apps(
                                &$shell.launcher_state.apps,
                                &$shell.search_query,
                                &$shell.hidden_execs,
                                $shell.launcher_state.category,
                                &$shell.pinned_apps,
                            );
                            hit.filter(|&i| i < filtered.len())
                        };
                        let new_settings = crate::app_view::hit_header_settings(
                            local_pos.0 as i32,
                            local_pos.1 as i32,
                            crate::LAUNCHER_WIDTH,
                        );
                        let new_pwr = crate::app_view::hit_footer_power_btn(
                            local_pos.0 as i32,
                            local_pos.1 as i32,
                            crate::LAUNCHER_HEIGHT,
                        );
                        let changed = new_bento != $shell.hovered_bento_idx
                            || new_app != $shell.hovered_app_card_idx
                            || new_settings != $shell.settings_hovered
                            || new_pwr != $shell.hovered_power_btn;
                        if changed {
                            $shell.hovered_bento_idx = new_bento;
                            $shell.hovered_app_card_idx = new_app;
                            $shell.settings_hovered = new_settings;
                            $shell.hovered_power_btn = new_pwr;
                        }
                    }
                }

                // ── Step 3: Right-click — open context menu for app under cursor.
                if let PointerEventKind::Press { button: 0x111, .. } = $event.kind {
                    $shell.context_menu = None;
                    if !$shell.launcher_settings_open {
                        let search_active = !$shell.search_query.is_empty();
                        let filtered = crate::app_view::collect_palette_apps(
                            &$shell.launcher_state.apps,
                            &$shell.search_query,
                            &$shell.hidden_execs,
                            $shell.launcher_state.category,
                            &$shell.pinned_apps,
                        );
                        let hit = crate::app_view::hit_app_row(
                            local_pos.0 as i32,
                            local_pos.1 as i32,
                            $shell.app_view_scroll_y,
                            crate::LAUNCHER_HEIGHT,
                            search_active,
                        );
                        if let Some(idx) = hit.filter(|&i| i < filtered.len()) {
                            let app = filtered[idx];
                            let exec_str: Box<str> = app.program.clone().into();
                            let app_name: Box<str> = app.name.clone().into();
                            let is_terminal = app.terminal;
                            let is_pinned = $shell
                                .pinned_apps
                                .iter()
                                .any(|p| p.program == exec_str.as_ref());
                            let running_window_id = $shell
                                .windows
                                .iter()
                                .find(|w| {
                                    w.app_id
                                        .as_deref()
                                        .map(|a| {
                                            a.eq_ignore_ascii_case(&exec_str)
                                                || a.to_lowercase()
                                                    .contains(&exec_str.to_lowercase())
                                        })
                                        .unwrap_or(false)
                                })
                                .map(|w| w.id.clone());
                            let is_running = running_window_id.is_some();
                            let items = context_menu::item_list(is_terminal, is_pinned, is_running);
                            let (mx, my) = context_menu::clamp_position(
                                local_pos.0 as i32,
                                local_pos.1 as i32,
                                items.len(),
                                crate::LAUNCHER_WIDTH as i32,
                                crate::LAUNCHER_HEIGHT as i32,
                            );
                            $shell.context_menu = Some(context_menu::ContextMenuState {
                                x: mx,
                                y: my,
                                app_name,
                                exec: exec_str,
                                is_terminal,
                                is_pinned,
                                running_window_id,
                                hover_idx: None,
                            });
                            $shell.draw_launcher($qh, RepaintReason::Pointer);
                            continue;
                        }
                    }
                    // Fall through to widget tree for settings right-click
                    let tree = if $shell.launcher_settings_open {
                        let system_info = crate::sysinfo::SystemInfo::gather();
                        crate::settings_view::build_settings_widget_tree(
                            crate::LAUNCHER_WIDTH,
                            crate::LAUNCHER_HEIGHT,
                            $shell.settings_category,
                            &$shell.settings_search,
                            &$shell.available_themes,
                            &$shell.theme_name,
                            &$shell.available_wallpapers,
                            &$shell.wallpaper_thumbnails,
                            $shell.wallpaper_path.as_deref(),
                            $shell.wallpaper_mode,
                            $shell.cursor_size,
                            $shell.available_cursor_themes.as_slice(),
                            $shell.cursor_theme.as_str(),
                            $shell.idle_timeout_secs,
                            &$shell.pinned_apps,
                            &$shell.output_workspaces,
                            $shell.display_mode_dropdown_open,
                            &$shell.printer_snapshot,
                            &$shell.audio_snapshot,
                            &system_info,
                            $shell.network_controller.state(),
                            $shell.network_profiles.as_slice(),
                            &$shell.bluetooth_snapshot,
                            $shell.wifi_networks.as_slice(),
                            $shell.settings_pinned_adding,
                            &$shell.launcher_state.apps,
                            &$shell.icon_cache,
                            None,
                            $shell.default_apps_index.as_ref(),
                            &$shell.default_apps_current,
                            $shell.default_apps_picker_open,
                            &crate::ui::tokens::theme_from_config(&$shell.theme),
                        )
                    } else {
                        return;
                    };
                    let pixel_size = meridian_ui::PixelSize {
                        width: crate::LAUNCHER_WIDTH,
                        height: crate::LAUNCHER_HEIGHT,
                    };
                    if let Ok(layout) = meridian_ui::compute_layout(&*tree, pixel_size) {
                        let pos = meridian_ui::PointerPosition {
                            x: local_pos.0 as i32,
                            y: local_pos.1 as i32,
                        };
                        if let Some(path) = meridian_ui::hit_test(&layout, pos) {
                            if let Some(widget) =
                                crate::widget_traversal::find_widget_at_path(&*tree, &path)
                            {
                                if let Some(exec) = widget.launch_exec() {
                                    let app =
                                        $shell.launcher_state.apps.iter().find(|a| a.program == exec);
                                    let app_name: Box<str> =
                                        app.map(|a| a.name.as_str()).unwrap_or(exec).into();
                                    let is_terminal = app.map(|a| a.terminal).unwrap_or(false);
                                    let exec_str: Box<str> = exec.into();
                                    let is_pinned = $shell
                                        .pinned_apps
                                        .iter()
                                        .any(|p| p.program == exec_str.as_ref());
                                    let running_window_id = $shell
                                        .windows
                                        .iter()
                                        .find(|w| {
                                            w.app_id
                                                .as_deref()
                                                .map(|a| {
                                                    a.eq_ignore_ascii_case(&exec_str)
                                                        || a.to_lowercase()
                                                            .contains(&exec_str.to_lowercase())
                                                })
                                                .unwrap_or(false)
                                        })
                                        .map(|w| w.id.clone());
                                    let is_running = running_window_id.is_some();
                                    let items =
                                        context_menu::item_list(is_terminal, is_pinned, is_running);
                                    let (mx, my) = context_menu::clamp_position(
                                        local_pos.0 as i32,
                                        local_pos.1 as i32,
                                        items.len(),
                                        crate::LAUNCHER_WIDTH as i32,
                                        crate::LAUNCHER_HEIGHT as i32,
                                    );
                                    $shell.context_menu = Some(context_menu::ContextMenuState {
                                        x: mx,
                                        y: my,
                                        app_name,
                                        exec: exec_str,
                                        is_terminal,
                                        is_pinned,
                                        running_window_id,
                                        hover_idx: None,
                                    });
                                    $shell.draw_launcher($qh, RepaintReason::Pointer);
                                }
                            }
                        }
                    }
                    continue;
                }

                // ── Step 4: Scroll in the launcher.
                if let PointerEventKind::Axis { vertical, .. } = $event.kind {
                    if !$shell.launcher_settings_open {
                        let step_px: i32 = 60;
                        let delta_px = if vertical.discrete != 0 {
                            $shell.launcher_scroll_remainder = 0.0;
                            -vertical.discrete * step_px
                        } else {
                            $shell.launcher_scroll_remainder += -vertical.absolute * 4.0;
                            if $shell.launcher_scroll_remainder.abs() < 6.0 {
                                0
                            } else {
                                let delta = $shell.launcher_scroll_remainder.trunc() as i32;
                                $shell.launcher_scroll_remainder -= f64::from(delta);
                                delta
                            }
                        };
                        let max_scroll = crate::app_view::max_scroll_for_palette(
                            &$shell.launcher_state.apps,
                            &$shell.search_query,
                            &$shell.hidden_execs,
                            $shell.launcher_state.category,
                            &$shell.pinned_apps,
                            crate::LAUNCHER_HEIGHT,
                        );
                        if delta_px != 0 {
                            let new_scroll =
                                ($shell.app_view_scroll_y + delta_px).clamp(0, max_scroll);
                            if new_scroll != $shell.app_view_scroll_y {
                                $shell.app_view_scroll_y = new_scroll;
                                $shell.draw_launcher($qh, RepaintReason::Pointer);
                            }
                        }
                    }
                }

                // ── Step 5: Command-palette left-click hit test.
                if !$shell.launcher_settings_open {
                    if let PointerEventKind::Press { button: 0x110, .. } = $event.kind {
                        let cx = local_pos.0 as i32;
                        let cy = local_pos.1 as i32;
                        let search_active = !$shell.search_query.is_empty();

                        // Category strip
                        if let Some(idx) =
                            crate::app_view::hit_bento_tile(
                                cx,
                                cy,
                                crate::launcher::LauncherCategory::ALL.len(),
                            )
                        {
                            if let Some(category) =
                                crate::launcher::LauncherCategory::ALL.get(idx).copied()
                            {
                                $shell.launcher_state.category = category;
                                $shell.app_view_scroll_y = 0;
                                $shell.launcher_selected_idx = None;
                                $shell.draw_launcher($qh, RepaintReason::Pointer);
                                continue;
                            }
                        }

                        // App grid / search results
                        if let Some(idx) = crate::app_view::hit_app_row(
                            cx,
                            cy,
                            $shell.app_view_scroll_y,
                            crate::LAUNCHER_HEIGHT,
                            search_active,
                        ) {
                            let filtered = crate::app_view::collect_palette_apps(
                                &$shell.launcher_state.apps,
                                &$shell.search_query,
                                &$shell.hidden_execs,
                                $shell.launcher_state.category,
                                &$shell.pinned_apps,
                            );
                            if let Some(app) = filtered.get(idx) {
                                crate::launcher::LauncherState::launch_desktop_app(
                                    (*app).clone(),
                                    &mut $shell.ipc,
                                );
                                $shell.close_launcher_after_launch($qh, RepaintReason::Pointer);
                                continue;
                            }
                        }

                        // Header settings button
                        if crate::app_view::hit_header_settings(cx, cy, crate::LAUNCHER_WIDTH) {
                            $shell.dispatch_widget_action(
                                $qh,
                                crate::widget_action::WidgetAction::ToggleSettings,
                            );
                            continue;
                        }

                        // Footer power buttons
                        if let Some(btn_idx) =
                            crate::app_view::hit_footer_power_btn(cx, cy, crate::LAUNCHER_HEIGHT)
                        {
                            if let Some(action) =
                                crate::app_view::power_widget_action_for_idx(btn_idx)
                            {
                                $shell.dispatch_widget_action($qh, action);
                                continue;
                            }
                        }
                    }
                }

                // ── Step 6: Widget-tree pointer events — settings view only.
                if $shell.launcher_settings_open {
                    if let Some(ev) = translate_pointer_event(&$event.kind, local_pos) {
                        let tree = {
                            let system_info = crate::sysinfo::SystemInfo::gather();
                            crate::settings_view::build_settings_widget_tree(
                                crate::LAUNCHER_WIDTH,
                                crate::LAUNCHER_HEIGHT,
                                $shell.settings_category,
                                &$shell.settings_search,
                                &$shell.available_themes,
                                &$shell.theme_name,
                                &$shell.available_wallpapers,
                                &$shell.wallpaper_thumbnails,
                                $shell.wallpaper_path.as_deref(),
                                $shell.wallpaper_mode,
                                $shell.cursor_size,
                                $shell.available_cursor_themes.as_slice(),
                                $shell.cursor_theme.as_str(),
                                $shell.idle_timeout_secs,
                                &$shell.pinned_apps,
                                &$shell.output_workspaces,
                                $shell.display_mode_dropdown_open,
                                &$shell.printer_snapshot,
                                &$shell.audio_snapshot,
                                &system_info,
                                $shell.network_controller.state(),
                                $shell.network_profiles.as_slice(),
                                &$shell.bluetooth_snapshot,
                                $shell.wifi_networks.as_slice(),
                                $shell.settings_pinned_adding,
                                &$shell.launcher_state.apps,
                                &$shell.icon_cache,
                                None,
                                $shell.default_apps_index.as_ref(),
                                &$shell.default_apps_current,
                                $shell.default_apps_picker_open,
                                &crate::ui::tokens::theme_from_config(&$shell.theme),
                            )
                        };
                        let pixel_size = meridian_ui::PixelSize {
                            width: crate::LAUNCHER_WIDTH,
                            height: crate::LAUNCHER_HEIGHT,
                        };
                        let layout = meridian_ui::compute_layout(&*tree, pixel_size);
                        match layout {
                            Ok(layout) => {
                                let pos = meridian_ui::PointerPosition {
                                    x: local_pos.0 as i32,
                                    y: local_pos.1 as i32,
                                };
                                let path = meridian_ui::hit_test(&layout, pos);
                                let clicked_path = super::pointer_state::detect_click(
                                    $shell.ui_preview_widget_state.as_ref(),
                                    &ev,
                                    path.as_ref(),
                                );
                                let new_state = super::pointer_state::apply_pointer_event(
                                    $shell.ui_preview_widget_state.clone(),
                                    &ev,
                                    path,
                                );
                                if new_state != $shell.ui_preview_widget_state {
                                    $shell.ui_preview_widget_state = new_state;
                                    $shell.draw_launcher($qh, RepaintReason::Pointer);
                                }
                                if let Some(clicked_path) = clicked_path {
                                    if let Some(widget) =
                                        crate::widget_traversal::find_widget_at_path(
                                            &*tree,
                                            &clicked_path,
                                        )
                                    {
                                        if let Some(action) = widget
                                            .id()
                                            .and_then(crate::widget_action::action_for_id)
                                        {
                                            $shell.dispatch_widget_action($qh, action);
                                        } else if let Some(exec) = widget.launch_exec() {
                                            $shell.dispatch_widget_action(
                                                $qh,
                                                crate::widget_action::WidgetAction::LaunchExec(
                                                    exec.to_string(),
                                                ),
                                            );
                                        } else if let Some((program, args)) = widget.launch_info() {
                                            $shell.dispatch_widget_action(
                                                $qh,
                                                crate::widget_action::WidgetAction::LaunchApp {
                                                    program: program.to_string(),
                                                    args: args.to_vec(),
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("launcher layout failed: {:?}", e);
                            }
                        }
                    }
                } // end launcher_settings_open
                continue;
            }
    }};
}
