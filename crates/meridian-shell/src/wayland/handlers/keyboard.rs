use smithay_client_toolkit::{
    seat::keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
    shell::WaylandSurface,
};
use tracing::debug;
use wayland_client::{
    protocol::{wl_keyboard, wl_surface},
    Connection, QueueHandle,
};

use crate::wayland::{CommitReason, RepaintReason, SurfaceKind};

use super::MeridianShell;

impl KeyboardHandler for MeridianShell {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        self.keyboard_focus = if self.launcher_layer.wl_surface() == surface {
            SurfaceKind::Launcher
        } else if self.calendar_layer.wl_surface() == surface {
            SurfaceKind::Calendar
        } else if self.workspace_layer.wl_surface() == surface {
            SurfaceKind::WorkspacePopup
        } else if self.network_layer.wl_surface() == surface {
            SurfaceKind::NetworkPopup
        } else if self.panel.wl_surface() == surface {
            SurfaceKind::Panel
        } else if self.desktop_menu_layer.wl_surface() == surface {
            SurfaceKind::DesktopMenu
        } else {
            SurfaceKind::None
        };
        if self.launcher_state.open {
            debug!(
                "launcher keyboard focus enter: focus={:?} open={} configured={}",
                self.keyboard_focus, self.launcher_state.open, self.launcher_configured
            );
        }
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        if self.launcher_state.open {
            debug!(
                "launcher keyboard focus leave: previous={:?} open={}",
                self.keyboard_focus, self.launcher_state.open
            );
        }
        self.keyboard_focus = SurfaceKind::None;
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        let is_escape = event.keysym == Keysym::Escape;

        // ── Popup / context-menu dismissals ──────────────────────────────────
        if is_escape && self.context_menu.is_some() {
            self.context_menu = None;
            self.draw_launcher(qh, RepaintReason::Keyboard);
            return;
        }

        // ── Desktop context menu: full keyboard navigation ────────────────────
        if self.desktop_menu_open {
            let is_down = event.keysym == Keysym::Down;
            let is_up = event.keysym == Keysym::Up;
            let is_right = event.keysym == Keysym::Right;
            let is_left = event.keysym == Keysym::Left;
            let is_enter = event.keysym == Keysym::Return || event.keysym == Keysym::KP_Enter;

            if is_escape {
                if self
                    .desktop_context_menu
                    .as_ref()
                    .is_some_and(|m| m.submenu_open)
                {
                    // First Esc: close submenu only
                    if let Some(ref mut menu) = self.desktop_context_menu {
                        menu.submenu_open = false;
                        menu.submenu_hover_idx = None;
                    }
                    self.resize_desktop_menu_surface(false);
                    self.draw_desktop_menu(qh, RepaintReason::Keyboard);
                } else {
                    // Second Esc (or no submenu): close whole menu
                    self.desktop_context_menu = None;
                    self.desktop_menu_open = false;
                    self.unmap_desktop_menu(CommitReason::Input);
                }
                return;
            }

            let n_main = crate::context_menu::desktop_item_list().len();
            let n_sub = crate::context_menu::submenu_items().len();
            let submenu_open = self
                .desktop_context_menu
                .as_ref()
                .is_some_and(|m| m.submenu_open);

            if is_down || is_up {
                if submenu_open {
                    if let Some(ref mut menu) = self.desktop_context_menu {
                        let cur = menu.submenu_hover_idx.unwrap_or(if is_down {
                            n_sub
                        } else {
                            n_sub + 1
                        });
                        menu.submenu_hover_idx = Some(if is_down {
                            if cur + 1 < n_sub {
                                cur + 1
                            } else {
                                0
                            }
                        } else {
                            if cur > 0 {
                                cur - 1
                            } else {
                                n_sub - 1
                            }
                        });
                    }
                } else {
                    if let Some(ref mut menu) = self.desktop_context_menu {
                        let cur =
                            menu.hover_idx
                                .unwrap_or(if is_down { n_main } else { n_main + 1 });
                        let next = if is_down {
                            if cur + 1 < n_main {
                                cur + 1
                            } else {
                                0
                            }
                        } else {
                            if cur > 0 {
                                cur - 1
                            } else {
                                n_main - 1
                            }
                        };
                        menu.hover_idx = Some(next);
                        // Auto-open submenu when Settings item is reached
                        if next == crate::context_menu::SETTINGS_ITEM_IDX {
                            menu.submenu_open = true;
                            menu.submenu_hover_idx = Some(0);
                        } else {
                            menu.submenu_open = false;
                            menu.submenu_hover_idx = None;
                        }
                    }
                    let new_submenu = self
                        .desktop_context_menu
                        .as_ref()
                        .is_some_and(|m| m.submenu_open);
                    if new_submenu != submenu_open {
                        self.resize_desktop_menu_surface(new_submenu);
                    }
                }
                self.draw_desktop_menu(qh, RepaintReason::Keyboard);
                return;
            }

            if is_right && !submenu_open {
                if self
                    .desktop_context_menu
                    .as_ref()
                    .is_some_and(|m| m.hover_idx == Some(crate::context_menu::SETTINGS_ITEM_IDX))
                {
                    if let Some(ref mut menu) = self.desktop_context_menu {
                        menu.submenu_open = true;
                        menu.submenu_hover_idx = Some(0);
                    }
                    self.resize_desktop_menu_surface(true);
                    self.draw_desktop_menu(qh, RepaintReason::Keyboard);
                }
                return;
            }

            if is_left && submenu_open {
                if let Some(ref mut menu) = self.desktop_context_menu {
                    menu.submenu_open = false;
                    menu.submenu_hover_idx = None;
                }
                self.resize_desktop_menu_surface(false);
                self.draw_desktop_menu(qh, RepaintReason::Keyboard);
                return;
            }

            if is_enter {
                if submenu_open {
                    let sub_action = self
                        .desktop_context_menu
                        .as_ref()
                        .and_then(|m| m.submenu_hover_idx)
                        .and_then(|idx| crate::context_menu::submenu_items().get(idx).map(|i| i.1));
                    self.desktop_context_menu = None;
                    self.desktop_menu_open = false;
                    self.unmap_desktop_menu(CommitReason::Input);
                    if let Some(sub) = sub_action {
                        self.handle_settings_sub_action(qh, sub);
                    }
                } else {
                    let action = self
                        .desktop_context_menu
                        .as_ref()
                        .and_then(|m| m.hover_idx)
                        .and_then(|idx| {
                            crate::context_menu::desktop_item_list()
                                .get(idx)
                                .map(|i| i.1)
                        });
                    self.desktop_context_menu = None;
                    self.desktop_menu_open = false;
                    self.unmap_desktop_menu(CommitReason::Input);
                    if let Some(action) = action {
                        self.handle_desktop_context_menu_action(qh, action);
                    }
                }
                return;
            }

            // Any other key closes the menu
            if event.keysym.key_char().is_none() {
                return;
            }
            return;
        }
        if self.network_popup_open && is_escape {
            self.close_network_popup(CommitReason::Input);
            self.draw_panel(qh, RepaintReason::Keyboard);
            return;
        }
        if self.audio_popup_open && is_escape {
            self.close_audio_popup(CommitReason::Input);
            self.draw_panel(qh, RepaintReason::Keyboard);
            return;
        }
        if self.workspace_popup_open && is_escape {
            self.close_workspace_popup(CommitReason::Input);
            self.draw_panel(qh, RepaintReason::Keyboard);
            return;
        }
        if self.calendar_popup_open && is_escape {
            self.close_calendar_popup(CommitReason::Input);
            self.draw_panel(qh, RepaintReason::Keyboard);
            if !self.launcher_state.open {
                return;
            }
        }

        if !self.launcher_state.open {
            return;
        }

        // ── Settings view: type-to-search; Escape clears then exits ───────────
        if self.launcher_settings_open {
            if is_escape {
                if !self.settings_search.is_empty() {
                    self.settings_search.clear();
                } else {
                    self.launcher_settings_open = false;
                    self.ui_preview_widget_state = None;
                }
                self.draw_launcher(qh, RepaintReason::Keyboard);
                return;
            }
            if event.keysym == Keysym::BackSpace {
                self.settings_search.pop();
                self.draw_launcher(qh, RepaintReason::Keyboard);
                return;
            }
            if let Some(ch) = event.keysym.key_char().filter(|c| !c.is_control()) {
                self.settings_search.push(ch);
                self.draw_launcher(qh, RepaintReason::Keyboard);
            }
            return;
        }

        // ── Command palette keyboard input ────────────────────────────────────
        let is_backspace = event.keysym == Keysym::BackSpace;
        let is_down = event.keysym == Keysym::Down;
        let is_up = event.keysym == Keysym::Up;
        let is_enter = event.keysym == Keysym::Return || event.keysym == Keysym::KP_Enter;

        if is_escape {
            if !self.search_query.is_empty() {
                self.search_query.clear();
                self.launcher_selected_idx = None;
                self.app_view_scroll_y = 0;
                self.draw_launcher(qh, RepaintReason::Keyboard);
            } else {
                self.close_launcher_after_launch(qh, RepaintReason::Keyboard);
            }
            return;
        }

        if is_backspace {
            self.search_query.pop();
            self.launcher_selected_idx = None;
            self.app_view_scroll_y = 0;
            self.draw_launcher(qh, RepaintReason::Keyboard);
            return;
        }

        if is_down || is_up {
            let filtered = crate::app_view::collect_palette_apps(
                &self.launcher_state.apps,
                &self.search_query,
                &self.icon_cache,
                &self.hidden_execs,
            );
            let n = filtered.len();
            if n > 0 {
                self.launcher_selected_idx = Some(match self.launcher_selected_idx {
                    None => {
                        if is_down {
                            0
                        } else {
                            n - 1
                        }
                    }
                    Some(i) => {
                        if is_down {
                            (i + 1).min(n - 1)
                        } else {
                            i.saturating_sub(1)
                        }
                    }
                });
            }
            self.draw_launcher(qh, RepaintReason::Keyboard);
            return;
        }

        if is_enter {
            let filtered = crate::app_view::collect_palette_apps(
                &self.launcher_state.apps,
                &self.search_query,
                &self.icon_cache,
                &self.hidden_execs,
            );
            let idx = self.launcher_selected_idx.unwrap_or(0);
            if let Some(app) = filtered.get(idx) {
                let app = (*app).clone();
                crate::launcher::LauncherState::launch_desktop_app(app, &mut self.ipc);
                self.close_launcher_after_launch(qh, RepaintReason::Keyboard);
            }
            return;
        }

        let ch = event.keysym.key_char().filter(|c| !c.is_control());
        if let Some(c) = ch {
            self.search_query.push(c);
            self.launcher_selected_idx = None;
            self.app_view_scroll_y = 0;
            self.draw_launcher(qh, RepaintReason::Keyboard);
        }
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _: Modifiers,
        _layout: u32,
    ) {
    }
}
