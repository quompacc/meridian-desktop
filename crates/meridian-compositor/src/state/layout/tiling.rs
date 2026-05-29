use meridian_wm::WorkspaceMode;
use smithay::{
    desktop::Window,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    utils::{Logical, Rectangle},
};

use super::super::MeridianState;
use crate::state::{
    normal_window_workarea_from_output_geometry, OutputGeometry, OutputId, OutputInfo,
};

impl MeridianState {
    pub fn tile_workspace(&mut self, idx: usize) {
        tracing::debug!("tiling output geometry requested: workspace={}", idx + 1);
        let output_rect =
            if let Some(selected) = select_tiling_output_from_infos(self.output_registry.list()) {
                tracing::debug!(
                    "tiling selected output: id={} name={} fallback_reason={}",
                    selected.id.0,
                    selected.name,
                    selected.fallback_reason
                );
                selected.geometry
            } else {
                tracing::debug!(
                    "tiling selected output: none (registry empty), using default geometry"
                );
                Rectangle::new((0, 0).into(), (1920, 1080).into())
            };
        let gap = self.theme_manager.current().config.decorations.gap as i32;

        let space_windows: Vec<Window> =
            self.workspaces.space_at(idx).elements().cloned().collect();
        for window in self.wm_workspaces[idx].tiled_windows() {
            if !space_windows
                .iter()
                .any(|space_window| space_window == &window)
            {
                self.wm_workspaces[idx].remove_tiled(&window);
            }
        }

        let assignments = self.wm_workspaces[idx].compute_tiled(output_rect, gap);
        if assignments.is_empty() {
            return;
        }

        let space = self.workspaces.space_at_mut(idx);
        for (window, rect) in assignments {
            if let Some(toplevel) = window.toplevel() {
                toplevel.with_pending_state(|state| {
                    state.size = Some(rect.size);
                    state.states.set(xdg_toplevel::State::TiledLeft);
                    state.states.set(xdg_toplevel::State::TiledRight);
                    state.states.set(xdg_toplevel::State::TiledTop);
                    state.states.set(xdg_toplevel::State::TiledBottom);
                });
                toplevel.send_pending_configure();
            }
            space.map_element(window, rect.loc, false);
        }
    }

    pub fn toggle_tiling(&mut self) {
        let active = self.workspaces.active;
        let new_mode = self.wm_workspaces[active].toggle_mode();
        if new_mode == WorkspaceMode::Tiling {
            let windows: Vec<Window> = self.workspaces.active_space().elements().cloned().collect();
            self.wm_workspaces[active].rebuild_tiling_from(windows.into_iter());
            self.tile_workspace(active);
        }
        tracing::info!("Workspace {} → {:?}", active, new_mode);
    }
}

#[derive(Debug, Clone)]
struct SelectedTilingOutput {
    id: OutputId,
    name: String,
    geometry: Rectangle<i32, Logical>,
    fallback_reason: &'static str,
}

fn output_geometry_to_rect(geometry: OutputGeometry) -> Rectangle<i32, Logical> {
    let geometry = normal_window_workarea_from_output_geometry(geometry);
    Rectangle::new(
        (geometry.x, geometry.y).into(),
        (geometry.width, geometry.height).into(),
    )
}

fn select_tiling_output_from_infos(infos: &[OutputInfo]) -> Option<SelectedTilingOutput> {
    if let Some(info) = infos.iter().find(|info| info.primary) {
        return Some(SelectedTilingOutput {
            id: info.id,
            name: info.name.clone(),
            geometry: output_geometry_to_rect(info.geometry),
            fallback_reason: "primary",
        });
    }

    infos.first().map(|info| SelectedTilingOutput {
        id: info.id,
        name: info.name.clone(),
        geometry: output_geometry_to_rect(info.geometry),
        fallback_reason: "first-fallback",
    })
}

#[cfg(test)]
mod tests {
    use smithay::utils::Transform;

    use crate::state::{OutputGeometry, OutputId, OutputInfo, NORMAL_WINDOW_BOTTOM_RESERVED_PX};

    use super::select_tiling_output_from_infos;

    fn info(id: u32, name: &str, primary: bool, x: i32) -> OutputInfo {
        OutputInfo {
            id: OutputId(id),
            name: name.to_string(),
            geometry: OutputGeometry {
                x,
                y: 0,
                width: 1920,
                height: 1080,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
            primary,
        }
    }

    #[test]
    fn tiling_selects_primary_output() {
        let infos = vec![info(1, "a", false, 0), info(2, "b", true, 1920)];
        let selected = select_tiling_output_from_infos(&infos).expect("selection");
        assert_eq!(selected.id.0, 2);
        assert_eq!(selected.fallback_reason, "primary");
        assert_eq!(
            selected.geometry.size.h,
            1080 - NORMAL_WINDOW_BOTTOM_RESERVED_PX
        );
    }

    #[test]
    fn tiling_selects_first_when_no_primary_exists() {
        let infos = vec![info(10, "first", false, 0), info(11, "second", false, 1920)];
        let selected = select_tiling_output_from_infos(&infos).expect("selection");
        assert_eq!(selected.id.0, 10);
        assert_eq!(selected.fallback_reason, "first-fallback");
    }

    #[test]
    fn tiling_handles_empty_infos() {
        assert!(select_tiling_output_from_infos(&[]).is_none());
    }
}
