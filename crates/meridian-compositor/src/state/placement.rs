use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle},
    wayland::seat::WaylandFocus,
};

use super::{normal_window_workarea_from_output_geometry, window_id, MeridianState};

fn centered_client_location(
    client_loc: Point<i32, Logical>,
    frame_rect: Rectangle<i32, Logical>,
    workarea: Rectangle<i32, Logical>,
) -> Point<i32, Logical> {
    let frame_width = frame_rect.size.w.max(1).min(workarea.size.w.max(1));
    let frame_height = frame_rect.size.h.max(1).min(workarea.size.h.max(1));
    let frame_x = workarea
        .loc
        .x
        .saturating_add(workarea.size.w.saturating_sub(frame_width) / 2);
    let frame_y = workarea
        .loc
        .y
        .saturating_add(workarea.size.h.saturating_sub(frame_height) / 2);
    let client_offset_x = client_loc.x.saturating_sub(frame_rect.loc.x);
    let client_offset_y = client_loc.y.saturating_sub(frame_rect.loc.y);

    (
        frame_x.saturating_add(client_offset_x),
        frame_y.saturating_add(client_offset_y),
    )
        .into()
}

impl MeridianState {
    pub(crate) fn center_pending_xdg_toplevel(&mut self, surface: &WlSurface) {
        let key = window_id(surface);
        if !self.pending_initial_xdg_placement.contains(&key) {
            return;
        }

        let Some((workspace, window, client_loc)) =
            (0..self.workspaces.count()).find_map(|index| {
                let space = self.workspaces.space_at(index);
                let window = space
                    .elements()
                    .find(|window| {
                        window
                            .wl_surface()
                            .map(|candidate| candidate.into_owned())
                            .as_ref()
                            == Some(surface)
                    })?
                    .clone();
                Some((index, window.clone(), space.element_location(&window)?))
            })
        else {
            return;
        };

        let content_size = window.geometry().size;
        if content_size.w <= 0 || content_size.h <= 0 {
            return;
        }

        let output_geometry = self
            .focused_output()
            .and_then(|id| self.output_registry.by_id(id))
            .or_else(|| self.output_registry.primary())
            .map(|info| info.geometry);
        let Some(output_geometry) = output_geometry else {
            return;
        };
        let workarea_geometry = normal_window_workarea_from_output_geometry(output_geometry);
        let workarea = Rectangle::new(
            (workarea_geometry.x, workarea_geometry.y).into(),
            (
                workarea_geometry.width.max(1),
                workarea_geometry.height.max(1),
            )
                .into(),
        );
        let frame = self.decoration_manager.ssd_render_metrics(
            surface,
            client_loc,
            content_size,
            &self.theme_manager.current().config.decorations,
        );
        let centered = centered_client_location(client_loc, frame.frame_rect, workarea);

        self.workspaces
            .space_at_mut(workspace)
            .map_element(window, centered, false);
        self.pending_initial_xdg_placement.remove(&key);
        self.mark_all_outputs_dirty("xdg-initial-placement-centered");
        tracing::debug!(
            window = %key,
            ?content_size,
            ?centered,
            "centered newly mapped xdg toplevel"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centers_the_complete_decorated_frame() {
        let client_loc = Point::from((100, 100));
        let frame = Rectangle::new((98, 66).into(), (804, 636).into());
        let workarea = Rectangle::new((0, 0).into(), (1920, 1044).into());

        let centered = centered_client_location(client_loc, frame, workarea);

        assert_eq!(centered, Point::from((560, 238)));
        assert_eq!(centered.x - 2, (1920 - 804) / 2);
        assert_eq!(centered.y - 34, (1044 - 636) / 2);
    }

    #[test]
    fn oversized_frame_is_anchored_at_the_workarea_origin() {
        let client_loc = Point::from((12, 34));
        let frame = Rectangle::new((10, 0).into(), (2200, 1200).into());
        let workarea = Rectangle::new((100, 50).into(), (1920, 1044).into());

        assert_eq!(
            centered_client_location(client_loc, frame, workarea),
            Point::from((102, 84))
        );
    }
}
