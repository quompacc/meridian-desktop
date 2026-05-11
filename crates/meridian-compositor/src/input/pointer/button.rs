use smithay::{
    backend::input::{ButtonState, InputBackend, PointerButtonEvent},
    input::pointer::{ButtonEvent, Focus},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::SERIAL_COUNTER,
    utils::{Logical, Point},
    wayland::seat::WaylandFocus,
};
use tracing::debug;

use crate::{
    decoration::DecorationHit, grabs::move_grab::MoveSurfaceGrab, state::MeridianState,
    state::OutputInfo,
};

fn select_pointer_button_output_info<'a>(
    infos: &'a [OutputInfo],
    point: Option<Point<f64, Logical>>,
) -> (Option<&'a OutputInfo>, &'static str) {
    if let Some(pos) = point {
        if let Some(output) = infos
            .iter()
            .find(|info| info.geometry.contains(pos.x, pos.y))
        {
            return (Some(output), "point-match");
        }
    }

    if let Some(output) = infos.iter().find(|info| info.primary) {
        return (Some(output), "fallback-primary");
    }

    if let Some(output) = infos.first() {
        return (Some(output), "fallback-first");
    }

    (None, "empty-registry")
}

pub fn handle_pointer_button<I: InputBackend>(
    state: &mut MeridianState,
    event: &impl PointerButtonEvent<I>,
) {
    let pointer = state.seat.get_pointer().unwrap();
    let serial = SERIAL_COUNTER.next_serial();
    let button = event.button_code();
    let button_state = event.state();

    if ButtonState::Pressed == button_state && !pointer.is_grabbed() {
        let location = pointer.current_location();
        if super::output_id_at_point_for_focus(&state.output_registry, location.x, location.y)
            .is_some()
        {
            state.update_focused_output_from_point(location, "pointer-button", true);
        }
        let (selected_output_info, fallback_reason) =
            select_pointer_button_output_info(state.output_registry.list(), Some(location));
        if let Some(info) = selected_output_info {
            debug!(
                "pointer button output selection requested: x={:.2} y={:.2} selected_output_id={} name={} fallback_reason={}",
                location.x, location.y, info.id.0, info.name, fallback_reason
            );
        } else {
            debug!(
                "pointer button output selection requested: x={:.2} y={:.2} selected_output=none fallback_reason={}",
                location.x, location.y, fallback_reason
            );
        }
        let under = state.surface_under(location);

        type HitInfo = (
            smithay::desktop::Window,
            DecorationHit,
            smithay::utils::Point<i32, smithay::utils::Logical>,
            Option<smithay::utils::Rectangle<i32, smithay::utils::Logical>>,
        );
        let hit_info: Option<HitInfo> = {
            let space = state.workspaces.active_space();
            let theme = &state.theme_manager.current().config.decorations;
            let output_geo = selected_output_info.and_then(|info| {
                let mapped = state
                    .outputs
                    .iter()
                    .find(|candidate| candidate.name() == info.name);
                if mapped.is_none() {
                    debug!(
                        "pointer button output selection fallback: registry output '{}' not present in active output list",
                        info.name
                    );
                }
                mapped.and_then(|output| space.output_geometry(output))
            });

            let hit_for_window =
                |window: &smithay::desktop::Window,
                 window_loc: smithay::utils::Point<i32, smithay::utils::Logical>| {
                    let wl_surf = window.wl_surface()?.into_owned();
                    let content_size = window.geometry().size;
                    let hit = state.decoration_manager.hit_test(
                        &wl_surf,
                        location,
                        window_loc,
                        content_size,
                        theme,
                    )?;
                    let initial_loc = space.element_location(window).unwrap_or_default();
                    Some((window.clone(), hit, initial_loc, output_geo))
                };

            space
                .element_under(location)
                .and_then(|(window, window_loc)| hit_for_window(window, window_loc))
                .or_else(|| {
                    // Fallback path: allow SSD frame hit-testing even when pointer is
                    // outside the client surface and element_under() returns None.
                    // Iterate in reverse mapped order to prefer topmost windows.
                    let windows: Vec<_> = space.elements().cloned().collect();
                    windows.iter().rev().find_map(|window| {
                        let window_loc = space.element_location(window)?;
                        hit_for_window(window, window_loc)
                    })
                })
        };

        if let Some((window, hit, initial_window_location, output_geo)) = hit_info {
            match hit {
                DecorationHit::CloseButton => {
                    if let Some(toplevel) = window.toplevel() {
                        toplevel.send_close();
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
                    return;
                }
                DecorationHit::MaximizeButton => {
                    if let Some(toplevel) = window.toplevel() {
                        let is_maxed = toplevel.with_committed_state(|s| {
                            s.map_or(false, |ts| {
                                ts.states.contains(
                                    smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized,
                                )
                            })
                        });
                        if is_maxed {
                            toplevel.with_pending_state(|s| {
                                s.states.unset(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized);
                                s.size = None;
                            });
                            state
                                .decoration_manager
                                .set_maximized(toplevel.wl_surface(), false);
                        } else if let Some(geo) = output_geo {
                            toplevel.with_pending_state(|s| {
                                s.states.set(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized);
                                s.size = Some(geo.size);
                            });
                            state
                                .decoration_manager
                                .set_maximized(toplevel.wl_surface(), true);
                            state.workspaces.active_space_mut().map_element(
                                window.clone(),
                                geo.loc,
                                true,
                            );
                        }
                        toplevel.send_pending_configure();
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
                    return;
                }
                DecorationHit::MinimizeButton => {
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
                    return;
                }
                DecorationHit::TitleBar | DecorationHit::Border => {
                    state
                        .workspaces
                        .active_space_mut()
                        .raise_element(&window, true);
                    if let Some(surface) = window.wl_surface() {
                        let surface = surface.into_owned();
                        state.set_keyboard_focus_with_decorations(Some(surface.clone()), serial);
                        state.broadcast_toplevel_focused(&surface);
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
                    if let Some(start_data) = pointer.grab_start_data() {
                        let grab = MoveSurfaceGrab {
                            start_data,
                            window: window.clone(),
                            initial_window_location,
                        };
                        pointer.set_grab(state, grab, serial, Focus::Clear);
                    }
                    return;
                }
            }
        }

        let window_under = {
            let space = state.workspaces.active_space();
            space
                .element_under(location)
                .and_then(|(window, window_location)| {
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
                state.set_keyboard_focus_with_decorations(Some(surface.clone()), serial);
                state.broadcast_toplevel_focused(&surface);
            }
            state.workspaces.active_space().elements().for_each(|w| {
                if let Some(t) = w.toplevel() {
                    t.send_pending_configure();
                }
            });
        } else if let Some((surface, _)) = under {
            state.set_keyboard_focus_with_decorations(Some(surface.clone()), serial);
            state.broadcast_toplevel_focused(&surface);
        } else {
            state.workspaces.active_space().elements().for_each(|w| {
                w.set_activated(false);
                if let Some(t) = w.toplevel() {
                    t.send_pending_configure();
                }
            });
            state.set_keyboard_focus_with_decorations(Option::<WlSurface>::None, serial);
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

#[cfg(test)]
mod tests {
    use smithay::utils::{Logical, Point, Transform};

    use crate::state::{OutputGeometry, OutputId, OutputInfo};

    #[test]
    fn click_point_on_output_one() {
        let infos = vec![
            OutputInfo {
                id: OutputId(1),
                name: "left".to_string(),
                geometry: OutputGeometry {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                scale: 1.0,
                transform: Transform::Normal,
                refresh_millihz: Some(60_000),
                primary: true,
            },
            OutputInfo {
                id: OutputId(2),
                name: "right".to_string(),
                geometry: OutputGeometry {
                    x: 1920,
                    y: 0,
                    width: 2560,
                    height: 1440,
                },
                scale: 1.0,
                transform: Transform::Normal,
                refresh_millihz: Some(60_000),
                primary: false,
            },
        ];
        let point: Point<f64, Logical> = (100.0, 200.0).into();
        let (selected, reason) = super::select_pointer_button_output_info(&infos, Some(point));
        assert_eq!(selected.map(|info| info.name.as_str()), Some("left"));
        assert_eq!(reason, "point-match");
    }

    #[test]
    fn click_point_on_output_two() {
        let infos = vec![
            OutputInfo {
                id: OutputId(1),
                name: "left".to_string(),
                geometry: OutputGeometry {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                scale: 1.0,
                transform: Transform::Normal,
                refresh_millihz: Some(60_000),
                primary: true,
            },
            OutputInfo {
                id: OutputId(2),
                name: "right".to_string(),
                geometry: OutputGeometry {
                    x: 1920,
                    y: 0,
                    width: 2560,
                    height: 1440,
                },
                scale: 1.0,
                transform: Transform::Normal,
                refresh_millihz: Some(60_000),
                primary: false,
            },
        ];
        let point: Point<f64, Logical> = (2200.0, 400.0).into();
        let (selected, reason) = super::select_pointer_button_output_info(&infos, Some(point));
        assert_eq!(selected.map(|info| info.name.as_str()), Some("right"));
        assert_eq!(reason, "point-match");
    }

    #[test]
    fn outside_point_uses_primary_fallback() {
        let infos = vec![OutputInfo {
            id: OutputId(11),
            name: "primary".to_string(),
            geometry: OutputGeometry {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
            primary: true,
        }];
        let point: Point<f64, Logical> = (-50.0, -50.0).into();
        let (selected, reason) = super::select_pointer_button_output_info(&infos, Some(point));
        assert_eq!(selected.map(|info| info.name.as_str()), Some("primary"));
        assert_eq!(reason, "fallback-primary");
    }

    #[test]
    fn no_primary_uses_first_fallback() {
        let infos = vec![
            OutputInfo {
                id: OutputId(21),
                name: "first".to_string(),
                geometry: OutputGeometry {
                    x: 0,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
                scale: 1.0,
                transform: Transform::Normal,
                refresh_millihz: Some(60_000),
                primary: false,
            },
            OutputInfo {
                id: OutputId(22),
                name: "second".to_string(),
                geometry: OutputGeometry {
                    x: 1280,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
                scale: 1.0,
                transform: Transform::Normal,
                refresh_millihz: Some(60_000),
                primary: false,
            },
        ];
        let point: Point<f64, Logical> = (-500.0, -10.0).into();
        let (selected, reason) = super::select_pointer_button_output_info(&infos, Some(point));
        assert_eq!(selected.map(|info| info.name.as_str()), Some("first"));
        assert_eq!(reason, "fallback-first");
    }

    #[test]
    fn empty_registry_is_safe() {
        let (selected, reason) = super::select_pointer_button_output_info(&[], None);
        assert!(selected.is_none());
        assert_eq!(reason, "empty-registry");
    }
}
