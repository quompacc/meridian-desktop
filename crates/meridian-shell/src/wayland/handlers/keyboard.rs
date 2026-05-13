use smithay_client_toolkit::{
    seat::keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
    shell::WaylandSurface,
};
use tracing::debug;
use wayland_client::{
    protocol::{wl_keyboard, wl_surface},
    Connection, QueueHandle,
};

use crate::{
    launcher,
    wayland::{CommitReason, RepaintReason, SurfaceKind},
};

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

        let keycode = event.raw_code;
        let is_enter = event.keysym == Keysym::Return || event.keysym == Keysym::KP_Enter;
        let is_backspace = event.keysym == Keysym::BackSpace;
        let is_up = event.keysym == Keysym::Up || event.keysym == Keysym::KP_Up || keycode == 103;
        let is_down =
            event.keysym == Keysym::Down || event.keysym == Keysym::KP_Down || keycode == 108;
        let ch = event.keysym.key_char().filter(|ch| !ch.is_control());
        debug!(
            "launcher key event: keycode={} keysym={:?} utf8={:?} key_char={:?} up={} down={} enter={} esc={} backspace={} focus={:?}",
            keycode,
            event.keysym,
            event.utf8,
            ch,
            is_up,
            is_down,
            is_enter,
            is_escape,
            is_backspace,
            self.keyboard_focus
        );

        let result = self.launcher_state.handle_key(
            event
                .utf8
                .as_deref()
                .and_then(|text| text.chars().next())
                .or(ch),
            is_backspace,
            is_enter,
            is_escape,
            is_up,
            is_down,
        );
        debug!(
            "launcher key result: {:?} selected_index={}",
            result, self.launcher_state.selected_index
        );
        match result {
            launcher::LauncherInputResult::Close => {
                self.unmap_launcher(CommitReason::Input);
                self.draw_panel(qh, RepaintReason::Keyboard);
            }
            launcher::LauncherInputResult::Launch(idx) => {
                self.launcher_state.launch_app(idx, &mut self.ipc);
                self.close_launcher_after_launch(qh, RepaintReason::Keyboard);
            }
            launcher::LauncherInputResult::Action(action) => {
                match self.launcher_state.trigger_action(action, &mut self.ipc) {
                    launcher::LauncherActionTriggerResult::Armed => {
                        self.draw_launcher(qh, RepaintReason::Keyboard);
                    }
                    launcher::LauncherActionTriggerResult::Sent => {
                        self.close_launcher_after_launch(qh, RepaintReason::Keyboard);
                    }
                    launcher::LauncherActionTriggerResult::Failed => {
                        self.draw_launcher(qh, RepaintReason::Keyboard);
                    }
                }
            }
            launcher::LauncherInputResult::Redraw => {
                self.draw_launcher(qh, RepaintReason::Keyboard);
            }
            launcher::LauncherInputResult::None => {}
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
