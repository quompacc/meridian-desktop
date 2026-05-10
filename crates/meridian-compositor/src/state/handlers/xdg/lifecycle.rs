use smithay::{
    desktop::{PopupKind, Window},
    utils::SERIAL_COUNTER,
    wayland::shell::xdg::{PopupSurface, ToplevelSurface},
};

use meridian_wm::WorkspaceMode;

use crate::state::MeridianState;

pub(super) fn handle_new_toplevel(state: &mut MeridianState, surface: ToplevelSurface) {
    tracing::info!(
        "new xdg toplevel: {}",
        crate::state::toplevel_title(&surface)
    );
    state.broadcast_toplevel_opened(&surface);
    let wl_surface = surface.wl_surface().clone();
    let window = Window::new_wayland_window(surface.clone());

    state.decoration_manager.set_ssd(&wl_surface, true);

    let active = state.workspaces.active;
    if state.wm_workspaces[active].mode == WorkspaceMode::Tiling {
        state
            .workspaces
            .active_space_mut()
            .map_element(window.clone(), (0, 0), true);
        state.decoration_manager.set_tiled(&wl_surface, true);
        let focused = state.focused_window();
        state.wm_workspaces[active].add_tiled(window, focused.as_ref());
        state.tile_workspace(active);
    } else {
        let theme = &state.theme_manager.current().config.decorations;
        let (ox, oy) = state
            .decoration_manager
            .decoration_offset(&wl_surface, theme);
        state
            .workspaces
            .active_space_mut()
            .map_element(window, (ox, oy), true);
    }

    let serial = SERIAL_COUNTER.next_serial();
    if let Some(keyboard) = state.seat.get_keyboard() {
        state.update_focus_decoration(None, Some(&wl_surface));
        keyboard.set_focus(state, Some(wl_surface.clone()), serial);
        state.update_focused_output_from_surface(&wl_surface, "keyboard-focus-new-toplevel");
        state.broadcast_toplevel_focused(&wl_surface);
    }
    state.mark_all_outputs_dirty("xdg-new-toplevel");
}

pub(super) fn handle_new_popup(state: &mut MeridianState, surface: PopupSurface) {
    let _ = state.popups.track_popup(PopupKind::Xdg(surface));
}

pub(super) fn handle_toplevel_destroyed(state: &mut MeridianState, surface: ToplevelSurface) {
    state.decoration_manager.remove(surface.wl_surface());
    state.broadcast_toplevel_closed(&surface);
    state.mark_all_outputs_dirty("xdg-toplevel-destroyed");
}

pub(super) fn handle_surface_metadata_changed(state: &mut MeridianState, surface: ToplevelSurface) {
    state.broadcast_toplevel_opened(&surface);
    state.mark_all_outputs_dirty("xdg-surface-metadata-changed");
}
