use smithay::{
    desktop::{
        find_popup_root_surface, PopupKeyboardGrab, PopupKind, PopupPointerGrab,
        PopupUngrabStrategy,
    },
    input::{pointer::Focus, Seat},
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::protocol::{wl_output::WlOutput, wl_seat::WlSeat, wl_surface::WlSurface},
    },
    utils::{Logical, Point, Rectangle, Serial},
    wayland::shell::xdg::{
        PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
    },
};

use crate::state::{normal_window_workarea_from_output_geometry, MeridianState};

mod lifecycle;
mod requests;

fn popup_parent_window_loc_and_size(
    state: &MeridianState,
    root_surface: &WlSurface,
) -> Option<(Point<i32, Logical>, smithay::utils::Size<i32, Logical>)> {
    let space = state.workspaces.active_space();
    let window = space
        .elements()
        .find(|window| {
            window
                .toplevel()
                .is_some_and(|toplevel| toplevel.wl_surface() == root_surface)
        })?
        .clone();
    let loc = space.element_location(&window)?;
    let size = window.geometry().size;
    Some((loc, size))
}

fn popup_parent_workarea(
    state: &MeridianState,
    root_surface: &WlSurface,
) -> Option<(Point<i32, Logical>, Rectangle<i32, Logical>)> {
    let (parent_loc, parent_size) = popup_parent_window_loc_and_size(state, root_surface)?;
    let center_x = parent_loc.x as f64 + (parent_size.w.max(1) as f64 * 0.5);
    let center_y = parent_loc.y as f64 + (parent_size.h.max(1) as f64 * 0.5);
    let output = state
        .output_registry
        .select_for_point_with_fallback(center_x, center_y)?;
    let workarea_geo = normal_window_workarea_from_output_geometry(output.geometry);
    let workarea = Rectangle::new(
        (workarea_geo.x, workarea_geo.y).into(),
        (workarea_geo.width.max(1), workarea_geo.height.max(1)).into(),
    );
    Some((parent_loc, workarea))
}

pub(super) fn unconstrain_popup_geometry(
    positioner: &PositionerState,
    parent_loc: Point<i32, Logical>,
    workarea: Rectangle<i32, Logical>,
) -> Rectangle<i32, Logical> {
    let workarea_rel = Rectangle::new(
        (
            workarea.loc.x.saturating_sub(parent_loc.x),
            workarea.loc.y.saturating_sub(parent_loc.y),
        )
            .into(),
        workarea.size,
    );
    (*positioner).get_unconstrained_geometry(workarea_rel)
}

pub(super) fn configure_popup_pending_state(
    state: &MeridianState,
    surface: &PopupSurface,
    positioner: PositionerState,
) {
    let geometry = find_popup_root_surface(&PopupKind::Xdg(surface.clone()))
        .ok()
        .and_then(|root_surface| popup_parent_workarea(state, &root_surface))
        .map(|(parent_loc, workarea)| unconstrain_popup_geometry(&positioner, parent_loc, workarea))
        .unwrap_or_else(|| positioner.get_geometry());

    surface.with_pending_state(|state| {
        state.geometry = geometry;
        state.positioner = positioner;
    });
}

impl XdgShellHandler for MeridianState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        lifecycle::handle_new_toplevel(self, surface);
    }

    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {
        lifecycle::handle_new_popup(self, surface, positioner);
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        lifecycle::handle_toplevel_destroyed(self, surface);
    }

    fn app_id_changed(&mut self, surface: ToplevelSurface) {
        lifecycle::handle_surface_metadata_changed(self, surface);
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        lifecycle::handle_surface_metadata_changed(self, surface);
    }

    fn grab(&mut self, surface: PopupSurface, seat: WlSeat, serial: Serial) {
        let Some(seat) = Seat::<Self>::from_resource(&seat) else {
            tracing::warn!("popup grab: wl_seat not associated with a known seat");
            return;
        };

        let kind = PopupKind::Xdg(surface);
        let root_surface = match find_popup_root_surface(&kind) {
            Ok(surface) => surface,
            Err(err) => {
                tracing::debug!("popup grab: cannot find root surface: {:?}", err);
                return;
            }
        };

        let mut grab = match self.popups.grab_popup(root_surface, kind, &seat, serial) {
            Ok(grab) => grab,
            Err(err) => {
                tracing::debug!("popup grab denied: {:?}", err);
                return;
            }
        };

        if let Some(keyboard) = seat.get_keyboard() {
            if keyboard.is_grabbed()
                && !(keyboard.has_grab(serial)
                    || keyboard.has_grab(grab.previous_serial().unwrap_or(serial)))
            {
                tracing::debug!("popup grab: keyboard already grabbed by other serial");
                grab.ungrab(PopupUngrabStrategy::All);
                return;
            }
            keyboard.set_focus(self, grab.current_grab(), serial);
            keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
        }

        if let Some(pointer) = seat.get_pointer() {
            if pointer.is_grabbed()
                && !(pointer.has_grab(serial)
                    || pointer.has_grab(grab.previous_serial().unwrap_or_else(|| grab.serial())))
            {
                tracing::debug!("popup grab: pointer already grabbed by other serial");
                grab.ungrab(PopupUngrabStrategy::All);
                return;
            }
            pointer.set_grab(self, PopupPointerGrab::new(&grab), serial, Focus::Keep);
        }
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        configure_popup_pending_state(self, &surface, positioner);
        surface.send_repositioned(token);
        if let Err(err) = surface.send_configure() {
            tracing::debug!("popup reposition: send_configure failed: {:?}", err);
        }
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: WlSeat, serial: Serial) {
        requests::handle_move_request(self, surface, seat, serial);
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        requests::handle_resize_request(self, surface, seat, serial, edges);
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        requests::handle_maximize_request(self, surface);
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        requests::handle_unmaximize_request(self, surface);
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, _output: Option<WlOutput>) {
        requests::handle_fullscreen_request(self, surface);
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        requests::handle_unfullscreen_request(self, surface);
    }

    fn minimize_request(&mut self, surface: ToplevelSurface) {
        requests::handle_minimize_request(self, surface);
    }
}

#[cfg(test)]
mod tests {
    use smithay::{
        reexports::wayland_protocols::xdg::shell::server::xdg_positioner::{
            Anchor, ConstraintAdjustment, Gravity,
        },
        utils::{Point, Rectangle},
        wayland::shell::xdg::PositionerState,
    };

    use super::unconstrain_popup_geometry;

    fn make_positioner(
        size: (i32, i32),
        anchor_rect: Rectangle<i32, smithay::utils::Logical>,
        anchor: Anchor,
        gravity: Gravity,
        adjustment: ConstraintAdjustment,
    ) -> PositionerState {
        let mut positioner = PositionerState::default();
        positioner.rect_size = size.into();
        positioner.anchor_rect = anchor_rect;
        positioner.anchor_edges = anchor;
        positioner.gravity = gravity;
        positioner.constraint_adjustment = adjustment;
        positioner
    }

    #[test]
    fn unconstrain_slides_left_when_popup_overflows_right_edge() {
        let positioner = make_positioner(
            (400, 200),
            Rectangle::new((0, 0).into(), (1, 1).into()),
            Anchor::Right,
            Gravity::Right,
            ConstraintAdjustment::SlideX,
        );
        let parent_loc = Point::from((1800, 200));
        let workarea = Rectangle::new((0, 0).into(), (1920, 1080).into());

        let original = positioner.get_geometry();
        let adjusted = unconstrain_popup_geometry(&positioner, parent_loc, workarea);

        assert!(adjusted.loc.x < original.loc.x);
        let abs_x = adjusted.loc.x + parent_loc.x;
        assert!(abs_x >= workarea.loc.x);
        assert!(abs_x + adjusted.size.w <= workarea.loc.x + workarea.size.w);
    }

    #[test]
    fn unconstrain_flips_up_when_popup_overflows_bottom_edge() {
        let positioner = make_positioner(
            (300, 200),
            Rectangle::new((0, 0).into(), (20, 20).into()),
            Anchor::Bottom,
            Gravity::Bottom,
            ConstraintAdjustment::FlipY,
        );
        let parent_loc = Point::from((100, 1000));
        let workarea = Rectangle::new((0, 0).into(), (1920, 1080).into());

        let original = positioner.get_geometry();
        let adjusted = unconstrain_popup_geometry(&positioner, parent_loc, workarea);

        assert!(adjusted.loc.y < original.loc.y);
        let abs_y = adjusted.loc.y + parent_loc.y;
        assert!(abs_y >= workarea.loc.y);
        assert!(abs_y + adjusted.size.h <= workarea.loc.y + workarea.size.h);
    }

    #[test]
    fn unconstrain_keeps_geometry_when_inside_workarea() {
        let positioner = make_positioner(
            (200, 100),
            Rectangle::new((0, 0).into(), (1, 1).into()),
            Anchor::TopLeft,
            Gravity::BottomRight,
            ConstraintAdjustment::all(),
        );
        let parent_loc = Point::from((500, 300));
        let workarea = Rectangle::new((0, 0).into(), (1920, 1080).into());

        let original = positioner.get_geometry();
        let adjusted = unconstrain_popup_geometry(&positioner, parent_loc, workarea);

        assert_eq!(adjusted, original);
    }

    #[test]
    fn unconstrain_resizes_when_popup_is_larger_than_workarea() {
        let positioner = make_positioner(
            (3000, 2000),
            Rectangle::new((0, 0).into(), (1, 1).into()),
            Anchor::TopLeft,
            Gravity::BottomRight,
            ConstraintAdjustment::ResizeX | ConstraintAdjustment::ResizeY,
        );
        let parent_loc = Point::from((0, 0));
        let workarea = Rectangle::new((0, 0).into(), (800, 600).into());

        let adjusted = unconstrain_popup_geometry(&positioner, parent_loc, workarea);

        assert_eq!(adjusted.loc, Point::from((0, 0)));
        assert_eq!(adjusted.size.w, 800);
        assert_eq!(adjusted.size.h, 600);
    }
}
