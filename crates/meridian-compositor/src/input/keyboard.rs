use meridian_config::{Action, Modifiers, SplitDir};
use smithay::{
    backend::input::{Event, InputBackend, KeyState, KeyboardKeyEvent},
    desktop::Window,
    input::keyboard::FilterResult,
    utils::SERIAL_COUNTER,
    wayland::seat::WaylandFocus,
};
use tracing::debug;

use crate::state::MeridianState;

struct KeyMatch {
    modifiers: Modifiers,
    keysym: u32,
}

fn wm_split_dir(dir: SplitDir) -> meridian_wm::SplitDir {
    match dir {
        SplitDir::Horizontal => meridian_wm::SplitDir::Horizontal,
        SplitDir::Vertical => meridian_wm::SplitDir::Vertical,
    }
}

fn focused_window_for_close(state: &MeridianState) -> Option<Window> {
    let focus_surface = state.seat.get_keyboard()?.current_focus()?;
    (0..state.workspaces.count()).find_map(|idx| {
        state
            .workspaces
            .space_at(idx)
            .elements()
            .find(|window| {
                window
                    .wl_surface()
                    .is_some_and(|surface| surface.as_ref() == &focus_surface)
            })
            .cloned()
    })
}

pub fn handle_keyboard<I: InputBackend>(
    state: &mut MeridianState,
    event: &impl KeyboardKeyEvent<I>,
) {
    let serial = SERIAL_COUNTER.next_serial();
    let time = Event::time_msec(event);
    let key_state = event.state();
    let Some(keyboard) = state.seat.get_keyboard() else {
        debug!("keyboard event ignored: seat has no keyboard");
        return;
    };

    let match_result = keyboard.input::<KeyMatch, _>(
        state,
        event.key_code(),
        key_state,
        serial,
        time,
        |data, modifiers, handle| {
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

            if let Some(&sym) = handle.raw_syms().first() {
                let keysym = sym.raw();
                let is_global_shortcut = data.keybind_config.find_action(mods, keysym).is_some()
                    || is_workspace_fallback_shortcut(mods, keysym);
                if !is_global_shortcut {
                    return FilterResult::Forward;
                }

                return FilterResult::Intercept(KeyMatch {
                    modifiers: mods,
                    keysym,
                });
            }

            FilterResult::Forward
        },
    );

    let Some(km) = match_result else { return };

    let action = match state.keybind_config.find_action(km.modifiers, km.keysym) {
        Some(a) => a.clone(),
        None => {
            // Keep Super+1..9 workspace switching available as a stable fallback,
            // even when custom keybind maps omit explicit workspace entries.
            if km.modifiers == Modifiers::SUPER {
                if let Some(idx) = workspace_idx_from_digit_keysym(km.keysym) {
                    let focused_output = state.focused_output();
                    let focused_output_name = focused_output.and_then(|id| {
                        state
                            .output_registry
                            .by_id(id)
                            .map(|info| info.name.as_str())
                    });
                    debug!(
                        "keybind switch workspace for focused output (fallback): keysym=0x{:x} target_workspace={} focused_output_id={:?} focused_output_name={:?}",
                        km.keysym,
                        idx + 1,
                        focused_output.map(|id| id.0),
                        focused_output_name
                    );
                    state.switch_workspace_for_focused_output(idx);
                }
            } else if km.modifiers == (Modifiers::SUPER | Modifiers::SHIFT) {
                if let Some(idx) = workspace_idx_from_digit_keysym(km.keysym) {
                    let focused_output = state.focused_output();
                    let focused_output_name = focused_output.and_then(|id| {
                        state
                            .output_registry
                            .by_id(id)
                            .map(|info| info.name.as_str())
                    });
                    debug!(
                        "keybind move workspace for focused output (fallback): keysym=0x{:x} target_workspace={} focused_output_id={:?} focused_output_name={:?}",
                        km.keysym,
                        idx + 1,
                        focused_output.map(|id| id.0),
                        focused_output_name
                    );
                    state.move_focused_window_to_workspace_consistent(idx);
                }
            }
            return;
        }
    };

    match action {
        Action::SwitchWorkspace(idx) => {
            let focused_output = state.focused_output();
            let focused_output_name = focused_output.and_then(|id| {
                state
                    .output_registry
                    .by_id(id)
                    .map(|info| info.name.as_str())
            });
            debug!(
                "keybind switch workspace for focused output: target_workspace={} focused_output_id={:?} focused_output_name={:?}",
                idx + 1,
                focused_output.map(|id| id.0),
                focused_output_name
            );
            state.switch_workspace_for_focused_output(idx)
        }
        Action::MoveToWorkspace(idx) => {
            let focused_output = state.focused_output();
            let focused_output_name = focused_output.and_then(|id| {
                state
                    .output_registry
                    .by_id(id)
                    .map(|info| info.name.as_str())
            });
            debug!(
                "keybind move workspace for focused output: target_workspace={} focused_output_id={:?} focused_output_name={:?}",
                idx + 1,
                focused_output.map(|id| id.0),
                focused_output_name
            );
            state.move_focused_window_to_workspace_consistent(idx)
        }
        Action::ToggleTiling => state.toggle_tiling(),
        Action::ForceSplit(dir) => {
            let active = state.workspaces.active;
            state.wm_workspaces[active].force_split(wm_split_dir(dir));
        }
        Action::ResizeTile { dir, delta } => {
            if let Some(window) = state.focused_window() {
                let active = state.workspaces.active;
                let wm_dir = wm_split_dir(dir);
                state.wm_workspaces[active].resize_focused(&window, wm_dir, delta);
                state.tile_workspace(active);
            }
        }
        Action::CloseWindow => {
            if let Some(window) = focused_window_for_close(state) {
                if let Some(toplevel) = window.toplevel() {
                    toplevel.send_close();
                } else if let Some(x11) = window.x11_surface() {
                    if let Err(err) = x11.close() {
                        debug!(
                            "xwayland close request failed for {}: {}",
                            x11.window_id(),
                            err
                        );
                    }
                }
            }
        }
        Action::ToggleLauncher => {
            state.broadcast_toggle_launcher();
        }
        Action::ReloadConfig => {
            state.reload_config();
        }
        Action::Quit => {
            state.loop_signal.stop();
        }
    }
}

fn workspace_idx_from_digit_keysym(keysym: u32) -> Option<usize> {
    match keysym {
        0x31 => Some(0),
        0x32 => Some(1),
        0x33 => Some(2),
        0x34 => Some(3),
        0x35 => Some(4),
        0x36 => Some(5),
        0x37 => Some(6),
        0x38 => Some(7),
        0x39 => Some(8),
        _ => None,
    }
}

fn is_workspace_fallback_shortcut(modifiers: Modifiers, keysym: u32) -> bool {
    workspace_idx_from_digit_keysym(keysym).is_some()
        && (modifiers == Modifiers::SUPER || modifiers == (Modifiers::SUPER | Modifiers::SHIFT))
}
