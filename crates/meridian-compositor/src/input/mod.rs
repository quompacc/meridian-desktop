pub mod keyboard;
pub mod pointer;

use smithay::backend::input::{InputBackend, InputEvent};

use crate::state::MeridianState;

impl MeridianState {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        self.idle_notifier.notify_activity(&self.seat);

        match event {
            InputEvent::Keyboard { event, .. } => {
                keyboard::handle_keyboard(self, &event);
            }
            InputEvent::PointerMotion { event, .. } => {
                pointer::handle_pointer_motion_relative(self, &event);
            }
            InputEvent::PointerMotionAbsolute { event, .. } => {
                pointer::handle_pointer_motion_absolute(self, &event);
            }
            InputEvent::PointerButton { event, .. } => {
                pointer::handle_pointer_button(self, &event);
            }
            InputEvent::PointerAxis { event, .. } => {
                pointer::handle_pointer_axis(self, &event);
            }
            _ => {}
        }
    }
}
