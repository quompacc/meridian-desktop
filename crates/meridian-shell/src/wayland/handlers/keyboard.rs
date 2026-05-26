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

        // ── Popup / context-menu dismissals (unchanged) ──────────────────────
        if is_escape && self.context_menu.is_some() {
            self.context_menu = None;
            self.draw_launcher(qh, RepaintReason::Keyboard);
            return;
        }
        if is_escape && self.desktop_menu_open {
            self.desktop_context_menu = None;
            self.desktop_menu_open = false;
            self.unmap_desktop_menu(CommitReason::Input);
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
                        if is_down { 0 } else { n - 1 }
                    }
                    Some(i) => {
                        if is_down { (i + 1).min(n - 1) } else { i.saturating_sub(1) }
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
