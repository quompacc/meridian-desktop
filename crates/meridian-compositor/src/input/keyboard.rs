use meridian_wm::SplitDir;
use smithay::{
    backend::input::{Event, InputBackend, KeyboardKeyEvent, KeyState},
    input::keyboard::FilterResult,
    utils::SERIAL_COUNTER,
};

use crate::state::MeridianState;

enum WsAction {
    Switch(usize),
    MoveWindow(usize),
    ToggleTiling,
    ForceSplit(SplitDir),
    ResizeTile(SplitDir, f32),
}

pub fn handle_keyboard<I: InputBackend>(
    state: &mut MeridianState,
    event: &impl KeyboardKeyEvent<I>,
) {
    let serial = SERIAL_COUNTER.next_serial();
    let time = Event::time_msec(event);
    let key_state = event.state();
    let keyboard = state.seat.get_keyboard().unwrap();

    let action = keyboard.input::<WsAction, _>(
        state,
        event.key_code(),
        key_state,
        serial,
        time,
        |_data, modifiers, handle| {
            if key_state == KeyState::Pressed && modifiers.logo {
                if let Some(&sym) = handle.raw_syms().iter().next() {
                    let raw = sym.raw();
                    // Super+1…9  workspace switch / window move
                    if raw >= 0x31 && raw <= 0x39 {
                        let idx = (raw - 0x31) as usize;
                        return if modifiers.shift {
                            FilterResult::Intercept(WsAction::MoveWindow(idx))
                        } else {
                            FilterResult::Intercept(WsAction::Switch(idx))
                        };
                    }
                    // Super+T  toggle tiling mode
                    if raw == 0x74 {
                        return FilterResult::Intercept(WsAction::ToggleTiling);
                    }
                    // Super+H / Super+V  force split direction
                    if raw == 0x68 {
                        return FilterResult::Intercept(WsAction::ForceSplit(SplitDir::Horizontal));
                    }
                    if raw == 0x76 {
                        return FilterResult::Intercept(WsAction::ForceSplit(SplitDir::Vertical));
                    }
                    // Super+Arrows  resize tile
                    match raw {
                        0xff51 => return FilterResult::Intercept(WsAction::ResizeTile(SplitDir::Horizontal, -0.05)),
                        0xff53 => return FilterResult::Intercept(WsAction::ResizeTile(SplitDir::Horizontal,  0.05)),
                        0xff52 => return FilterResult::Intercept(WsAction::ResizeTile(SplitDir::Vertical,   -0.05)),
                        0xff54 => return FilterResult::Intercept(WsAction::ResizeTile(SplitDir::Vertical,    0.05)),
                        _ => {}
                    }
                }
            }
            FilterResult::Forward
        },
    );

    match action {
        Some(WsAction::Switch(idx)) => state.switch_workspace(idx),
        Some(WsAction::MoveWindow(idx)) => state.move_focused_window_to_workspace(idx),
        Some(WsAction::ToggleTiling) => state.toggle_tiling(),
        Some(WsAction::ForceSplit(dir)) => {
            let active = state.workspaces.active;
            state.wm_workspaces[active].force_split(dir);
        }
        Some(WsAction::ResizeTile(dir, delta)) => {
            if let Some(window) = state.focused_window() {
                let active = state.workspaces.active;
                state.wm_workspaces[active].resize_focused(&window, dir, delta);
                state.tile_workspace(active);
            }
        }
        None => {}
    }
}
