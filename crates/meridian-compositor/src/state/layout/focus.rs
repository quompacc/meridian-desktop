use smithay::{
    desktop::Window, reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::SERIAL_COUNTER,
};

use super::super::MeridianState;

impl MeridianState {
    pub fn focused_window(&self) -> Option<Window> {
        let surface = self.seat.get_keyboard()?.current_focus()?;
        let idx = self.current_workspace_index();
        self.workspaces
            .space_at(idx)
            .elements()
            .find(|window| {
                window
                    .toplevel()
                    .map_or(false, |toplevel| toplevel.wl_surface() == &surface)
            })
            .cloned()
    }

    pub fn move_focused_window_to_workspace(&mut self, target: usize) {
        let keyboard = match self.seat.get_keyboard() {
            Some(keyboard) => keyboard,
            None => return,
        };
        let surface = match keyboard.current_focus() {
            Some(surface) => surface,
            None => return,
        };
        let window = self
            .workspaces
            .active_space()
            .elements()
            .find(|window| {
                window
                    .toplevel()
                    .map_or(false, |toplevel| toplevel.wl_surface() == &surface)
            })
            .cloned();
        if let Some(window) = window {
            let serial = SERIAL_COUNTER.next_serial();
            self.set_keyboard_focus_with_decorations(Option::<WlSurface>::None, serial);
            self.workspaces.move_window_to(window, target);
            self.broadcast_window_snapshot();
        }
    }
}
