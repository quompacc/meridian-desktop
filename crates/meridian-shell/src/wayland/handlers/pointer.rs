use smithay_client_toolkit::{
    seat::pointer::{PointerEvent, PointerEventKind, PointerHandler},
    shell::WaylandSurface,
};
use wayland_client::{protocol::wl_pointer, Connection, QueueHandle};

use crate::{
    context_menu,
    network_popup::popup_hit_test,
    wayland::{RepaintReason, SurfaceKind},
};

use super::{pointer_translate::translate_pointer_event, MeridianShell};

impl PointerHandler for MeridianShell {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            self.pointer_surface = if &event.surface == self.panel.wl_surface() {
                SurfaceKind::Panel
            } else if &event.surface == self.launcher_layer.wl_surface() {
                SurfaceKind::Launcher
            } else if &event.surface == self.calendar_layer.wl_surface() {
                SurfaceKind::Calendar
            } else if &event.surface == self.workspace_layer.wl_surface() {
                SurfaceKind::WorkspacePopup
            } else if &event.surface == self.network_layer.wl_surface() {
                SurfaceKind::NetworkPopup
            } else if &event.surface == self.thumbnail_layer.wl_surface() {
                SurfaceKind::ThumbnailPopup
            } else {
                SurfaceKind::None
            };
            self.pointer_position = event.position;

            if let PointerEventKind::Leave { .. } = event.kind {
                self.pointer_position = (-1.0, -1.0);
                match self.pointer_surface {
                    SurfaceKind::Panel => {
                        if self.panel_widget_state.is_some() {
                            self.panel_widget_state = None;
                        }
                        self.thumbnail_hover_app_idx = None;
                        self.thumbnail_hover_since = None;
                        if self.thumbnail_popup_open {
                            self.close_thumbnail_popup(crate::wayland::CommitReason::Input);
                        }
                        self.draw_panel(qh, RepaintReason::Pointer);
                    }
                    SurfaceKind::Launcher => {
                        if self.ui_preview_widget_state.is_some() {
                            self.ui_preview_widget_state = None;
                        }
                        self.draw_launcher(qh, RepaintReason::Pointer)
                    }
                    SurfaceKind::WorkspacePopup => {
                        self.draw_workspace_popup(qh, RepaintReason::Pointer)
                    }
                    SurfaceKind::NetworkPopup => {
                        self.draw_network_popup(qh, RepaintReason::Pointer)
                    }
                    SurfaceKind::ThumbnailPopup | SurfaceKind::Calendar | SurfaceKind::None => {}
                }
                self.pointer_surface = SurfaceKind::None;
                continue;
            }

            if self.pointer_surface == SurfaceKind::Launcher {
                // In fullscreen mode, translate position to launcher-local coords.
                // A click outside the visual launcher area dismisses it.
                let local_pos = if self.launcher_is_fullscreen {
                    let (px, py) = event.position;
                    let vx = self.launcher_visual_x as f64;
                    let vy = self.launcher_visual_y as f64;
                    let (lw, lh) = (crate::LAUNCHER_WIDTH as f64, crate::LAUNCHER_HEIGHT as f64);
                    if px < vx || px >= vx + lw || py < vy || py >= vy + lh {
                        if let PointerEventKind::Press { button: 0x110, .. } = event.kind {
                            self.close_launcher_after_launch(qh, RepaintReason::Pointer);
                        }
                        continue;
                    }
                    (px - vx, py - vy)
                } else {
                    event.position
                };
                // App grid direct hit test (left-click in grid area, bypasses widget tree)
                if self.app_view_open {
                    if let PointerEventKind::Press { button: 0x110, .. } = event.kind {
                        let cx = local_pos.0 as i32;
                        let cy = local_pos.1 as i32;
                        let grid_start = crate::app_view::APP_GRID_HEADER_H;
                        let grid_end = crate::LAUNCHER_HEIGHT as i32
                            - crate::app_view::APP_GRID_FOOTER_H;
                        if cy >= grid_start && cy < grid_end {
                            let grid_x = crate::app_view::app_grid_content_x(
                                crate::LAUNCHER_WIDTH,
                            );
                            let rel_y = cy - grid_start + self.app_view_scroll_y;
                            let rel_x = cx - grid_x;
                            if rel_x >= 0
                                && rel_x < crate::app_view::APP_GRID_CONTENT_W
                                && rel_y >= 0
                            {
                                let row = (rel_y / crate::app_view::APP_GRID_ROW_H) as usize;
                                let col = (rel_x
                                    / (crate::app_view::APP_CARD_WIDTH + 8))
                                    as usize;
                                if col < crate::app_view::APP_GRID_COLS {
                                    let idx = row * crate::app_view::APP_GRID_COLS + col;
                                    let filtered = crate::app_view::collect_filtered_apps(
                                        &self.launcher_state.apps,
                                        self.app_view_category,
                                        &self.search_query,
                                        &self.icon_cache,
                                    );
                                    if let Some(app) = filtered.get(idx) {
                                        let exec = app.program.clone();
                                        self.dispatch_widget_action(
                                            qh,
                                            crate::widget_action::WidgetAction::LaunchExec(exec),
                                        );
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(ev) = translate_pointer_event(&event.kind, local_pos) {
                    let tree = if self.launcher_settings_open {
                        crate::settings_view::build_settings_widget_tree(
                            crate::LAUNCHER_WIDTH,
                            crate::LAUNCHER_HEIGHT,
                            self.settings_category,
                            &self.available_themes,
                            &self.theme_name,
                            &self.available_wallpapers,
                            &self.wallpaper_thumbnails,
                            self.wallpaper_path.as_deref(),
                            self.wallpaper_mode,
                            &self.pinned_apps,
                            self.settings_pinned_adding,
                            &self.launcher_state.apps,
                            &self.icon_cache,
                            None,
                        )
                    } else if self.app_view_open {
                        crate::app_view::build_app_view_widget_tree(
                            crate::LAUNCHER_WIDTH,
                            crate::LAUNCHER_HEIGHT,
                            &self.launcher_state.apps,
                            self.app_view_category,
                            &self.icon_cache,
                            &self.search_query,
                            self.app_view_scroll_y,
                            None,
                        )
                    } else {
                        crate::ui_preview::build_ui_preview_widget_tree(
                            crate::LAUNCHER_WIDTH,
                            crate::LAUNCHER_HEIGHT,
                            &self.launcher_state.apps,
                            &self.icon_cache,
                            None,
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
                                self.ui_preview_widget_state.as_ref(),
                                &ev,
                                path.as_ref(),
                            );
                            let new_state = super::pointer_state::apply_pointer_event(
                                self.ui_preview_widget_state.clone(),
                                &ev,
                                path,
                            );
                            if new_state != self.ui_preview_widget_state {
                                self.ui_preview_widget_state = new_state;
                                self.draw_launcher(qh, RepaintReason::Pointer);
                            }
                            if let Some(clicked_path) = clicked_path {
                                if let Some(widget) = crate::widget_traversal::find_widget_at_path(
                                    &*tree,
                                    &clicked_path,
                                ) {
                                    if let Some(action) =
                                        widget.id().and_then(crate::widget_action::action_for_id)
                                    {
                                        self.dispatch_widget_action(qh, action);
                                    } else if let Some(exec) = widget.launch_exec() {
                                        self.dispatch_widget_action(
                                            qh,
                                            crate::widget_action::WidgetAction::LaunchExec(
                                                exec.to_string(),
                                            ),
                                        );
                                    } else if let Some((program, args)) = widget.launch_info() {
                                        self.dispatch_widget_action(
                                            qh,
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
                continue;
            }

            if self.pointer_surface == SurfaceKind::Panel {
                if let Some(ev) = translate_pointer_event(&event.kind, event.position) {
                    let tree = crate::panel_view::build_panel_widget_tree(
                        self.width,
                        &self.pinned_apps,
                        &self.panel_window_entries(self.panel_active_workspace()),
                        self.network_controller.state(),
                        self.network_popup_open,
                        self.panel_active_workspace(),
                        9,
                        &self.last_clock,
                        &self.icon_cache,
                        None, // screenshot_icon — nur für Hover-Layout, Icon irrelevant
                    );
                    let pixel_size = meridian_ui::PixelSize {
                        width: self.width,
                        height: crate::PANEL_HEIGHT,
                    };
                    if let Ok(layout) = meridian_ui::compute_layout(&*tree, pixel_size) {
                        let pos = meridian_ui::PointerPosition {
                            x: event.position.0 as i32,
                            y: event.position.1 as i32,
                        };
                        let path = meridian_ui::hit_test(&layout, pos);
                        let new_state = super::pointer_state::apply_pointer_event(
                            self.panel_widget_state.clone(),
                            &ev,
                            path,
                        );
                        if new_state != self.panel_widget_state {
                            self.panel_widget_state = new_state;
                            self.draw_panel(qh, RepaintReason::Pointer);
                        }
                    }
                }
                // No continue — Press events must fall through to the ClickZone handler below.
            }

            // Thumbnail hover detection on panel
            if self.pointer_surface == SurfaceKind::Panel {
                if let PointerEventKind::Motion { .. } = event.kind {
                    let hovered_pinned_idx = self.panel_state.clicks.iter()
                        .find(|z| {
                            z.rect.contains(event.position.0, event.position.1)
                                && matches!(z.action, crate::wayland::ClickAction::LaunchPinnedApp(_))
                        })
                        .and_then(|z| {
                            if let crate::wayland::ClickAction::LaunchPinnedApp(idx) = z.action {
                                Some(idx)
                            } else {
                                None
                            }
                        });

                    let ws = self.panel_active_workspace();
                    let has_windows = hovered_pinned_idx
                        .and_then(|idx| self.pinned_apps.get(idx))
                        .map(|app| {
                            crate::wayland::state::pinned_app_has_windows_on_workspace(
                                app, &self.windows, ws,
                            )
                        })
                        .unwrap_or(false);

                    let new_hover = if has_windows { hovered_pinned_idx } else { None };

                    if new_hover != self.thumbnail_hover_app_idx {
                        self.thumbnail_hover_app_idx = new_hover;
                        self.thumbnail_hover_since = new_hover.map(|_| std::time::Instant::now());
                        if new_hover.is_none() && self.thumbnail_popup_open {
                            self.close_thumbnail_popup(crate::wayland::CommitReason::Input);
                        }
                        // Prefetch thumbnails as soon as hover begins so they
                        // are cached by the time the 400ms popup-open delay
                        // elapses. Without prefetch the popup briefly opens at
                        // max placeholder width and visibly snaps smaller.
                        if let Some(idx) = new_hover {
                            if let Some(app) = self.pinned_apps.get(idx).cloned() {
                                let window_ids = crate::wayland::state::pinned_app_window_ids(
                                    &app, &self.windows, ws,
                                );
                                for id in window_ids.iter().take(crate::THUMBNAIL_MAX_WINDOWS) {
                                    let cmd = meridian_ipc::ShellCommand::CaptureWindowThumbnail {
                                        id: id.clone(),
                                        max_width: crate::THUMBNAIL_THUMB_W,
                                        max_height: crate::THUMBNAIL_THUMB_H,
                                    };
                                    let _ = self.ipc.send(&cmd);
                                }
                            }
                        }
                    }

                    // Popup is opened from tick() after THUMBNAIL_HOVER_DELAY_MS
                }
            }

            if self.pointer_surface == SurfaceKind::Launcher
                && matches!(
                    event.kind,
                    PointerEventKind::Motion { .. } | PointerEventKind::Press { .. }
                )
                && self
                    .launcher_state
                    .update_hover_selection(event.position.0, event.position.1)
            {
                self.draw_launcher(qh, RepaintReason::Pointer);
            }
            if self.workspace_popup_open
                && self.pointer_surface == SurfaceKind::WorkspacePopup
                && matches!(event.kind, PointerEventKind::Motion { .. })
            {
                self.draw_workspace_popup(qh, RepaintReason::Pointer);
            }

            // Context menu: hover tracking and right-click opening
            if self.pointer_surface == SurfaceKind::Launcher {
                if let PointerEventKind::Motion { .. } = event.kind {
                    if let Some(ref mut cm) = self.context_menu {
                        let items = context_menu::item_list(cm.is_terminal, cm.is_pinned);
                        let n = items.len();
                        let new_hover =
                            context_menu::hit_item(cm, n, event.position.0, event.position.1);
                        if new_hover != cm.hover_idx {
                            cm.hover_idx = new_hover;
                            self.draw_launcher(qh, RepaintReason::Pointer);
                        }
                    }
                }

                if let PointerEventKind::Press { button: 0x111, .. } = event.kind {
                    // Right-click: open or replace context menu for the hovered app.
                    self.context_menu = None;

                    // App grid direct hit test for right-click
                    if self.app_view_open {
                        let cx = event.position.0 as i32;
                        let cy = event.position.1 as i32;
                        let grid_start = crate::app_view::APP_GRID_HEADER_H;
                        let grid_end = crate::LAUNCHER_HEIGHT as i32
                            - crate::app_view::APP_GRID_FOOTER_H;
                        if cy >= grid_start && cy < grid_end {
                            let grid_x =
                                crate::app_view::app_grid_content_x(crate::LAUNCHER_WIDTH);
                            let rel_y = cy - grid_start + self.app_view_scroll_y;
                            let rel_x = cx - grid_x;
                            if rel_x >= 0
                                && rel_x < crate::app_view::APP_GRID_CONTENT_W
                                && rel_y >= 0
                            {
                                let row =
                                    (rel_y / crate::app_view::APP_GRID_ROW_H) as usize;
                                let col =
                                    (rel_x / (crate::app_view::APP_CARD_WIDTH + 8)) as usize;
                                if col < crate::app_view::APP_GRID_COLS {
                                    let idx = row * crate::app_view::APP_GRID_COLS + col;
                                    let filtered = crate::app_view::collect_filtered_apps(
                                        &self.launcher_state.apps,
                                        self.app_view_category,
                                        &self.search_query,
                                        &self.icon_cache,
                                    );
                                    if let Some(app) = filtered.get(idx) {
                                        let exec: Box<str> = app.program.clone().into();
                                        let app_name: Box<str> = app.name.clone().into();
                                        let is_terminal = app.terminal;
                                        let is_pinned = self
                                            .pinned_apps
                                            .iter()
                                            .any(|p| p.program == exec.as_ref());
                                        let items =
                                            context_menu::item_list(is_terminal, is_pinned);
                                        let (mx, my) = context_menu::clamp_position(
                                            event.position.0 as i32,
                                            event.position.1 as i32,
                                            items.len(),
                                            crate::LAUNCHER_WIDTH as i32,
                                            crate::LAUNCHER_HEIGHT as i32,
                                        );
                                        self.context_menu =
                                            Some(context_menu::ContextMenuState {
                                                x: mx,
                                                y: my,
                                                app_name,
                                                exec,
                                                is_terminal,
                                                is_pinned,
                                                hover_idx: None,
                                            });
                                        self.draw_launcher(qh, RepaintReason::Pointer);
                                        continue;
                                    }
                                }
                            }
                        }
                    }

                    let tree = if self.app_view_open {
                        crate::app_view::build_app_view_widget_tree(
                            crate::LAUNCHER_WIDTH,
                            crate::LAUNCHER_HEIGHT,
                            &self.launcher_state.apps,
                            self.app_view_category,
                            &self.icon_cache,
                            &self.search_query,
                            self.app_view_scroll_y,
                            None,
                        )
                    } else {
                        crate::ui_preview::build_ui_preview_widget_tree(
                            crate::LAUNCHER_WIDTH,
                            crate::LAUNCHER_HEIGHT,
                            &self.launcher_state.apps,
                            &self.icon_cache,
                            None,
                        )
                    };
                    let pixel_size = meridian_ui::PixelSize {
                        width: crate::LAUNCHER_WIDTH,
                        height: crate::LAUNCHER_HEIGHT,
                    };
                    if let Ok(layout) = meridian_ui::compute_layout(&*tree, pixel_size) {
                        let pos = meridian_ui::PointerPosition {
                            x: event.position.0 as i32,
                            y: event.position.1 as i32,
                        };
                        let path = meridian_ui::hit_test(&layout, pos);
                        if let Some(path) = path {
                            if let Some(widget) =
                                crate::widget_traversal::find_widget_at_path(&*tree, &path)
                            {
                                if let Some(exec) = widget.launch_exec() {
                                    let app = self
                                        .launcher_state
                                        .apps
                                        .iter()
                                        .find(|a| a.program == exec);
                                    let app_name: Box<str> = app
                                        .map(|a| a.name.as_str())
                                        .unwrap_or(exec)
                                        .into();
                                    let is_terminal =
                                        app.map(|a| a.terminal).unwrap_or(false);
                                    let exec_str: Box<str> = exec.into();
                                    let is_pinned = self
                                        .pinned_apps
                                        .iter()
                                        .any(|p| p.program == exec_str.as_ref());
                                    let items = context_menu::item_list(is_terminal, is_pinned);
                                    let (mx, my) = context_menu::clamp_position(
                                        event.position.0 as i32,
                                        event.position.1 as i32,
                                        items.len(),
                                        crate::LAUNCHER_WIDTH as i32,
                                        crate::LAUNCHER_HEIGHT as i32,
                                    );
                                    self.context_menu =
                                        Some(context_menu::ContextMenuState {
                                            x: mx,
                                            y: my,
                                            app_name,
                                            exec: exec_str,
                                            is_terminal,
                                            is_pinned,
                                            hover_idx: None,
                                        });
                                    self.draw_launcher(qh, RepaintReason::Pointer);
                                }
                            }
                        }
                    }
                    continue;
                }
            }

            if let PointerEventKind::Axis { vertical, .. } = event.kind {
                if self.pointer_surface == SurfaceKind::Launcher
                    && self.launcher_state.view() == crate::launcher::LauncherView::TileStart
                {
                    let step_px: i32 = 60;
                    let delta_px = if vertical.discrete != 0 {
                        vertical.discrete * step_px
                    } else {
                        vertical.absolute as i32
                    };
                    if delta_px != 0
                        && self.launcher_state.scroll_tile_area(
                            delta_px,
                            self.launcher_state.tile_viewport_h_cache,
                            self.launcher_state.tile_content_h_cache,
                        )
                    {
                        self.draw_launcher(qh, RepaintReason::Pointer);
                    }
                } else if self.pointer_surface == SurfaceKind::Launcher && self.app_view_open {
                    let step_px: i32 = 60;
                    let delta_px = if vertical.discrete != 0 {
                        vertical.discrete * step_px
                    } else {
                        vertical.absolute as i32
                    };
                    if delta_px != 0 {
                        let filtered = crate::app_view::collect_filtered_apps(
                            &self.launcher_state.apps,
                            self.app_view_category,
                            &self.search_query,
                            &self.icon_cache,
                        );
                        let content_h = ((filtered.len() + crate::app_view::APP_GRID_COLS - 1)
                            / crate::app_view::APP_GRID_COLS) as i32
                            * crate::app_view::APP_GRID_ROW_H;
                        let grid_h = crate::LAUNCHER_HEIGHT as i32
                            - crate::app_view::APP_GRID_HEADER_H
                            - crate::app_view::APP_GRID_FOOTER_H;
                        let max_scroll = (content_h - grid_h).max(0);
                        let new_scroll =
                            (self.app_view_scroll_y + delta_px).clamp(0, max_scroll);
                        if new_scroll != self.app_view_scroll_y {
                            self.app_view_scroll_y = new_scroll;
                            self.draw_launcher(qh, RepaintReason::Pointer);
                        }
                    }
                }
            }

            // Context menu: left-click — execute item or dismiss
            if let PointerEventKind::Press { button: 0x110, .. } = event.kind {
                if self.pointer_surface == SurfaceKind::Launcher {
                    if let Some(cm) = self.context_menu.take() {
                        let items = context_menu::item_list(cm.is_terminal, cm.is_pinned);
                        let n = items.len();
                        if let Some(idx) =
                            context_menu::hit_item(&cm, n, event.position.0, event.position.1)
                        {
                            let action = items[idx].1;
                            self.handle_context_menu_action(qh, action, &cm);
                            self.draw_launcher(qh, RepaintReason::Pointer);
                            continue;
                        }
                        // Click outside menu: dismiss and let the event fall through.
                        self.draw_launcher(qh, RepaintReason::Pointer);
                    }
                }
            }

            if let PointerEventKind::Press { button: 0x110, .. } = event.kind {
                let action = match self.pointer_surface {
                    SurfaceKind::Panel => self
                        .panel_state
                        .clicks
                        .iter()
                        .find(|zone| zone.rect.contains(event.position.0, event.position.1))
                        .map(|zone| zone.action.clone()),
                    SurfaceKind::Launcher => self
                        .launcher_state
                        .clicks
                        .iter()
                        .find(|zone| zone.rect.contains(event.position.0, event.position.1))
                        .map(|zone| zone.action.clone()),
                    SurfaceKind::WorkspacePopup => self
                        .workspace_state
                        .clicks
                        .iter()
                        .find(|zone| zone.rect.contains(event.position.0, event.position.1))
                        .map(|zone| zone.action.clone()),
                    SurfaceKind::NetworkPopup => {
                        if popup_hit_test(
                            self.network_width,
                            self.network_height,
                            event.position.0,
                            event.position.1,
                        ) {
                            None
                        } else {
                            Some(crate::wayland::ClickAction::ToggleNetworkPopup)
                        }
                    }
                    SurfaceKind::ThumbnailPopup => None,
                    SurfaceKind::Calendar => None,
                    SurfaceKind::None => None,
                };
                let keep_workspace_popup_open = matches!(
                    action,
                    Some(crate::wayland::ClickAction::ToggleWorkspacePopup)
                );
                let keep_network_popup_open = matches!(
                    action,
                    Some(crate::wayland::ClickAction::ToggleNetworkPopup)
                );
                if self.workspace_popup_open
                    && self.pointer_surface != SurfaceKind::WorkspacePopup
                    && !keep_workspace_popup_open
                {
                    self.close_workspace_popup(crate::wayland::CommitReason::Input);
                    self.draw_panel(qh, RepaintReason::Pointer);
                }
                if self.network_popup_open
                    && self.pointer_surface != SurfaceKind::NetworkPopup
                    && !keep_network_popup_open
                {
                    self.close_network_popup(crate::wayland::CommitReason::Input);
                    self.draw_panel(qh, RepaintReason::Pointer);
                }
                // Close launcher on any click outside the launcher surface,
                // but only when the click itself is not a launcher-toggle action
                // (ToggleLauncher lets toggle_launcher() handle the close cleanly).
                let is_launcher_toggle =
                    matches!(action, Some(crate::wayland::ClickAction::ToggleLauncher));
                if self.launcher_state.open
                    && self.pointer_surface != SurfaceKind::Launcher
                    && !is_launcher_toggle
                {
                    self.close_launcher_after_launch(qh, RepaintReason::Pointer);
                }
                if let Some(action) = action {
                    match self.pointer_surface {
                        SurfaceKind::Panel => self.handle_panel_click(qh, action),
                        SurfaceKind::Launcher => self.handle_launcher_click(qh, action),
                        SurfaceKind::WorkspacePopup => self.handle_workspace_click(qh, action),
                        SurfaceKind::NetworkPopup => {}
                        SurfaceKind::ThumbnailPopup => {}
                        SurfaceKind::Calendar => {}
                        SurfaceKind::None => {}
                    }
                }
            }
        }
    }
}
