use smithay::{
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
