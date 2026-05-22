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
        } else if self.settings_layer.wl_surface() == surface {
            SurfaceKind::Settings
        } else if self.panel.wl_surface() == surface {
            SurfaceKind::Panel
        } else {
            SurfaceKind::None
        };
        if self.launcher_state.open {
            debug!(
                "launcher keyboard focus enter: focus={:?} launcher_open={} launcher_configured={}",
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
                "launcher keyboard focus leave: previous_focus={:?} launcher_open={} launcher_configured={}",
                self.keyboard_focus, self.launcher_state.open, self.launcher_configured
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
        if is_escape && self.context_menu.is_some() {
            self.context_menu = None;
            self.draw_launcher(qh, RepaintReason::Keyboard);
            return;
        }
        if self.network_popup_open && is_escape {
            self.close_network_popup(CommitReason::Input);
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

        if self.app_view_open {
            let is_backspace = event.keysym == Keysym::BackSpace;
            if is_escape && self.search_query.is_empty() {
                self.app_view_open = false;
                self.ui_preview_widget_state = None;
                self.draw_launcher(qh, RepaintReason::Keyboard);
                return;
            }
            if is_escape {
                self.search_query.clear();
                self.draw_launcher(qh, RepaintReason::Keyboard);
                return;
            }
            if is_backspace {
                self.search_query.pop();
                self.draw_launcher(qh, RepaintReason::Keyboard);
                return;
            }
            let ch = event.keysym.key_char().filter(|c| !c.is_control());
            if let Some(c) = ch {
                self.search_query.push(c);
                self.draw_launcher(qh, RepaintReason::Keyboard);
            }
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
