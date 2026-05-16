use std::os::unix::io::OwnedFd;

use smithay::{
    delegate_dispatch2,
    desktop::Window,
    input::{dnd::DndGrabHandler, pointer::CursorImageStatus, Seat, SeatHandler},
    reexports::{
        wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode,
        wayland_server::{protocol::wl_surface::WlSurface, Resource},
    },
    utils::{Logical, Point, Rectangle, Serial},
    wayland::{
        output::OutputHandler,
        selection::{
            data_device::{DataDeviceHandler, DataDeviceState, WaylandDndGrabHandler},
            primary_selection::{PrimarySelectionHandler, PrimarySelectionState},
            SelectionHandler, SelectionSource, SelectionTarget,
        },
        shell::xdg::{decoration::XdgDecorationHandler, ToplevelSurface},
    },
};
use tracing::debug;

use crate::state::{normal_window_workarea_from_output_geometry, MeridianState};

fn clamp_client_loc_for_visible_frame(
    client_loc: Point<i32, Logical>,
    frame_rect: Rectangle<i32, Logical>,
    workarea: Rectangle<i32, Logical>,
) -> Point<i32, Logical> {
    let workarea_right = workarea.loc.x.saturating_add(workarea.size.w.max(1));
    let workarea_bottom = workarea.loc.y.saturating_add(workarea.size.h.max(1));

    let mut frame_x = frame_rect.loc.x;
    if frame_rect.size.w >= workarea.size.w {
        frame_x = workarea.loc.x;
    } else {
        let max_x = workarea_right.saturating_sub(frame_rect.size.w.max(1));
        if frame_x < workarea.loc.x {
            frame_x = workarea.loc.x;
        }
        if frame_x > max_x {
            frame_x = max_x;
        }
    }

    let mut frame_y = frame_rect.loc.y;
    if frame_rect.size.h >= workarea.size.h {
        frame_y = workarea.loc.y;
    } else {
        let max_y = workarea_bottom.saturating_sub(frame_rect.size.h.max(1));
        if frame_y < workarea.loc.y {
            frame_y = workarea.loc.y;
        }
        if frame_y > max_y {
            frame_y = max_y;
        }
    }

    let dx = i64::from(frame_x) - i64::from(frame_rect.loc.x);
    let dy = i64::from(frame_y) - i64::from(frame_rect.loc.y);
    let corrected_x =
        (i64::from(client_loc.x) + dx).clamp(i64::from(i32::MIN), i64::from(i32::MAX));
    let corrected_y =
        (i64::from(client_loc.y) + dy).clamp(i64::from(i32::MIN), i64::from(i32::MAX));

    (corrected_x as i32, corrected_y as i32).into()
}

fn find_mapped_xdg_window(
    state: &MeridianState,
    surface: &WlSurface,
) -> Option<(usize, Window, Point<i32, Logical>)> {
    (0..state.workspaces.count()).find_map(|workspace| {
        let space = state.workspaces.space_at(workspace);
        let window = space
            .elements()
            .find(|window| {
                window
                    .toplevel()
                    .is_some_and(|toplevel| toplevel.wl_surface() == surface)
            })?
            .clone();
        let loc = space.element_location(&window)?;
        Some((workspace, window, loc))
    })
}

fn reposition_xdg_window_for_visible_frame(state: &mut MeridianState, toplevel: &ToplevelSurface) {
    let Some((workspace, window, client_loc)) =
        find_mapped_xdg_window(state, toplevel.wl_surface())
    else {
        return;
    };

    let theme = &state.theme_manager.current().config.decorations;
    let content_size = window.geometry().size;
    let frame = state.decoration_manager.ssd_render_metrics(
        toplevel.wl_surface(),
        client_loc,
        content_size,
        theme,
    );
    let frame_rect = frame.frame_rect;
    let center_x = frame_rect.loc.x as f64 + (frame_rect.size.w.max(1) as f64 * 0.5);
    let center_y = frame_rect.loc.y as f64 + (frame_rect.size.h.max(1) as f64 * 0.5);

    let Some(output_geometry) = state
        .output_registry
        .select_for_point_with_fallback(center_x, center_y)
        .map(|output| output.geometry)
    else {
        return;
    };

    let workarea_geo = normal_window_workarea_from_output_geometry(output_geometry);
    let workarea = Rectangle::new(
        (workarea_geo.x, workarea_geo.y).into(),
        (workarea_geo.width.max(1), workarea_geo.height.max(1)).into(),
    );
    let corrected_client_loc = clamp_client_loc_for_visible_frame(client_loc, frame_rect, workarea);
    if corrected_client_loc != client_loc {
        state
            .workspaces
            .space_at_mut(workspace)
            .map_element(window, corrected_client_loc, false);
        state.mark_all_outputs_dirty("xdg-decoration-mode-reposition");
    }
}

impl SeatHandler for MeridianState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut smithay::input::SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.display_handle;
        let client = focused.and_then(|surface| dh.get_client(surface.id()).ok());
        smithay::wayland::selection::data_device::set_data_device_focus(dh, seat, client.clone());
        smithay::wayland::selection::primary_selection::set_primary_focus(dh, seat, client);
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        debug!(?image, "cursor image update received from client");
        self.cursor_status = image;
        self.mark_all_outputs_dirty("cursor-status-changed");
    }
}

impl OutputHandler for MeridianState {}

impl SelectionHandler for MeridianState {
    type SelectionUserData = ();

    fn new_selection(
        &mut self,
        ty: SelectionTarget,
        source: Option<SelectionSource>,
        _seat: Seat<Self>,
    ) {
        if let Some(xwm) = self.xwm.as_mut() {
            if let Err(err) = xwm.new_selection(ty, source.map(|source| source.mime_types())) {
                tracing::warn!(
                    ?err,
                    ?ty,
                    "failed to set xwayland selection from wayland owner"
                );
            }
        }
    }

    fn send_selection(
        &mut self,
        ty: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
        _seat: Seat<Self>,
        _user_data: &Self::SelectionUserData,
    ) {
        if let Some(xwm) = self.xwm.as_mut() {
            if let Err(err) = xwm.send_selection(ty, mime_type, fd) {
                tracing::warn!(
                    ?err,
                    ?ty,
                    "failed to send x11 selection data to wayland requestor"
                );
            }
        }
    }
}

impl PrimarySelectionHandler for MeridianState {
    fn primary_selection_state(&mut self) -> &mut PrimarySelectionState {
        &mut self.primary_selection_state
    }
}

impl WaylandDndGrabHandler for MeridianState {}

impl DataDeviceHandler for MeridianState {
    fn data_device_state(&mut self) -> &mut DataDeviceState {
        &mut self.data_device_state
    }
}

impl DndGrabHandler for MeridianState {}

impl XdgDecorationHandler for MeridianState {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = None;
        });
        self.decoration_manager
            .set_ssd(toplevel.wl_surface(), false);
        reposition_xdg_window_for_visible_frame(self, &toplevel);
        toplevel.send_configure();
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: DecorationMode) {
        let ssd = mode == DecorationMode::ServerSide;
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(mode);
        });
        self.decoration_manager.set_ssd(toplevel.wl_surface(), ssd);
        reposition_xdg_window_for_visible_frame(self, &toplevel);
        toplevel.send_configure();
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = None;
        });
        self.decoration_manager
            .set_ssd(toplevel.wl_surface(), false);
        reposition_xdg_window_for_visible_frame(self, &toplevel);
        toplevel.send_configure();
    }
}

#[cfg(test)]
mod tests {
    use smithay::utils::{Point, Rectangle, Size};

    use super::clamp_client_loc_for_visible_frame;

    #[test]
    fn moves_client_down_when_frame_top_would_be_offscreen() {
        let client = Point::from((0, 0));
        let frame = Rectangle::new((-2, -34).into(), (644, 436).into());
        let workarea = Rectangle::new((0, 0).into(), (1920, 1044).into());
        let corrected = clamp_client_loc_for_visible_frame(client, frame, workarea);
        assert_eq!(corrected, Point::from((2, 34)));
    }

    #[test]
    fn moves_client_right_when_left_border_would_be_offscreen() {
        let client = Point::from((0, 100));
        let frame = Rectangle::new((-6, 66).into(), (800, 500).into());
        let workarea = Rectangle::new((0, 0).into(), (1920, 1044).into());
        let corrected = clamp_client_loc_for_visible_frame(client, frame, workarea);
        assert_eq!(corrected.x, 6);
        assert_eq!(corrected.y, 100);
    }

    #[test]
    fn keeps_location_when_frame_is_already_fully_visible() {
        let client = Point::from((120, 140));
        let frame = Rectangle::new((100, 100).into(), (640, 480).into());
        let workarea = Rectangle::new((0, 0).into(), (1920, 1044).into());
        let corrected = clamp_client_loc_for_visible_frame(client, frame, workarea);
        assert_eq!(corrected, client);
    }

    #[test]
    fn oversized_window_keeps_top_left_reachable() {
        let client = Point::from((50, 50));
        let frame = Rectangle::new((20, 10).into(), Size::from((3000, 2000)));
        let workarea = Rectangle::new((0, 0).into(), (1920, 1044).into());
        let corrected = clamp_client_loc_for_visible_frame(client, frame, workarea);
        assert_eq!(corrected, Point::from((30, 40)));
    }
}

impl MeridianState {
    pub fn update_focus_decoration(&mut self, old: Option<&WlSurface>, new: Option<&WlSurface>) {
        if let Some(old_surf) = old {
            self.decoration_manager.set_focused(old_surf, false);
        }
        if let Some(new_surf) = new {
            self.decoration_manager.set_focused(new_surf, true);
        }
    }

    pub fn set_keyboard_focus_with_decorations(
        &mut self,
        new_focus: Option<WlSurface>,
        serial: Serial,
    ) {
        let Some(keyboard) = self.seat.get_keyboard() else {
            return;
        };

        let old_focus = keyboard.current_focus();
        if old_focus != new_focus {
            self.update_focus_decoration(old_focus.as_ref(), new_focus.as_ref());
            self.mark_all_outputs_dirty("keyboard-focus-change");
        }

        keyboard.set_focus(self, new_focus, serial);
    }
}

delegate_dispatch2!(MeridianState);
