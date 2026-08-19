use smithay_client_toolkit::{
    seat::pointer::{PointerEvent, PointerEventKind, PointerHandler},
    shell::WaylandSurface,
};
use wayland_client::{protocol::wl_pointer, Connection, QueueHandle};

use crate::{
    audio_popup, context_menu,
    network_popup::popup_hit_test,
    status_notifier_popup,
    wayland::{RepaintReason, SurfaceKind},
    workspaces,
};

use super::{pointer_translate::translate_pointer_event, MeridianShell};

include!("pointer/overlays_and_desktop.rs");
include!("pointer/launcher.rs");
include!("pointer/panel_and_popups.rs");

impl PointerHandler for MeridianShell {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            handle_overlays_and_desktop_pointer!(self, qh, event);
            handle_launcher_pointer!(self, qh, event);
            handle_panel_and_popups_pointer!(self, qh, event);
        }
    }
}
