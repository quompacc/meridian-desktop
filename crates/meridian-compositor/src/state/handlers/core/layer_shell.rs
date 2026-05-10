use smithay::{
    desktop::{layer_map_for_output, LayerSurface as DesktopLayerSurface, PopupKind},
    output::Output,
    reexports::wayland_server::protocol::wl_output::WlOutput,
    wayland::shell::{
        wlr_layer::{
            Layer as WlrLayer, LayerSurface as WlrLayerSurface, WlrLayerShellHandler,
            WlrLayerShellState,
        },
        xdg::PopupSurface,
    },
};

use super::super::super::MeridianState;
use crate::state::OutputInfo;

fn select_layer_output_info<'a>(
    infos: &'a [OutputInfo],
    requested_output_name: Option<&str>,
) -> Option<(&'a OutputInfo, &'static str)> {
    if let Some(requested) = requested_output_name {
        if let Some(info) = infos.iter().find(|info| info.name == requested) {
            return Some((info, "explicit-output"));
        }
        if let Some(info) = infos.iter().find(|info| info.primary) {
            return Some((info, "fallback-primary-unknown-requested"));
        }
        return infos
            .first()
            .map(|info| (info, "fallback-first-unknown-requested"));
    }

    if let Some(info) = infos.iter().find(|info| info.primary) {
        return Some((info, "fallback-primary"));
    }
    infos.first().map(|info| (info, "fallback-first"))
}

fn select_layer_recovery_output_info<'a>(
    infos: &'a [OutputInfo],
    current_output_name: Option<&str>,
) -> Option<(&'a OutputInfo, &'static str)> {
    if let Some(current) = current_output_name {
        if let Some(info) = infos.iter().find(|info| info.name == current) {
            return Some((info, "keep-assignment"));
        }
    }
    if let Some(info) = infos.iter().find(|info| info.primary) {
        return Some((info, "fallback-primary"));
    }
    infos.first().map(|info| (info, "fallback-first"))
}

impl MeridianState {
    pub fn reconcile_layer_shell_outputs_after_output_change(
        &mut self,
        action: &str,
        changed_output_name: Option<&str>,
    ) {
        if self.outputs.is_empty() {
            tracing::warn!(
                "no output available for layer-shell surface: action={} changed_output={:?}",
                action,
                changed_output_name
            );
            return;
        }

        if action == "output-removed" {
            let fallback = select_layer_recovery_output_info(self.output_registry.list(), None)
                .and_then(|(info, _)| {
                    self.outputs
                        .iter()
                        .find(|candidate| candidate.name() == info.name)
                        .map(|candidate| candidate.name())
                });
            tracing::debug!(
                "layer-shell output lost: removed_output={:?} fallback_output={:?}",
                changed_output_name,
                fallback
            );
            if let Some(fallback_name) = fallback {
                tracing::debug!(
                    "layer-shell reassigned to fallback output: output={}",
                    fallback_name
                );
            } else {
                tracing::warn!(
                    "no output available for layer-shell surface: action=output-removed removed_output={:?}",
                    changed_output_name
                );
            }
        }

        let outputs = self.outputs.clone();
        for output in &outputs {
            let current_name = output.name();
            let mut map = layer_map_for_output(output);
            map.arrange();
            self.mark_output_dirty_by_name(&current_name, "layer-shell-output-change-arrange");
            let layers: Vec<_> = map.layers().cloned().collect();
            if action == "output-reconfigured"
                && changed_output_name
                    .map(|changed| changed == current_name)
                    .unwrap_or(false)
            {
                tracing::debug!(
                    "layer-shell output reconfigured: output={} layers={}",
                    current_name,
                    layers.len()
                );
            }
            if action == "output-removed" {
                if let Some((target, reason)) = select_layer_recovery_output_info(
                    self.output_registry.list(),
                    Some(&current_name),
                ) {
                    if reason != "keep-assignment" {
                        tracing::debug!(
                            "layer-shell reassigned to fallback output: from={} to={} reason={}",
                            current_name,
                            target.name,
                            reason
                        );
                    }
                }
            }
            for layer in layers {
                layer.layer_surface().send_configure();
                self.mark_output_dirty_by_name(
                    &current_name,
                    "layer-shell-output-change-configure",
                );
            }
        }
    }
}

impl WlrLayerShellHandler for MeridianState {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        output: Option<WlOutput>,
        layer: WlrLayer,
        namespace: String,
    ) {
        let requested_output_name = output
            .as_ref()
            .and_then(Output::from_resource)
            .map(|output| output.name());
        tracing::info!(
            "New layer surface: namespace={}, layer={:?}, requested_output={:?}",
            namespace,
            layer,
            requested_output_name
        );

        let selected = select_layer_output_info(
            self.output_registry.list(),
            requested_output_name.as_deref(),
        );

        if requested_output_name.is_some()
            && selected
                .as_ref()
                .map(|(_info, reason)| {
                    *reason == "fallback-primary-unknown-requested"
                        || *reason == "fallback-first-unknown-requested"
                })
                .unwrap_or(false)
        {
            tracing::warn!(
                "layer surface requested output {:?} not found in output registry; applying fallback",
                requested_output_name
            );
        }

        let output = selected.and_then(|(info, reason)| {
            tracing::debug!(
                "layer surface selected output: id={} name={} fallback_reason={}",
                info.id.0,
                info.name,
                reason
            );
            self.outputs
                .iter()
                .find(|candidate| candidate.name() == info.name)
                .cloned()
        });

        let Some(output) = output else {
            tracing::warn!(
                "Closing layer surface namespace={} because no output is available (registry selection failed)",
                namespace
            );
            surface.send_close();
            return;
        };

        tracing::info!(
            "Mapping layer surface: namespace={}, output={}",
            namespace,
            output.name()
        );

        let layer = DesktopLayerSurface::new(surface, namespace);
        let map_result = {
            let mut map = layer_map_for_output(&output);
            map.map_layer(&layer)
        };

        if let Err(err) = map_result {
            tracing::warn!("failed to map layer surface: {}", err);
            layer.layer_surface().send_close();
        } else {
            self.mark_output_dirty_by_name(&output.name(), "layer-surface-mapped");
        }
    }

    fn new_popup(&mut self, _parent: WlrLayerSurface, popup: PopupSurface) {
        let _ = self.popups.track_popup(PopupKind::Xdg(popup));
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        let outputs = self.outputs.clone();
        for output in &outputs {
            let output_name = output.name();
            let mut map = layer_map_for_output(output);
            let layer = map
                .layers()
                .find(|layer| layer.layer_surface() == &surface)
                .cloned();

            if let Some(layer) = layer {
                map.unmap_layer(&layer);
                self.mark_output_dirty_by_name(&output_name, "layer-surface-destroyed");
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use smithay::utils::Transform;

    use crate::state::{OutputGeometry, OutputId, OutputInfo};

    use super::{select_layer_output_info, select_layer_recovery_output_info};

    fn info(id: u32, name: &str, primary: bool) -> OutputInfo {
        OutputInfo {
            id: OutputId(id),
            name: name.to_string(),
            geometry: OutputGeometry {
                x: 0,
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
    fn explicit_output_wins() {
        let infos = vec![info(1, "a", true), info(2, "b", false)];
        let (selected, reason) = select_layer_output_info(&infos, Some("b")).expect("selection");
        assert_eq!(selected.id.0, 2);
        assert_eq!(reason, "explicit-output");
    }

    #[test]
    fn unknown_requested_output_falls_back_to_primary() {
        let infos = vec![info(1, "a", true), info(2, "b", false)];
        let (selected, reason) =
            select_layer_output_info(&infos, Some("missing")).expect("selection");
        assert_eq!(selected.id.0, 1);
        assert_eq!(reason, "fallback-primary-unknown-requested");
    }

    #[test]
    fn primary_fallback_without_request() {
        let infos = vec![info(3, "primary", true), info(4, "other", false)];
        let (selected, reason) = select_layer_output_info(&infos, None).expect("selection");
        assert_eq!(selected.id.0, 3);
        assert_eq!(reason, "fallback-primary");
    }

    #[test]
    fn first_fallback_without_primary() {
        let infos = vec![info(8, "first", false), info(9, "second", false)];
        let (selected, reason) = select_layer_output_info(&infos, None).expect("selection");
        assert_eq!(selected.id.0, 8);
        assert_eq!(reason, "fallback-first");
    }

    #[test]
    fn empty_registry_is_safe() {
        assert!(select_layer_output_info(&[], None).is_none());
    }

    #[test]
    fn recovery_lost_output_falls_back_to_primary() {
        let infos = vec![info(1, "primary", true), info(2, "other", false)];
        let (selected, reason) =
            select_layer_recovery_output_info(&infos, Some("lost")).expect("selection");
        assert_eq!(selected.name, "primary");
        assert_eq!(reason, "fallback-primary");
    }

    #[test]
    fn recovery_lost_output_falls_back_to_first_without_primary() {
        let infos = vec![info(1, "first", false), info(2, "second", false)];
        let (selected, reason) =
            select_layer_recovery_output_info(&infos, Some("lost")).expect("selection");
        assert_eq!(selected.name, "first");
        assert_eq!(reason, "fallback-first");
    }

    #[test]
    fn recovery_no_outputs_is_safe_none() {
        assert!(select_layer_recovery_output_info(&[], Some("lost")).is_none());
    }

    #[test]
    fn recovery_reconfigure_keeps_same_output_assignment() {
        let infos = vec![info(1, "panel-out", true), info(2, "other", false)];
        let (selected, reason) =
            select_layer_recovery_output_info(&infos, Some("panel-out")).expect("selection");
        assert_eq!(selected.name, "panel-out");
        assert_eq!(reason, "keep-assignment");
    }
}
