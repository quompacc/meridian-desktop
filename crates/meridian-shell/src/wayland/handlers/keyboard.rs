use smithay_client_toolkit::{
    seat::keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
    shell::WaylandSurface,
};
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
        } else if self.panel.wl_surface() == surface {
            SurfaceKind::Panel
        } else {
            SurfaceKind::None
        };
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
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
        if !self.launcher_state.open {
            return;
        }

        let is_escape = event.keysym == Keysym::Escape;
        let is_enter = event.keysym == Keysym::Return || event.keysym == Keysym::KP_Enter;
        let is_backspace = event.keysym == Keysym::BackSpace;
        let is_up = event.keysym == Keysym::Up;
        let is_down = event.keysym == Keysym::Down;
        let ch = event.keysym.key_char().filter(|ch| !ch.is_control());

        match self
            .launcher_state
            .handle_key(ch, is_backspace, is_enter, is_escape, is_up, is_down)
        {
            launcher::LauncherInputResult::Close => {
                self.unmap_launcher(CommitReason::Input);
                self.draw_panel(qh, RepaintReason::Keyboard);
            }
            launcher::LauncherInputResult::Launch(idx) => {
                self.launcher_state.launch_app(idx, &mut self.ipc);
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
        _modifiers: Modifiers,
        _layout: u32,
    ) {
    }
}
