type HitInfo = (
    smithay::desktop::Window,
    DecorationHit,
    smithay::utils::Point<i32, smithay::utils::Logical>,
    Option<smithay::utils::Rectangle<i32, smithay::utils::Logical>>,
);

fn decoration_hit_info(
    state: &MeridianState,
    location: Point<f64, Logical>,
    selected_output_info: Option<&OutputInfo>,
    under_is_layer_surface: bool,
) -> Option<HitInfo> {
    if under_is_layer_surface {
        return None;
    }

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
            // Allow SSD frame hit-testing when the pointer is outside the client
            // surface and element_under() returns None. Reverse order prefers topmost.
            let windows: Vec<_> = space.elements().cloned().collect();
            windows.iter().rev().find_map(|window| {
                let window_loc = space.element_location(window)?;
                hit_for_window(window, window_loc)
            })
        })
}

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

fn send_pointer_button(
    pointer: &smithay::input::pointer::PointerHandle<MeridianState>,
    state: &mut MeridianState,
    button: u32,
    button_state: ButtonState,
    serial: smithay::utils::Serial,
    time: u32,
) {
    pointer.button(
        state,
        &ButtonEvent {
            button,
            state: button_state,
            serial,
            time,
        },
    );
    pointer.frame(state);
}
