macro_rules! handle_overlays_and_desktop_pointer {
    ($shell:ident, $qh:ident, $event:ident) => {{
        $shell.pointer_surface = if &$event.surface == $shell.desktop_menu_layer.wl_surface() {
            SurfaceKind::DesktopMenu
        } else if &$event.surface == $shell.desktop_layer.wl_surface() {
            SurfaceKind::Desktop
        } else if &$event.surface == $shell.panel.wl_surface() {
            SurfaceKind::Panel
        } else if &$event.surface == $shell.launcher_layer.wl_surface() {
            SurfaceKind::Launcher
        } else if &$event.surface == $shell.calendar_layer.wl_surface() {
            SurfaceKind::Calendar
        } else if &$event.surface == $shell.workspace_layer.wl_surface() {
            SurfaceKind::WorkspacePopup
        } else if &$event.surface == $shell.network_layer.wl_surface() {
            SurfaceKind::NetworkPopup
        } else if &$event.surface == $shell.thumbnail_layer.wl_surface() {
            SurfaceKind::ThumbnailPopup
        } else {
            SurfaceKind::None
        };
        $shell.pointer_position = $event.position;

        // Screenshot region picker is also a fullscreen exclusive
        // overlay — same short-circuit pattern as the consent modal.
        if &$event.surface == $shell.region_picker_layer.wl_surface() {
            if $shell.region_picker_open {
                $shell.handle_region_picker_pointer($qh, $event);
            }
            continue;
        }

        // Screenshot consent modal grabs the input focus while open;
        // route its pointer events directly and skip the rest of the
        // surface-kind cascade so popup hover state stays stable.
        if &$event.surface == $shell.consent_layer.wl_surface() {
            if $shell.consent_open {
                $shell.handle_consent_pointer($qh, $event);
            }
            continue;
        }

        // Wi-Fi password modal: same exclusive-overlay short-circuit as the
        // consent modal.
        if &$event.surface == $shell.wifi_modal_layer.wl_surface() {
            if $shell.wifi_modal_open {
                $shell.handle_wifi_modal_pointer($qh, $event);
            }
            continue;
        }

        if let PointerEventKind::Leave { .. } = $event.kind {
            $shell.pointer_position = (-1.0, -1.0);
            match $shell.pointer_surface {
                SurfaceKind::Panel => {
                    let had_panel_widget_state = $shell.panel_widget_state.take().is_some();
                    let had_thumbnail_hover = $shell.thumbnail_hover_app_idx.take().is_some();
                    $shell.thumbnail_hover_since = None;
                    let closed_thumbnail_popup = if $shell.thumbnail_popup_open {
                        $shell.close_thumbnail_popup(crate::wayland::CommitReason::Input);
                        true
                    } else {
                        false
                    };
                    if had_panel_widget_state || had_thumbnail_hover || closed_thumbnail_popup {
                        $shell.draw_panel($qh, RepaintReason::Pointer);
                    }
                }
                SurfaceKind::Launcher => {
                    let changed = $shell.ui_preview_widget_state.take().is_some()
                        | $shell.hovered_app_card_idx.take().is_some()
                        | $shell.hovered_bento_idx.take().is_some()
                        | std::mem::replace(&mut $shell.settings_hovered, false)
                        | $shell.hovered_power_btn.take().is_some();
                    if changed {
                        $shell.draw_launcher($qh, RepaintReason::Pointer)
                    }
                }
                SurfaceKind::WorkspacePopup => {
                    if $shell.workspace_hover_idx.take().is_some() {
                        $shell.draw_workspace_popup($qh, RepaintReason::Pointer)
                    }
                }
                SurfaceKind::NetworkPopup => {}
                SurfaceKind::DesktopMenu => {
                    let hover_changed = $shell
                        .desktop_context_menu
                        .as_mut()
                        .and_then(|menu| menu.hover_idx.take())
                        .is_some();
                    if hover_changed {
                        $shell.draw_desktop_menu($qh, RepaintReason::Pointer);
                    }
                }
                SurfaceKind::Desktop
                | SurfaceKind::ThumbnailPopup
                | SurfaceKind::Calendar
                | SurfaceKind::None => {}
            }
            $shell.pointer_surface = SurfaceKind::None;
            continue;
        }

        if $shell.pointer_surface == SurfaceKind::DesktopMenu {
            if let PointerEventKind::Motion { .. } = $event.kind {
                let pad = crate::POPUP_SHADOW_PAD as f64;
                let (px, py) = ($event.position.0 - pad, $event.position.1 - pad);
                // Wider hover region (main menu → gap → flyout) keeps
                // submenu_open sticky across the visual gap. Narrow
                // region is still used for the actual flyout click hit.
                let in_hover_region = context_menu::is_in_submenu_hover_region(px);
                let in_submenu = context_menu::is_in_submenu_area(px);
                let new_hover = if in_hover_region {
                    Some(context_menu::SETTINGS_ITEM_IDX)
                } else {
                    context_menu::desktop_hit_item_local(px, py)
                };
                let new_sub_hover = if in_submenu {
                    context_menu::submenu_hit_item_local(px, py)
                } else {
                    None
                };
                let want_submenu = new_hover == Some(context_menu::SETTINGS_ITEM_IDX);

                let (changed, submenu_toggled) =
                    if let Some(ref mut menu) = $shell.desktop_context_menu {
                        let mut ch = false;
                        let toggled = want_submenu != menu.submenu_open;
                        if new_hover != menu.hover_idx {
                            menu.hover_idx = new_hover;
                            ch = true;
                        }
                        if new_sub_hover != menu.submenu_hover_idx {
                            menu.submenu_hover_idx = new_sub_hover;
                            ch = true;
                        }
                        if toggled {
                            menu.submenu_open = want_submenu;
                            ch = true;
                        }
                        (ch, toggled)
                    } else {
                        (false, false)
                    };

                if submenu_toggled {
                    let submenu_open = $shell
                        .desktop_context_menu
                        .as_ref()
                        .is_some_and(|m| m.submenu_open);
                    $shell.resize_desktop_menu_surface(submenu_open);
                }
                if changed {
                    $shell.draw_desktop_menu($qh, RepaintReason::Pointer);
                }
                continue;
            }

            if let PointerEventKind::Press { button: 0x110, .. } = $event.kind {
                let pad = crate::POPUP_SHADOW_PAD as f64;
                let (px, py) = ($event.position.0 - pad, $event.position.1 - pad);
                let sub_action = context_menu::submenu_hit_item_local(px, py)
                    .and_then(|idx| context_menu::submenu_items().get(idx).map(|item| item.1));
                let main_action = if sub_action.is_none() {
                    context_menu::desktop_hit_item_local(px, py).and_then(|idx| {
                        context_menu::desktop_item_list()
                            .get(idx)
                            .map(|item| item.1)
                    })
                } else {
                    None
                };
                $shell.desktop_context_menu = None;
                $shell.desktop_menu_open = false;
                $shell.unmap_desktop_menu(crate::wayland::CommitReason::Input);
                if let Some(sub) = sub_action {
                    $shell.handle_settings_sub_action($qh, sub);
                } else if let Some(action) = main_action {
                    $shell.handle_desktop_context_menu_action($qh, action);
                }
                continue;
            }

            if let PointerEventKind::Press { button: 0x111, .. } = $event.kind {
                $shell.desktop_context_menu = None;
                $shell.desktop_menu_open = false;
                $shell.unmap_desktop_menu(crate::wayland::CommitReason::Input);
                continue;
            }
        }
    }};
}
