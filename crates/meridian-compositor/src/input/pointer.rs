use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, InputBackend, PointerAxisEvent,
        PointerButtonEvent,
    },
    input::pointer::{AxisFrame, ButtonEvent, Focus, MotionEvent},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::SERIAL_COUNTER,
    wayland::seat::WaylandFocus,
};

use crate::grabs::move_grab::MoveSurfaceGrab;

use crate::decoration::DecorationHit;
use crate::state::MeridianState;

pub fn handle_pointer_motion_absolute<I: InputBackend>(
    state: &mut MeridianState,
    event: &impl AbsolutePositionEvent<I>,
) {
    let output = match state.outputs.first().cloned() {
        Some(o) => o,
        None => return,
    };
    let output_geo = state
        .workspaces
        .active_space()
        .output_geometry(&output)
        .unwrap();
    let pos = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();
    let serial = SERIAL_COUNTER.next_serial();
    let pointer = state.seat.get_pointer().unwrap();
    let under = state.surface_under(pos);

    pointer.motion(
        state,
        under,
        &MotionEvent {
            location: pos,
            serial,
            time: event.time_msec(),
        },
    );
    pointer.frame(state);
}

pub fn handle_pointer_button<I: InputBackend>(
    state: &mut MeridianState,
    event: &impl PointerButtonEvent<I>,
) {
    let pointer = state.seat.get_pointer().unwrap();
    let keyboard = state.seat.get_keyboard().unwrap();
    let serial = SERIAL_COUNTER.next_serial();
    let button = event.button_code();
    let button_state = event.state();

    if ButtonState::Pressed == button_state && !pointer.is_grabbed() {
        let location = pointer.current_location();
        let under = state.surface_under(location);

        // Phase 1: read-only scan — collect owned hit data before any mutation
        type HitInfo = (smithay::desktop::Window, DecorationHit, smithay::utils::Point<i32, smithay::utils::Logical>, Option<smithay::utils::Rectangle<i32, smithay::utils::Logical>>);
        let hit_info: Option<HitInfo> = {
            let space = state.workspaces.active_space();
            let theme = &state.theme_manager.current().config.decorations;
            space.element_under(location).and_then(|(window, window_loc)| {
                let wl_surf = window.wl_surface()?.into_owned();
                let content_size = window.geometry().size;
                let hit = state.decoration_manager.hit_test(&wl_surf, location, window_loc, content_size, theme)?;
                let initial_loc = space.element_location(window).unwrap_or_default();
                let output_geo = state.outputs.first().and_then(|o| space.output_geometry(o));
                Some((window.clone(), hit, initial_loc, output_geo))
            })
        }; // space and theme drop here

        // Phase 2: mutation based on owned hit data
        if let Some((window, hit, initial_window_location, output_geo)) = hit_info {
            match hit {
                DecorationHit::CloseButton => {
                    if let Some(toplevel) = window.toplevel() {
                        toplevel.send_close();
                    }
                    pointer.button(state, &ButtonEvent { button, state: button_state, serial, time: event.time_msec() });
                    pointer.frame(state);
                    return;
                }
                DecorationHit::MaximizeButton => {
                    if let Some(toplevel) = window.toplevel() {
                        let is_maxed = toplevel.with_committed_state(|s| {
                            s.map_or(false, |ts| ts.states.contains(
                                smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized,
                            ))
                        });
                        if is_maxed {
                            toplevel.with_pending_state(|s| {
                                s.states.unset(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized);
                                s.size = None;
                            });
                            state.decoration_manager.set_maximized(toplevel.wl_surface(), false);
                        } else if let Some(geo) = output_geo {
                            toplevel.with_pending_state(|s| {
                                s.states.set(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized);
                                s.size = Some(geo.size);
                            });
                            state.decoration_manager.set_maximized(toplevel.wl_surface(), true);
                            state.workspaces.active_space_mut().map_element(window.clone(), geo.loc, true);
                        }
                        toplevel.send_pending_configure();
                    }
                    pointer.button(state, &ButtonEvent { button, state: button_state, serial, time: event.time_msec() });
                    pointer.frame(state);
                    return;
                }
                DecorationHit::MinimizeButton => {
                    pointer.button(state, &ButtonEvent { button, state: button_state, serial, time: event.time_msec() });
                    pointer.frame(state);
                    return;
                }
                DecorationHit::TitleBar | DecorationHit::Border => {
                    state.workspaces.active_space_mut().raise_element(&window, true);
                    if let Some(surface) = window.wl_surface() {
                        let surface = surface.into_owned();
                        let old_focus = keyboard.current_focus();
                        state.update_focus_decoration(old_focus.as_ref(), Some(&surface));
                        keyboard.set_focus(state, Some(surface.clone()), serial);
                        state.broadcast_toplevel_focused(&surface);
                    }
                    // pointer.button() triggers ClickGrab internally; we then upgrade to MoveSurfaceGrab
                    pointer.button(state, &ButtonEvent { button, state: button_state, serial, time: event.time_msec() });
                    pointer.frame(state);
                    if let Some(start_data) = pointer.grab_start_data() {
                        let grab = MoveSurfaceGrab { start_data, window: window.clone(), initial_window_location };
                        pointer.set_grab(state, grab, serial, Focus::Clear);
                    }
                    return;
                }
            }
        }

        let window_under = {
            let space = state.workspaces.active_space();
            space.element_under(location).and_then(|(window, window_location)| {
                let (surface, _) = under.as_ref()?;
                let local = location - window_location.to_f64();
                let window_surface = window
                    .surface_under(local, smithay::desktop::WindowSurfaceType::ALL)?
                    .0;
                (window_surface == *surface).then(|| window.clone())
            })
        };

        if let Some(window) = window_under {
            state
                .workspaces
                .active_space_mut()
                .raise_element(&window, true);
            if let Some(surface) = window.wl_surface() {
                let surface = surface.into_owned();
                let old_focus = keyboard.current_focus();
                state.update_focus_decoration(old_focus.as_ref(), Some(&surface));
                keyboard.set_focus(state, Some(surface.clone()), serial);
                state.broadcast_toplevel_focused(&surface);
            }
            state.workspaces.active_space().elements().for_each(|w| {
                if let Some(t) = w.toplevel() {
                    t.send_pending_configure();
                }
            });
        } else if let Some((surface, _)) = under {
            keyboard.set_focus(state, Some(surface.clone()), serial);
            state.broadcast_toplevel_focused(&surface);
        } else {
            state.workspaces.active_space().elements().for_each(|w| {
                w.set_activated(false);
                if let Some(t) = w.toplevel() {
                    t.send_pending_configure();
                }
            });
            keyboard.set_focus(state, Option::<WlSurface>::None, serial);
        }
    }

    pointer.button(
        state,
        &ButtonEvent {
            button,
            state: button_state,
            serial,
            time: event.time_msec(),
        },
    );
    pointer.frame(state);
}

pub fn handle_pointer_axis<I: InputBackend>(
    state: &mut MeridianState,
    event: &impl PointerAxisEvent<I>,
) {
    let source = event.source();

    let h = event
        .amount(Axis::Horizontal)
        .unwrap_or_else(|| event.amount_v120(Axis::Horizontal).unwrap_or(0.0) * 15.0 / 120.0);
    let v = event
        .amount(Axis::Vertical)
        .unwrap_or_else(|| event.amount_v120(Axis::Vertical).unwrap_or(0.0) * 15.0 / 120.0);
    let h120 = event.amount_v120(Axis::Horizontal);
    let v120 = event.amount_v120(Axis::Vertical);

    let mut frame = AxisFrame::new(event.time_msec()).source(source);
    if h != 0.0 {
        frame = frame.value(Axis::Horizontal, h);
        if let Some(d) = h120 {
            frame = frame.v120(Axis::Horizontal, d as i32);
        }
    }
    if v != 0.0 {
        frame = frame.value(Axis::Vertical, v);
        if let Some(d) = v120 {
            frame = frame.v120(Axis::Vertical, d as i32);
        }
    }
    if source == AxisSource::Finger {
        if event.amount(Axis::Horizontal) == Some(0.0) {
            frame = frame.stop(Axis::Horizontal);
        }
        if event.amount(Axis::Vertical) == Some(0.0) {
            frame = frame.stop(Axis::Vertical);
        }
    }

    let pointer = state.seat.get_pointer().unwrap();
    pointer.axis(state, frame);
    pointer.frame(state);
}
