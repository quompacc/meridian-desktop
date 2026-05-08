use meridian_config::{Action, Modifiers, SplitDir};
use smithay::{
    backend::input::{Event, InputBackend, KeyboardKeyEvent, KeyState},
    input::keyboard::FilterResult,
    utils::SERIAL_COUNTER,
};

use crate::state::MeridianState;

struct KeyMatch {
    modifiers: Modifiers,
    keysym: u32,
}

pub fn handle_keyboard<I: InputBackend>(
    state: &mut MeridianState,
    event: &impl KeyboardKeyEvent<I>,
) {
    let serial = SERIAL_COUNTER.next_serial();
    let time = Event::time_msec(event);
    let key_state = event.state();
    let keyboard = state.seat.get_keyboard().unwrap();

    let match_result = keyboard.input::<KeyMatch, _>(
        state,
        event.key_code(),
        key_state,
        serial,
        time,
        |_data, modifiers, handle| {
            if key_state != KeyState::Pressed {
                return FilterResult::Forward;
            }

            let mut mods = Modifiers::empty();
            if modifiers.logo {
                mods |= Modifiers::SUPER;
            }
            if modifiers.shift {
                mods |= Modifiers::SHIFT;
            }
            if modifiers.ctrl {
                mods |= Modifiers::CTRL;
            }
            if modifiers.alt {
                mods |= Modifiers::ALT;
            }

            if let Some(&sym) = handle.raw_syms().iter().next() {
                return FilterResult::Intercept(KeyMatch {
                    modifiers: mods,
                    keysym: sym.raw(),
                });
            }

            FilterResult::Forward
        },
    );

    let Some(km) = match_result else { return };

    let action = match state.keybind_config.find_action(km.modifiers, km.keysym) {
        Some(a) => a.clone(),
        None => return,
    };

    match action {
        Action::SwitchWorkspace(idx) => state.switch_workspace(idx),
        Action::MoveToWorkspace(idx) => state.move_focused_window_to_workspace(idx),
        Action::ToggleTiling => state.toggle_tiling(),
        Action::ForceSplit(dir) => {
            let active = state.workspaces.active;
            state.wm_workspaces[active].force_split(match dir {
                SplitDir::Horizontal => meridian_wm::SplitDir::Horizontal,
                SplitDir::Vertical => meridian_wm::SplitDir::Vertical,
            });
        }
        Action::ResizeTile { dir, delta } => {
            if let Some(window) = state.focused_window() {
                let active = state.workspaces.active;
                let wm_dir = match dir {
                    SplitDir::Horizontal => meridian_wm::SplitDir::Horizontal,
                    SplitDir::Vertical => meridian_wm::SplitDir::Vertical,
                };
                state.wm_workspaces[active].resize_focused(&window, wm_dir, delta);
                state.tile_workspace(active);
            }
        }
        Action::CloseWindow => {
            if let Some(window) = state.focused_window() {
                if let Some(toplevel) = window.toplevel() {
                    toplevel.send_close();
                }
            }
        }
        Action::ToggleLauncher => {
            state.broadcast_toggle_launcher();
        }
        Action::Quit => {
            state.loop_signal.stop();
        }
    }
}
