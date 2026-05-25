use smithay::{
    backend::input::{ButtonState, InputBackend, PointerButtonEvent},
    desktop::{layer_map_for_output, WindowSurfaceType},
    input::pointer::{ButtonEvent, Focus, MotionEvent},
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
    utils::{Rectangle, SERIAL_COUNTER},
    wayland::{compositor::get_parent, seat::WaylandFocus},
};
use tracing::{debug, error, warn};

use crate::{
    decoration::{DecorationHit, DecorationResizeEdge},
    grabs::{
        move_grab::MoveSurfaceGrab,
        resize_grab::{ResizeEdge, ResizeSurfaceGrab},
    },
    protocols::xwayland::{
        apply_x11_maximize, apply_x11_unmaximize, clear_managed_xwayland_maximized_state,
        x11_window_key,
    },
    state::OutputInfo,
    state::{
        clear_tiled_toplevel_states, maximized_client_loc_from_output,
        normal_window_workarea_from_rect, remember_maximize_restore_geometry,
        resolve_unmaximize_restore_client_loc, take_maximize_restore_geometry, window_id,
        MaximizeRestoreGeometry, MeridianState, MinimizedWindowEntry, XwaylandOrDiagPointerEvent,
    },
};

fn select_pointer_button_output_info(
    infos: &[OutputInfo],
    point: Option<Point<f64, Logical>>,
) -> (Option<&OutputInfo>, &'static str) {
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

fn surface_belongs_to_layer(state: &MeridianState, surface: &WlSurface) -> bool {
    let mut current = Some(surface.clone());
    while let Some(candidate) = current {
        if state.outputs.iter().any(|output| {
            let map = layer_map_for_output(output);
            map.layer_for_surface(&candidate, WindowSurfaceType::ALL)
                .is_some()
        }) {
            return true;
        }
        current = get_parent(&candidate);
    }
    false
}

fn xwayland_override_redirect_window_under_pointer(
    state: &MeridianState,
    location: Point<f64, Logical>,
    under: &(WlSurface, Point<f64, Logical>),
) -> Option<smithay::desktop::Window> {
    let (surface, _) = under;
    let space = state.workspaces.active_space();
    space
        .element_under(location)
        .and_then(|(window, window_location)| {
            let local = location - window_location.to_f64();
            let window_surface = window
                .surface_under(local, smithay::desktop::WindowSurfaceType::ALL)?
                .0;
            (window_surface == *surface).then(|| window.clone())
        })
        .and_then(|window| match window.x11_surface() {
            Some(x11) if x11.is_override_redirect() => Some(window),
            _ => None,
        })
}

fn started_move_grab_window_states(window: &smithay::desktop::Window) -> (bool, bool) {
    if let Some(toplevel) = window.toplevel() {
        let maximized = toplevel.with_committed_state(|s| {
            s.is_some_and(|ts| ts.states.contains(xdg_toplevel::State::Maximized))
        }) || toplevel
            .with_pending_state(|s| s.states.contains(xdg_toplevel::State::Maximized));
        let fullscreen = toplevel.with_committed_state(|s| {
            s.is_some_and(|ts| ts.states.contains(xdg_toplevel::State::Fullscreen))
        }) || toplevel
            .with_pending_state(|s| s.states.contains(xdg_toplevel::State::Fullscreen));
        (maximized, fullscreen)
    } else {
        (false, false)
    }
}

fn decoration_resize_edge_to_resize_edge(edge: DecorationResizeEdge) -> ResizeEdge {
    match edge {
        DecorationResizeEdge::Top => ResizeEdge::TOP,
        DecorationResizeEdge::Left => ResizeEdge::LEFT,
        DecorationResizeEdge::Right => ResizeEdge::RIGHT,
        DecorationResizeEdge::Bottom => ResizeEdge::BOTTOM,
        DecorationResizeEdge::TopLeft => ResizeEdge::TOP_LEFT,
        DecorationResizeEdge::TopRight => ResizeEdge::TOP_RIGHT,
        DecorationResizeEdge::BottomLeft => ResizeEdge::BOTTOM_LEFT,
        DecorationResizeEdge::BottomRight => ResizeEdge::BOTTOM_RIGHT,
    }
}

fn raise_window_and_focus(
    state: &mut MeridianState,
    window: &smithay::desktop::Window,
    serial: smithay::utils::Serial,
) {
    state
        .workspaces
        .active_space_mut()
        .raise_element(window, true);
    if let Some(surface) = window.wl_surface() {
        let surface = surface.into_owned();
        state.set_keyboard_focus_with_decorations(Some(surface.clone()), serial);
        state.broadcast_toplevel_focused(&surface);
    }
}

pub fn handle_pointer_button<I: InputBackend>(
    state: &mut MeridianState,
    event: &impl PointerButtonEvent<I>,
) {
    let Some(pointer) = state.seat.get_pointer() else {
        debug!("pointer button ignored: seat has no pointer");
        return;
    };
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
        let under_is_layer_surface = under
            .as_ref()
            .map(|(surface, _)| surface_belongs_to_layer(state, surface))
            .unwrap_or(false);

        type HitInfo = (
            smithay::desktop::Window,
            DecorationHit,
            smithay::utils::Point<i32, smithay::utils::Logical>,
            Option<smithay::utils::Rectangle<i32, smithay::utils::Logical>>,
        );
        let hit_info: Option<HitInfo> = if under_is_layer_surface {
            None
        } else {
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
                mapped
                    .and_then(|output| space.output_geometry(output))
                    .map(normal_window_workarea_from_rect)
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
                    } else if let Some(x11) = window.x11_surface() {
                        if let Err(err) = x11.close() {
                            error!("x11 close failed: {}", err);
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
                    return;
                }
                DecorationHit::MaximizeButton => {
                    if let Some(x11) = window.x11_surface() {
                        if x11.is_maximized() {
                            apply_x11_unmaximize(state, x11);
                        } else {
                            apply_x11_maximize(state, x11);
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

                    if let Some(toplevel) = window.toplevel() {
                        let is_maxed = toplevel.with_committed_state(|s| {
                            s.is_some_and(|ts| {
                                ts.states.contains(
                                    smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized,
                                )
                            })
                        }) || toplevel.with_pending_state(|s| {
                            s.states.contains(
                                smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized,
                            )
                        });
                        if is_maxed {
                            toplevel.with_pending_state(|s| {
                                s.states.unset(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized);
                                s.size = None;
                            });
                            state
                                .decoration_manager
                                .set_maximized(toplevel.wl_surface(), false);
                            let restore_geometry = take_maximize_restore_geometry(
                                &mut state.maximize_restore_locations,
                                toplevel.wl_surface(),
                            );
                            let (restore_loc, used_fallback) = if restore_geometry.is_some() {
                                resolve_unmaximize_restore_client_loc(restore_geometry, (0, 0))
                            } else {
                                let theme = &state.theme_manager.current().config.decorations;
                                let (x_off, y_off) = state
                                    .decoration_manager
                                    .decoration_offset(toplevel.wl_surface(), theme);
                                resolve_unmaximize_restore_client_loc(None, (x_off, y_off))
                            };
                            if used_fallback {
                                warn!(
                                    x = restore_loc.x,
                                    y = restore_loc.y,
                                    "unmaximize restore location missing in SSD button path; applying fallback client origin"
                                );
                            }
                            state.workspaces.active_space_mut().map_element(
                                window.clone(),
                                restore_loc,
                                true,
                            );
                        } else if let Some(geo) = output_geo {
                            toplevel.with_pending_state(|s| {
                                clear_tiled_toplevel_states(s);
                                s.states.set(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized);
                                s.size = Some(geo.size);
                            });
                            state
                                .decoration_manager
                                .set_maximized(toplevel.wl_surface(), true);
                            let theme = &state.theme_manager.current().config.decorations;
                            let (x_off, y_off) = state
                                .decoration_manager
                                .decoration_offset(toplevel.wl_surface(), theme);
                            let maximized_client_loc =
                                maximized_client_loc_from_output(geo.loc, (x_off, y_off));
                            if let Some(current_loc) =
                                state.workspaces.active_space().element_location(&window)
                            {
                                remember_maximize_restore_geometry(
                                    &mut state.maximize_restore_locations,
                                    window_id(toplevel.wl_surface()),
                                    MaximizeRestoreGeometry::new(
                                        current_loc,
                                        Some(window.geometry().size),
                                    ),
                                );
                            }
                            state.workspaces.active_space_mut().map_element(
                                window.clone(),
                                maximized_client_loc,
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
                    let window_key = window
                        .toplevel()
                        .map(|toplevel| window_id(toplevel.wl_surface()))
                        .or_else(|| window.x11_surface().map(x11_window_key));
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

                    let Some(window_key) = window_key else {
                        return;
                    };
                    let workspace = state.workspaces.active;
                    let restore_loc = state
                        .workspaces
                        .space_at(workspace)
                        .element_location(&window)
                        .unwrap_or_default();
                    state.minimized_windows.insert(
                        window_key.clone(),
                        MinimizedWindowEntry {
                            window: window.clone(),
                            workspace,
                            restore_loc,
                        },
                    );
                    state.workspaces.space_at_mut(workspace).unmap_elem(&window);

                    let window_surface = window.wl_surface().map(|surface| surface.into_owned());
                    let was_focused = window_surface.as_ref().is_some_and(|window_surface| {
                        state
                            .seat
                            .get_keyboard()
                            .and_then(|keyboard| keyboard.current_focus())
                            .as_ref()
                            == Some(window_surface)
                    });
                    if was_focused {
                        let fallback_surface = state
                            .workspaces
                            .space_at(workspace)
                            .elements()
                            .filter_map(|candidate| {
                                candidate.wl_surface().map(|surface| surface.into_owned())
                            })
                            .next_back();
                        if let Some(surface) = fallback_surface {
                            state
                                .set_keyboard_focus_with_decorations(Some(surface.clone()), serial);
                            state.update_focused_output_from_surface(
                                &surface,
                                "keyboard-focus-ssd-minimize-fallback",
                            );
                            state.broadcast_toplevel_focused(&surface);
                        } else {
                            state.set_keyboard_focus_with_decorations(
                                Option::<WlSurface>::None,
                                serial,
                            );
                            state.broadcast_toplevel_focus_cleared();
                        }
                    }

                    state
                        .workspaces
                        .space_at(workspace)
                        .elements()
                        .for_each(|w| {
                            if let Some(t) = w.toplevel() {
                                t.send_pending_configure();
                            }
                        });
                    state.mark_all_outputs_dirty("ssd-minimize-window");
                    state.broadcast_window_snapshot();
                    return;
                }
                DecorationHit::TitleBar => {
                    raise_window_and_focus(state, &window, serial);
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
                        let (started_maximized, started_fullscreen) =
                            started_move_grab_window_states(&window);
                        let grab = MoveSurfaceGrab {
                            start_data,
                            window: window.clone(),
                            initial_window_location,
                            latest_pointer_location: None,
                            started_maximized,
                            started_fullscreen,
                            drag_restore_done: false,
                        };
                        pointer.set_grab(state, grab, serial, Focus::Clear);
                    }
                    return;
                }
                DecorationHit::Resize(edge) => {
                    let resize_edges = decoration_resize_edge_to_resize_edge(edge);

                    raise_window_and_focus(state, &window, serial);
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
                        if let Some(toplevel) = window.toplevel() {
                            toplevel.with_pending_state(|pending| {
                                pending.states.set(xdg_toplevel::State::Resizing);
                            });
                            toplevel.send_pending_configure();

                            let grab = ResizeSurfaceGrab::start(
                                start_data,
                                window.clone(),
                                resize_edges,
                                Rectangle::new(initial_window_location, window.geometry().size),
                            );
                            pointer.set_grab(state, grab, serial, Focus::Clear);
                        } else if let Some(x11) = window.x11_surface() {
                            clear_managed_xwayland_maximized_state(state, x11);
                            let grab = ResizeSurfaceGrab::start(
                                start_data,
                                window.clone(),
                                resize_edges,
                                Rectangle::new(initial_window_location, window.geometry().size),
                            );
                            pointer.set_grab(state, grab, serial, Focus::Clear);
                        }
                    }
                    return;
                }
            }
        }

        const BTN_LEFT: u32 = 0x110;
        const BTN_RIGHT: u32 = 0x111;
        if button == BTN_RIGHT && !under_is_layer_surface && under.is_none() {
            state.broadcast_desktop_context_menu(
                location.x.round() as i32,
                location.y.round() as i32,
            );
            return;
        }

        if button == BTN_LEFT && !under_is_layer_surface {
            if let Some((window, edge, initial_window_location)) =
                super::xwayland_resize_edge_hit_for_pointer(state, location)
            {
                let resize_edges = decoration_resize_edge_to_resize_edge(edge);
                raise_window_and_focus(state, &window, serial);
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
                    if let Some(x11) = window.x11_surface() {
                        clear_managed_xwayland_maximized_state(state, x11);
                    }
                    let grab = ResizeSurfaceGrab::start(
                        start_data,
                        window.clone(),
                        resize_edges,
                        Rectangle::new(initial_window_location, window.geometry().size),
                    );
                    pointer.set_grab(state, grab, serial, Focus::Clear);
                }
                return;
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
            let focus_before = state.keyboard_focus_diag_target();
            if button == BTN_LEFT {
                if let Some(x11) = window.x11_surface() {
                    debug!(
                        event = "pointer.button.xwayland_left_press",
                        window_id = x11.window_id(),
                        override_redirect = x11.is_override_redirect(),
                        pointer_location = ?location,
                        "left click targeted xwayland window"
                    );
                }
            }
            if window
                .x11_surface()
                .is_some_and(|x11| x11.is_override_redirect())
            {
                state
                    .workspaces
                    .active_space_mut()
                    .raise_element(&window, false);
                if let Some(x11) = window.x11_surface() {
                    let focus_after = state.keyboard_focus_diag_target();
                    let focus_changed = focus_before != focus_after;
                    debug!(
                        event = "xwayland.or_diag.pointer_press",
                        phase = "press",
                        target_window_id = x11.window_id(),
                        target_kind = "or",
                        focus_before = ?focus_before,
                        focus_after = ?focus_after,
                        focus_changed,
                        focus_change_reason = "or_policy_no_keyboard_focus",
                        "xwayland.or_diag: pointer press on OR x11 window"
                    );
                    if let Some(entry) = state.xwayland_or_diag.get_mut(&x11.window_id()) {
                        entry.last_pointer_event = Some(XwaylandOrDiagPointerEvent {
                            phase: "press",
                            target_window_id: x11.window_id(),
                            target_kind: "or",
                            focus_changed,
                            focus_change_reason: "or_policy_no_keyboard_focus",
                        });
                    }
                }
            } else {
                raise_window_and_focus(state, &window, serial);
                if let Some(x11) = window.x11_surface() {
                    let focus_after = state.keyboard_focus_diag_target();
                    let focus_changed = focus_before != focus_after;
                    debug!(
                        event = "xwayland.or_diag.pointer_press",
                        phase = "press",
                        target_window_id = x11.window_id(),
                        target_kind = "managed",
                        focus_before = ?focus_before,
                        focus_after = ?focus_after,
                        focus_changed,
                        focus_change_reason = "managed_raise_and_focus_path",
                        "xwayland.or_diag: pointer press on managed x11 window"
                    );
                    if let Some(entry) = state.xwayland_or_diag.get_mut(&x11.window_id()) {
                        entry.last_pointer_event = Some(XwaylandOrDiagPointerEvent {
                            phase: "press",
                            target_window_id: x11.window_id(),
                            target_kind: "managed",
                            focus_changed,
                            focus_change_reason: "managed_raise_and_focus_path",
                        });
                    }
                }
            }
            state.workspaces.active_space().elements().for_each(|w| {
                if let Some(t) = w.toplevel() {
                    t.send_pending_configure();
                }
            });
        } else if let Some(under_surface) = under {
            if xwayland_override_redirect_window_under_pointer(state, location, &under_surface)
                .is_none()
            {
                let (surface, _) = under_surface;
                state.set_keyboard_focus_with_decorations(Some(surface.clone()), serial);
                state.broadcast_toplevel_focused(&surface);
            }
        } else {
            state.workspaces.active_space().elements().for_each(|w| {
                w.set_activated(false);
                if let Some(t) = w.toplevel() {
                    t.send_pending_configure();
                }
            });
            state.set_keyboard_focus_with_decorations(Option::<WlSurface>::None, serial);
            state.broadcast_toplevel_focus_cleared();
        }
    }

    if button_state == ButtonState::Released && !pointer.is_grabbed() {
        let location = pointer.current_location();
        if let Some(under) = state.surface_under(location) {
            if let Some(window) =
                xwayland_override_redirect_window_under_pointer(state, location, &under)
            {
                if let Some(x11) = window.x11_surface() {
                    let focus_before = state.keyboard_focus_diag_target();
                    debug!(
                        event = "pointer.button.xwayland_release_retarget",
                        window_id = x11.window_id(),
                        pointer_location = ?location,
                        "retargeting button release to mapped xwayland override-redirect surface"
                    );
                    let focus_after = state.keyboard_focus_diag_target();
                    let focus_changed = focus_before != focus_after;
                    debug!(
                        event = "xwayland.or_diag.pointer_release",
                        phase = "release",
                        target_window_id = x11.window_id(),
                        target_kind = "or",
                        focus_before = ?focus_before,
                        focus_after = ?focus_after,
                        focus_changed,
                        focus_change_reason = "release_retarget_no_direct_focus_path",
                        "xwayland.or_diag: pointer release retargeted to OR x11 window"
                    );
                    if let Some(entry) = state.xwayland_or_diag.get_mut(&x11.window_id()) {
                        entry.last_pointer_event = Some(XwaylandOrDiagPointerEvent {
                            phase: "release",
                            target_window_id: x11.window_id(),
                            target_kind: "or",
                            focus_changed,
                            focus_change_reason: "release_retarget_no_direct_focus_path",
                        });
                    }
                }
                pointer.motion(
                    state,
                    Some(under),
                    &MotionEvent {
                        location,
                        serial,
                        time: event.time_msec(),
                    },
                );
            }
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
