macro_rules! handle_panel_and_popups_pointer {
    ($shell:ident, $qh:ident, $event:ident) => {{
        if $shell.pointer_surface == SurfaceKind::Panel {
            if let Some(ev) = translate_pointer_event(&$event.kind, $event.position) {
                let tree = crate::panel_view::build_panel_widget_tree(
                    $shell.width,
                    &$shell.pinned_apps,
                    &$shell.panel_window_entries($shell.panel_active_workspace()),
                    $shell.network_controller.state(),
                    &$shell.audio_snapshot,
                    &$shell.status_notifier_items,
                    $shell.network_popup_open,
                    $shell.audio_popup_open,
                    &$shell.battery_snapshot,
                    $shell.power_profile,
                    $shell.panel_active_workspace(),
                    9,
                    &$shell.last_clock,
                    &$shell.icon_cache,
                    None, // screenshot_icon — nur für Hover-Layout, Icon irrelevant
                    &crate::ui::tokens::theme_from_config(&$shell.theme),
                );
                let pixel_size = meridian_ui::PixelSize {
                    width: $shell.width,
                    height: crate::PANEL_SURFACE_HEIGHT,
                };
                if let Ok(layout) = meridian_ui::compute_layout(&*tree, pixel_size) {
                    let pos = meridian_ui::PointerPosition {
                        x: $event.position.0 as i32,
                        y: $event.position.1 as i32,
                    };
                    let path = meridian_ui::hit_test(&layout, pos);
                    let new_state = super::pointer_state::apply_pointer_event(
                        $shell.panel_widget_state.clone(),
                        &ev,
                        path,
                    );
                    if new_state != $shell.panel_widget_state {
                        $shell.panel_widget_state = new_state;
                        $shell.draw_panel($qh, RepaintReason::Pointer);
                    }
                }
            }
            // No continue — Press events must fall through to the ClickZone handler below.
        }

        // Thumbnail hover detection on panel
        if $shell.pointer_surface == SurfaceKind::Panel {
            if let PointerEventKind::Motion { .. } = $event.kind {
                let hovered_pinned_idx = $shell
                    .panel_state
                    .clicks
                    .iter()
                    .find(|z| {
                        z.rect.contains($event.position.0, $event.position.1)
                            && matches!(z.action, crate::wayland::ClickAction::LaunchPinnedApp(_))
                    })
                    .and_then(|z| {
                        if let crate::wayland::ClickAction::LaunchPinnedApp(idx) = z.action {
                            Some(idx)
                        } else {
                            None
                        }
                    });

                let ws = $shell.panel_active_workspace();
                let has_windows = hovered_pinned_idx
                    .and_then(|idx| $shell.pinned_apps.get(idx))
                    .map(|app| {
                        crate::wayland::state::pinned_app_has_windows_on_workspace(
                            app,
                            &$shell.windows,
                            ws,
                        )
                    })
                    .unwrap_or(false);

                let new_hover = if has_windows {
                    hovered_pinned_idx
                } else {
                    None
                };

                if new_hover != $shell.thumbnail_hover_app_idx {
                    $shell.thumbnail_hover_app_idx = new_hover;
                    $shell.thumbnail_hover_since = new_hover.map(|_| std::time::Instant::now());
                    if new_hover.is_none() && $shell.thumbnail_popup_open {
                        $shell.close_thumbnail_popup(crate::wayland::CommitReason::Input);
                    }
                    // Prefetch thumbnails as soon as hover begins so they
                    // are cached by the time the 400ms popup-open delay
                    // elapses. Without prefetch the popup briefly opens at
                    // max placeholder width and visibly snaps smaller.
                    if let Some(idx) = new_hover {
                        if let Some(app) = $shell.pinned_apps.get(idx).cloned() {
                            let window_ids = crate::wayland::state::pinned_app_window_ids(
                                &app,
                                &$shell.windows,
                                ws,
                            );
                            for id in window_ids.iter().take(crate::THUMBNAIL_MAX_WINDOWS) {
                                let cmd = meridian_ipc::ShellCommand::CaptureWindowThumbnail {
                                    id: id.clone(),
                                    max_width: crate::THUMBNAIL_THUMB_W,
                                    max_height: crate::THUMBNAIL_THUMB_H,
                                };
                                let _ = $shell.ipc.send(&cmd);
                            }
                        }
                    }
                }

                // Popup is opened from tick() after THUMBNAIL_HOVER_DELAY_MS
            }
        }

        if $shell.workspace_popup_open
            && $shell.pointer_surface == SurfaceKind::WorkspacePopup
            && matches!($event.kind, PointerEventKind::Motion { .. })
        {
            let pad = crate::POPUP_SHADOW_PAD as f64;
            let new_hover = workspaces::workspace_popup_hover_idx(
                $event.position.0 - pad,
                $event.position.1 - pad,
            );
            if new_hover != $shell.workspace_hover_idx {
                $shell.workspace_hover_idx = new_hover;
                $shell.draw_workspace_popup($qh, RepaintReason::Pointer);
            }
        }

        if let PointerEventKind::Press { button: 0x111, .. } = $event.kind {
            if $shell.pointer_surface == SurfaceKind::Panel {
                let action = $shell
                    .panel_state
                    .clicks
                    .iter()
                    .find(|zone| zone.rect.contains($event.position.0, $event.position.1))
                    .map(|zone| zone.action.clone());
                if let Some(crate::wayland::ClickAction::ActivateStatusNotifierItem(idx)) = action {
                    $shell.handle_status_notifier_context_menu(idx);
                }
            }
        }

        // Volume OSD: its slider is draggable. Press jumps to the position and
        // starts a drag; motion tracks the pointer; release ends it. Every
        // change resets the auto-hide timer. These events are fully handled
        // here and `continue`d so the popup-close machinery below never sees
        // them — the OSD stays put while you drag it.
        if $shell.volume_osd_open && $shell.pointer_surface == SurfaceKind::NetworkPopup {
            let pad = crate::POPUP_SHADOW_PAD as f64;
            let px = $event.position.0 - pad;
            match $event.kind {
                PointerEventKind::Press { button: 0x110, .. } => {
                    if let Some(percent) = audio_popup::volume_from_x(px) {
                        $shell.audio_volume_dragging = true;
                        $shell.apply_osd_volume($qh, percent);
                    }
                    continue;
                }
                PointerEventKind::Motion { .. } if $shell.audio_volume_dragging => {
                    if let Some(percent) = audio_popup::volume_from_x(px) {
                        let current = $shell
                            .audio_snapshot
                            .default_output
                            .as_ref()
                            .and_then(|device| device.volume_percent);
                        if current != Some(percent) {
                            $shell.apply_osd_volume($qh, percent);
                        }
                    }
                    continue;
                }
                PointerEventKind::Release { button: 0x110, .. } => {
                    $shell.audio_volume_dragging = false;
                    continue;
                }
                _ => {}
            }
        }

        // Quick Settings volume: preview every motion locally and invoke the
        // platform mixer only once on release. Spawning mixerctl for each
        // pointer event makes dragging visibly stall on OpenBSD.
        if $shell.network_popup_open && $shell.pointer_surface == SurfaceKind::NetworkPopup {
            let pad = crate::POPUP_SHADOW_PAD as f64;
            let pad2 = 2 * crate::POPUP_SHADOW_PAD as u32;
            let px = $event.position.0 - pad;
            let py = $event.position.1 - pad;
            match $event.kind {
                PointerEventKind::Press { button: 0x110, .. }
                    if !$shell.status_notifier_menu_open && !$shell.audio_popup_open =>
                {
                    let card_w = $shell.network_width.saturating_sub(pad2);
                    let card_h = $shell.network_height.saturating_sub(pad2);
                    if let Some(crate::network_popup::NetworkPopupHit::Quick(
                        crate::quick_settings_popup::QuickSettingsHit::Volume(percent),
                    )) = crate::network_popup::popup_hit_test(card_w, card_h, px, py)
                    {
                        $shell.preview_quick_settings_volume($qh, percent);
                        continue;
                    }
                }
                PointerEventKind::Motion { .. }
                    if $shell.quick_settings_volume_pending.is_some() =>
                {
                    if let Some(percent) = crate::quick_settings_popup::volume_from_x(px) {
                        if $shell.quick_settings_volume_pending != Some(percent) {
                            $shell.preview_quick_settings_volume($qh, percent);
                        }
                    }
                    continue;
                }
                PointerEventKind::Release { button: 0x110, .. }
                    if $shell.quick_settings_volume_pending.is_some() =>
                {
                    $shell.commit_quick_settings_volume();
                    continue;
                }
                _ => {}
            }
        }

        if let PointerEventKind::Press { button: 0x112, .. } = $event.kind {
            if $shell.pointer_surface == SurfaceKind::Panel {
                let action = $shell
                    .panel_state
                    .clicks
                    .iter()
                    .find(|zone| zone.rect.contains($event.position.0, $event.position.1))
                    .map(|zone| zone.action.clone());
                if let Some(crate::wayland::ClickAction::ActivateStatusNotifierItem(idx)) = action {
                    $shell.handle_status_notifier_secondary_activate(idx);
                }
            }
        }

        if $shell.pointer_surface == SurfaceKind::WorkspacePopup
            && workspace_click_activation(&$event.kind)
        {
            let pad = crate::POPUP_SHADOW_PAD as f64;
            let px = $event.position.0 - pad;
            let py = $event.position.1 - pad;
            if let Some(action) = $shell
                .workspace_state
                .clicks
                .iter()
                .find(|zone| zone.rect.contains(px, py))
                .map(|zone| zone.action.clone())
            {
                $shell.handle_workspace_click($qh, action);
            }
            continue;
        }

        if let PointerEventKind::Press { button: 0x110, .. } = $event.kind {
            let action = match $shell.pointer_surface {
                SurfaceKind::Panel => $shell
                    .panel_state
                    .clicks
                    .iter()
                    .find(|zone| zone.rect.contains($event.position.0, $event.position.1))
                    .map(|zone| zone.action.clone()),
                SurfaceKind::Launcher => None,
                SurfaceKind::WorkspacePopup => None,
                SurfaceKind::NetworkPopup => {
                    let pad = crate::POPUP_SHADOW_PAD as f64;
                    let pad2 = 2 * crate::POPUP_SHADOW_PAD as u32;
                    let px = $event.position.0 - pad;
                    let py = $event.position.1 - pad;
                    if $shell.status_notifier_menu_open {
                        let card_h = $shell.status_notifier_menu_height.saturating_sub(pad2);
                        let hit = status_notifier_popup::hit_item(
                            &$shell.status_notifier_menu_entries,
                            card_h,
                            px,
                            py,
                        );
                        if let Some(item_id) = hit {
                            if let Some(menu_state) = $shell.status_notifier_menu.as_ref() {
                                crate::status_notifier::activate_menu_item(
                                    menu_state.service.clone(),
                                    menu_state.menu_path.clone(),
                                    item_id,
                                );
                            }
                            Some(crate::wayland::ClickAction::CloseStatusNotifierMenu)
                        } else {
                            let card_w = $shell.status_notifier_menu_width.saturating_sub(pad2);
                            let inside = popup_hit_test(card_w, card_h, px, py).is_some();
                            if inside {
                                None
                            } else {
                                Some(crate::wayland::ClickAction::CloseStatusNotifierMenu)
                            }
                        }
                    } else if $shell.audio_popup_open {
                        // The corner audio popup is display-only; the volume
                        // slider lives in the OSD. Clicking the body keeps it
                        // open; only the settings link and a click outside act.
                        let card_w = $shell.audio_width.saturating_sub(pad2);
                        let card_h = $shell.audio_height.saturating_sub(pad2);
                        match audio_popup::popup_hit_test(card_w, card_h, px, py) {
                            Some(audio_popup::AudioPopupHit::SettingsLink) => {
                                Some(crate::wayland::ClickAction::OpenSoundSettings)
                            }
                            Some(_) => None,
                            None => Some(crate::wayland::ClickAction::ToggleAudioPopup),
                        }
                    } else {
                        let card_w = $shell.network_width.saturating_sub(pad2);
                        let card_h = $shell.network_height.saturating_sub(pad2);
                        match crate::network_popup::popup_hit_test(card_w, card_h, px, py) {
                            Some(crate::network_popup::NetworkPopupHit::Quick(hit)) => {
                                use crate::quick_settings_popup::QuickSettingsHit;
                                match hit {
                                    QuickSettingsHit::Settings | QuickSettingsHit::Appearance => {
                                        $shell.settings_category =
                                            crate::settings_view::SettingsCategory::Theme;
                                        $shell.handle_panel_click(
                                            $qh,
                                            crate::wayland::ClickAction::ToggleSettings,
                                        );
                                        $shell.draw_panel($qh, RepaintReason::Pointer);
                                        if $shell.launcher_state.open {
                                            $shell.draw_launcher($qh, RepaintReason::Pointer);
                                        }
                                    }
                                    QuickSettingsHit::Network => {
                                        $shell.switch_network_tab(
                                            $qh,
                                            crate::network_popup::NetworkTab::Wifi,
                                        );
                                    }
                                    QuickSettingsHit::AudioMute => {
                                        crate::audio::toggle_default_sink_mute();
                                        $shell.audio_snapshot = crate::audio::AudioSnapshot::poll();
                                        $shell.draw_network_popup($qh, RepaintReason::Pointer);
                                        $shell.draw_panel($qh, RepaintReason::Pointer);
                                    }
                                    QuickSettingsHit::Volume(volume) => {
                                        crate::audio::set_default_sink_volume(volume);
                                        $shell.audio_snapshot = crate::audio::AudioSnapshot::poll();
                                        $shell.draw_network_popup($qh, RepaintReason::Pointer);
                                        $shell.draw_panel($qh, RepaintReason::Pointer);
                                    }
                                    QuickSettingsHit::PowerProfile => {
                                        use crate::power_profile::{self, PowerProfile};
                                        let current = power_profile::current()
                                            .unwrap_or(PowerProfile::Standard);
                                        let idx = PowerProfile::ALL
                                            .iter()
                                            .position(|profile| *profile == current)
                                            .unwrap_or(0);
                                        let next = PowerProfile::ALL
                                            [(idx + 1) % PowerProfile::ALL.len()];
                                        if power_profile::set(next) {
                                            $shell.power_profile = Some(next);
                                        }
                                        $shell.draw_network_popup($qh, RepaintReason::Pointer);
                                        $shell.draw_panel($qh, RepaintReason::Pointer);
                                    }
                                    QuickSettingsHit::Lock => {
                                        $shell.close_network_popup(
                                            crate::wayland::CommitReason::Input,
                                        );
                                        if !$shell.ipc.send(
                                            &meridian_ipc::ShellCommand::LockSession,
                                        ) {
                                            tracing::warn!(
                                                "quick settings: compositor lock IPC unavailable"
                                            );
                                        }
                                    }
                                    QuickSettingsHit::PowerOff => {
                                        $shell.dispatch_widget_action(
                                            $qh,
                                            crate::widget_action::WidgetAction::PowerOff,
                                        );
                                        if $shell.network_popup_open {
                                            $shell.draw_network_popup(
                                                $qh,
                                                RepaintReason::Pointer,
                                            );
                                        }
                                    }
                                    QuickSettingsHit::Card => {}
                                }
                                None
                            }
                            Some(crate::network_popup::NetworkPopupHit::SettingsLink) => {
                                Some(crate::wayland::ClickAction::OpenNetworkSettings)
                            }
                            // Tab switch / Wi-Fi row act in place and keep the
                            // popup open, so they run here and yield no
                            // ClickAction (which would route through the
                            // panel-click close logic).
                            Some(crate::network_popup::NetworkPopupHit::Tab(tab)) => {
                                $shell.switch_network_tab($qh, tab);
                                None
                            }
                            Some(crate::network_popup::NetworkPopupHit::WifiNetwork(idx)) => {
                                $shell.connect_wifi_from_popup($qh, idx);
                                None
                            }
                            Some(crate::network_popup::NetworkPopupHit::Card) => None,
                            None => Some(crate::wayland::ClickAction::ToggleNetworkPopup),
                        }
                    }
                }
                SurfaceKind::ThumbnailPopup => None,
                SurfaceKind::Calendar => None,
                SurfaceKind::Desktop | SurfaceKind::DesktopMenu => None,
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
            let keep_audio_popup_open =
                matches!(action, Some(crate::wayland::ClickAction::ToggleAudioPopup));
            let keep_status_notifier_menu_open = matches!(
                action,
                Some(crate::wayland::ClickAction::CloseStatusNotifierMenu)
            );
            if $shell.workspace_popup_open
                && $shell.pointer_surface != SurfaceKind::WorkspacePopup
                && !keep_workspace_popup_open
            {
                $shell.close_workspace_popup(crate::wayland::CommitReason::Input);
                $shell.draw_panel($qh, RepaintReason::Pointer);
            }
            if $shell.network_popup_open
                && $shell.pointer_surface != SurfaceKind::NetworkPopup
                && !keep_network_popup_open
            {
                $shell.close_network_popup(crate::wayland::CommitReason::Input);
                $shell.draw_panel($qh, RepaintReason::Pointer);
            }
            if $shell.audio_popup_open
                && $shell.pointer_surface != SurfaceKind::NetworkPopup
                && !keep_audio_popup_open
            {
                $shell.close_audio_popup(crate::wayland::CommitReason::Input);
                $shell.draw_panel($qh, RepaintReason::Pointer);
            }
            if $shell.status_notifier_menu_open
                && $shell.pointer_surface != SurfaceKind::NetworkPopup
                && !keep_status_notifier_menu_open
            {
                $shell.close_status_notifier_menu(crate::wayland::CommitReason::Input);
                $shell.draw_panel($qh, RepaintReason::Pointer);
            }
            // Close launcher on any click outside the launcher surface,
            // but only when the click itself is not a launcher-toggle action
            // (ToggleLauncher lets toggle_launcher() handle the close cleanly).
            let is_launcher_toggle =
                matches!(action, Some(crate::wayland::ClickAction::ToggleLauncher));
            if $shell.launcher_state.open
                && $shell.pointer_surface != SurfaceKind::Launcher
                && !is_launcher_toggle
            {
                $shell.close_launcher_after_launch($qh, RepaintReason::Pointer);
            }
            if let Some(action) = action {
                match $shell.pointer_surface {
                    SurfaceKind::Panel => $shell.handle_panel_click($qh, action),
                    SurfaceKind::Launcher => $shell.handle_launcher_click($qh, action),
                    SurfaceKind::WorkspacePopup => $shell.handle_workspace_click($qh, action),
                    SurfaceKind::NetworkPopup => $shell.handle_panel_click($qh, action),
                    SurfaceKind::ThumbnailPopup => {}
                    SurfaceKind::Calendar => {}
                    SurfaceKind::Desktop | SurfaceKind::DesktopMenu => {}
                    SurfaceKind::None => {}
                }
            }
        }
    }};
}
