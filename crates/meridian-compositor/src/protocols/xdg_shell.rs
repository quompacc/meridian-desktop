use smithay::{
    desktop::{PopupKind, PopupManager, Space, Window},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    wayland::{compositor::with_states, shell::xdg::XdgToplevelSurfaceData},
};

/// Sendet den initialen Configure und tracked Popup-Commits.
/// Muss aus `CompositorHandler::commit` aufgerufen werden.
pub fn handle_commit(popups: &mut PopupManager, space: &Space<Window>, surface: &WlSurface) {
    // Initialen configure an neue Toplevels senden (Wayland only — X11 windows have no toplevel)
    if let Some(window) = space
        .elements()
        .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == surface))
        .cloned()
    {
        let toplevel = match window.toplevel() {
            Some(t) => t,
            None => return,
        };
        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });
        if !initial_configure_sent {
            toplevel.send_configure();
        }
    }

    // Popup commits und initialer configure
    popups.commit(surface);
    if let Some(popup) = popups.find_popup(surface) {
        match popup {
            PopupKind::Xdg(ref xdg) => {
                if !xdg.is_initial_configure_sent() {
                    if let Err(err) = xdg.send_configure() {
                        // A misbehaving client can reach PopupConfigureError here
                        // (popup already configured, old protocol version without
                        // reposition). Dismiss that one popup instead of tearing
                        // down the whole session (P2-3, AUDIT_2026-08-19).
                        tracing::warn!(
                            "initial popup configure failed ({:?}); sending popup_done",
                            err
                        );
                        xdg.send_popup_done();
                    }
                }
            }
            PopupKind::InputMethod(_) => {}
        }
    }
}
