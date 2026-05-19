use smithay_client_toolkit::{
    seat::pointer::{PointerEvent, PointerEventKind, PointerHandler},
    shell::WaylandSurface,
};
use wayland_client::{protocol::wl_pointer, Connection, QueueHandle};

use crate::{
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
                    SurfaceKind::Calendar | SurfaceKind::None => {}
                }
                self.pointer_surface = SurfaceKind::None;
                continue;
            }

            if self.pointer_surface == SurfaceKind::Launcher {
                if let Some(ev) = translate_pointer_event(&event.kind, event.position) {
                    let tree = if self.app_view_open {
                        crate::app_view::build_app_view_widget_tree(
                            crate::LAUNCHER_WIDTH,
                            crate::LAUNCHER_HEIGHT,
                            &self.launcher_state.apps,
                            self.app_view_category,
                            &self.icon_cache,
                            &self.search_query,
                        )
                    } else {
                        crate::ui_preview::build_ui_preview_widget_tree(
                            crate::LAUNCHER_WIDTH,
                            crate::LAUNCHER_HEIGHT,
                            &self.launcher_state.apps,
                            &self.icon_cache,
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
                                x: event.position.0 as i32,
                                y: event.position.1 as i32,
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
                                if let Some(widget) =
                                    crate::widget_traversal::find_widget_at_path(
                                        &*tree,
                                        &clicked_path,
                                    )
                                {
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
                if let Some(action) = action {
                    match self.pointer_surface {
                        SurfaceKind::Panel => self.handle_panel_click(qh, action),
                        SurfaceKind::Launcher => self.handle_launcher_click(qh, action),
                        SurfaceKind::WorkspacePopup => self.handle_workspace_click(qh, action),
                        SurfaceKind::NetworkPopup => {}
                        SurfaceKind::Calendar => {}
                        SurfaceKind::None => {}
                    }
                }
            }
        }
    }
}
