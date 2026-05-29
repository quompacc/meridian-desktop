use smithay::{
    desktop::Window,
    reexports::wayland_server::{protocol::wl_surface::WlSurface, Client, Resource},
    wayland::{
        compositor::{with_states, CompositorClientState},
        shell::xdg::XdgToplevelSurfaceData,
    },
    xwayland::XWaylandClientData,
};

use super::ClientState;

pub(crate) fn window_id(surface: &WlSurface) -> String {
    surface.id().to_string()
}

/// Window-list / runtime-state key for an XWayland window. Shared between
/// `window_list_entry` (the id handed to the shell) and the compositor's
/// destroy/unmap cleanup so the two never diverge -- a mismatch would leave
/// closed X11 windows lingering as taskbar ghosts.
pub(crate) fn x11_window_id_key(window_id: u32) -> String {
    format!("x11:{window_id}")
}

pub(crate) fn window_list_entry(window: &Window) -> Option<(String, String)> {
    if let Some(toplevel) = window.toplevel() {
        return Some((window_id(toplevel.wl_surface()), toplevel_title(toplevel)));
    }

    window.x11_surface().map(|x11| {
        let id = x11_window_id_key(x11.window_id());
        let title = x11.title();
        let class = x11.class();
        let instance = x11.instance();
        let fallback_title = format!("X11 window {}", x11.window_id());
        let resolved_title = if !title.trim().is_empty() {
            title
        } else if !class.trim().is_empty() && !instance.trim().is_empty() && class != instance {
            format!("{} ({})", class, instance)
        } else if !class.trim().is_empty() {
            class
        } else if !instance.trim().is_empty() {
            instance
        } else {
            fallback_title
        };
        (id, resolved_title)
    })
}

pub(crate) fn window_app_id(window: &Window) -> Option<String> {
    if let Some(toplevel) = window.toplevel() {
        return with_states(toplevel.wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()?
                .lock()
                .ok()?
                .app_id
                .clone()
        });
    }
    window.x11_surface().map(|x11| {
        let class = x11.class();
        let instance = x11.instance();
        if !class.trim().is_empty() {
            class
        } else {
            instance
        }
    })
}

pub(crate) fn toplevel_title(surface: &smithay::wayland::shell::xdg::ToplevelSurface) -> String {
    with_states(surface.wl_surface(), |states| {
        let data = states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .unwrap()
            .lock()
            .unwrap();

        data.title
            .clone()
            .or_else(|| data.app_id.clone())
            .unwrap_or_else(|| "Window".to_string())
    })
}

pub(crate) fn client_compositor_state(client: &Client) -> &CompositorClientState {
    if let Some(state) = client.get_data::<XWaylandClientData>() {
        return &state.compositor_state;
    }
    &client.get_data::<ClientState>().unwrap().compositor_state
}

#[cfg(test)]
mod tests {
    use super::x11_window_id_key;

    #[test]
    fn x11_window_id_key_matches_window_list_scheme() {
        // window_list_entry derives the X11 id via this helper, and the
        // xwayland destroyed/unmapped cleanup keys runtime state by it
        // (via x11_window_key). If the formats diverge, destroyed X11
        // windows leak as taskbar ghosts (regression: XW-1).
        assert_eq!(x11_window_id_key(42), "x11:42");
        assert_eq!(x11_window_id_key(0), "x11:0");
        assert_eq!(x11_window_id_key(u32::MAX), format!("x11:{}", u32::MAX));
    }
}
