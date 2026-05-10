use smithay::{desktop::Window, wayland::shell::xdg::ToplevelSurface};

use crate::state::MeridianState;

pub(super) fn find_active_window(
    state: &MeridianState,
    surface: &ToplevelSurface,
) -> Option<Window> {
    state
        .workspaces
        .active_space()
        .elements()
        .find(|window| {
            window.toplevel().map_or(false, |toplevel| {
                toplevel.wl_surface() == surface.wl_surface()
            })
        })
        .cloned()
}
