use std::time::{Duration, Instant};

use smithay::{
    desktop::{layer_map_for_output, WindowSurfaceType},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
};
use tracing::debug;

use super::super::MeridianState;
use crate::state::OutputInfo;

fn select_surface_output_info(
    infos: &[OutputInfo],
    point: Option<Point<f64, Logical>>,
) -> (Option<&OutputInfo>, &'static str) {
    if let Some(pos) = point {
        if let Some(output) = infos
            .iter()
            .find(|info| info.geometry.contains(pos.x, pos.y))
        {
            return (Some(output), "point-match");
        }
    }

    if let Some(output) = infos.iter().find(|info| info.primary) {
        return (Some(output), "fallback-primary");
    }

    if let Some(output) = infos.first() {
        return (Some(output), "fallback-first");
    }

    (None, "empty-registry")
}

impl MeridianState {
    fn should_log_surface_under_pointer_diag(&mut self, pos: Point<f64, Logical>) -> bool {
        const MIN_DELTA_PX: f64 = 5.0;
        const MIN_INTERVAL: Duration = Duration::from_millis(100);

        let moved_enough = self
            .last_diag_pointer_pos
            .map(|(last_x, last_y)| {
                (pos.x - last_x).abs() >= MIN_DELTA_PX || (pos.y - last_y).abs() >= MIN_DELTA_PX
            })
            .unwrap_or(true);

        let now = Instant::now();
        let interval_elapsed = self
            .last_diag_pointer_log_at
            .map(|last| now.duration_since(last) >= MIN_INTERVAL)
            .unwrap_or(true);

        if moved_enough && interval_elapsed {
            self.last_diag_pointer_pos = Some((pos.x, pos.y));
            self.last_diag_pointer_log_at = Some(now);
            return true;
        }

        false
    }

    pub fn surface_under(
        &mut self,
        pos: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        let (selected_info, fallback_reason) =
            select_surface_output_info(self.output_registry.list(), Some(pos));
        if let Some(info) = selected_info {
            debug!(
                "surface output selection requested: x={:.2} y={:.2} selected_output_id={} name={} fallback_reason={}",
                pos.x, pos.y, info.id.0, info.name, fallback_reason
            );
        } else {
            debug!(
                "surface output selection requested: x={:.2} y={:.2} selected_output=none fallback_reason={}",
                pos.x, pos.y, fallback_reason
            );
        }

        let output = selected_info.and_then(|info| {
            let mapped = self
                .outputs
                .iter()
                .find(|candidate| candidate.name() == info.name);
            if mapped.is_none() {
                debug!(
                    "surface output selection fallback: registry output '{}' not present in active output list",
                    info.name
                );
            }
            mapped.cloned()
        });

        if let Some(output) = output.as_ref() {
            let output_geo = self.workspaces.active_space().output_geometry(output)?;
            let layer_map = layer_map_for_output(output);
            let local = pos - output_geo.loc.to_f64();

            for layer in [
                smithay::wayland::shell::wlr_layer::Layer::Overlay,
                smithay::wayland::shell::wlr_layer::Layer::Top,
            ] {
                if let Some(surface) = layer_map.layer_under(layer, local) {
                    if let Some(geo) = layer_map.layer_geometry(surface) {
                        return surface
                            .surface_under(local - geo.loc.to_f64(), WindowSurfaceType::ALL)
                            .map(|(surface, point)| {
                                (surface, (point + output_geo.loc + geo.loc).to_f64())
                            });
                    }
                }
            }

            // Keep launcher hit-testing top-priority even if its cached role is stale.
            let launcher_surface = layer_map
                .layers()
                .find(|layer| layer.namespace() == "meridian-launcher")
                .cloned();
            if let Some(launcher_surface) = launcher_surface {
                if let Some(geo) = layer_map.layer_geometry(&launcher_surface) {
                    if let Some((surface, point)) = launcher_surface
                        .surface_under(local - geo.loc.to_f64(), WindowSurfaceType::ALL)
                    {
                        return Some((surface, (point + output_geo.loc + geo.loc).to_f64()));
                    }
                }
            }
        }

        let window_hit = {
            let space = self.workspaces.active_space();
            space
                .element_under(pos)
                .map(|(window, location)| (window.clone(), location))
        };
        let window_surface = window_hit.and_then(|(window, location)| {
            let geometry = window.geometry();
            let bbox = window.bbox();
            let local = pos - location.to_f64();
            if self.should_log_surface_under_pointer_diag(pos) {
                tracing::info!(
                    input_pos = ?(pos.x, pos.y),
                    window_location = ?(location.x, location.y),
                    window_geometry = ?(geometry.loc.x, geometry.loc.y, geometry.size.w, geometry.size.h),
                    window_bbox = ?(bbox.loc.x, bbox.loc.y, bbox.size.w, bbox.size.h),
                    local_pointer = ?(local.x, local.y),
                    "diagnostic: surface_under pointer trace"
                );
            }
            window
                .surface_under(local, WindowSurfaceType::ALL)
                .map(|(surface, point)| (surface, (point + location).to_f64()))
        });

        if window_surface.is_some() {
            return window_surface;
        }

        if let Some(output) = output.as_ref() {
            let output_geo = self.workspaces.active_space().output_geometry(output)?;
            let layer_map = layer_map_for_output(output);
            let local = pos - output_geo.loc.to_f64();

            for layer in [
                smithay::wayland::shell::wlr_layer::Layer::Bottom,
                smithay::wayland::shell::wlr_layer::Layer::Background,
            ] {
                if let Some(surface) = layer_map.layer_under(layer, local) {
                    if let Some(geo) = layer_map.layer_geometry(surface) {
                        return surface
                            .surface_under(local - geo.loc.to_f64(), WindowSurfaceType::ALL)
                            .map(|(surface, point)| {
                                (surface, (point + output_geo.loc + geo.loc).to_f64())
                            });
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use smithay::utils::{Logical, Point, Transform};

    use crate::state::{OutputGeometry, OutputId, OutputInfo, OutputRegistration, OutputRegistry};

    fn reg(name: &str, x: i32, y: i32, width: i32, height: i32) -> OutputRegistration {
        OutputRegistration {
            name: name.to_string(),
            geometry: OutputGeometry {
                x,
                y,
                width,
                height,
            },
            scale: 1.0,
            transform: Transform::Normal,
            refresh_millihz: Some(60_000),
        }
    }

    #[test]
    fn point_on_output_one_is_selected() {
        let mut registry = OutputRegistry::new();
        registry.upsert(reg("left", 0, 0, 1920, 1080));
        registry.upsert(reg("right", 1920, 0, 2560, 1440));

        let point: Point<f64, Logical> = (100.0, 100.0).into();
        let (selected, reason) = super::select_surface_output_info(registry.list(), Some(point));
        assert_eq!(selected.map(|output| output.name.as_str()), Some("left"));
        assert_eq!(reason, "point-match");
    }

    #[test]
    fn point_on_output_two_is_selected() {
        let mut registry = OutputRegistry::new();
        registry.upsert(reg("left", 0, 0, 1920, 1080));
        registry.upsert(reg("right", 1920, 0, 2560, 1440));

        let point: Point<f64, Logical> = (2300.0, 300.0).into();
        let (selected, reason) = super::select_surface_output_info(registry.list(), Some(point));
        assert_eq!(selected.map(|output| output.name.as_str()), Some("right"));
        assert_eq!(reason, "point-match");
    }

    #[test]
    fn outside_point_uses_primary_fallback() {
        let mut registry = OutputRegistry::new();
        registry.upsert(reg("primary", 0, 0, 1920, 1080));
        registry.upsert(reg("other", 1920, 0, 1920, 1080));

        let point: Point<f64, Logical> = (-100.0, -100.0).into();
        let (selected, reason) = super::select_surface_output_info(registry.list(), Some(point));
        assert_eq!(selected.map(|output| output.name.as_str()), Some("primary"));
        assert_eq!(reason, "fallback-primary");
    }

    #[test]
    fn first_fallback_is_used_when_no_primary_exists() {
        let infos = vec![
            OutputInfo {
                id: OutputId(1),
                name: "first".to_string(),
                geometry: OutputGeometry {
                    x: 0,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
                scale: 1.0,
                transform: Transform::Normal,
                refresh_millihz: Some(60_000),
                primary: false,
            },
            OutputInfo {
                id: OutputId(2),
                name: "second".to_string(),
                geometry: OutputGeometry {
                    x: 1280,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
                scale: 1.0,
                transform: Transform::Normal,
                refresh_millihz: Some(60_000),
                primary: false,
            },
        ];

        let (selected, reason) = super::select_surface_output_info(&infos, None);
        assert_eq!(selected.map(|output| output.name.as_str()), Some("first"));
        assert_eq!(reason, "fallback-first");
    }

    #[test]
    fn empty_registry_is_safe() {
        let registry = OutputRegistry::new();
        let (selected, reason) = super::select_surface_output_info(registry.list(), None);
        assert!(selected.is_none());
        assert_eq!(reason, "empty-registry");
    }
}
