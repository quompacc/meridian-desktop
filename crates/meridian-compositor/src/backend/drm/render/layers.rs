use std::time::Duration;

use smithay::backend::renderer::{
    element::{surface::render_elements_from_surface_tree, Kind},
    gles::GlesRenderer,
    utils::RendererSurfaceStateUserData,
};
use smithay::{
    desktop::{layer_map_for_output, LayerSurface},
    output::Output,
    utils::{Logical, Rectangle, Scale},
    wayland::{
        compositor::with_states,
        shell::wlr_layer::{Layer as WlrLayer, LayerSurfaceData},
    },
};
use tracing::{debug, warn};

use super::MeridianRenderElements;

#[derive(Debug)]
struct LayerRenderState {
    has_buffer: bool,
    has_view: bool,
    initial_configure_sent: bool,
    buffer_size: Option<String>,
    surface_size: Option<String>,
}

impl LayerRenderState {
    fn mapped(&self) -> bool {
        self.has_buffer && self.has_view
    }
}

fn layer_render_state(
    surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
) -> LayerRenderState {
    with_states(surface, |states| {
        let renderer_state = states.data_map.get::<RendererSurfaceStateUserData>();
        let (has_buffer, has_view, buffer_size, surface_size) =
            if let Some(renderer_state) = renderer_state {
                let renderer_state = renderer_state.lock().unwrap();
                (
                    renderer_state.buffer().is_some(),
                    renderer_state.view().is_some(),
                    renderer_state
                        .buffer_size()
                        .map(|size| format!("{:?}", size)),
                    renderer_state
                        .surface_size()
                        .map(|size| format!("{:?}", size)),
                )
            } else {
                (false, false, None, None)
            };

        let initial_configure_sent = states
            .data_map
            .get::<LayerSurfaceData>()
            .map(|data| data.lock().unwrap().initial_configure_sent)
            .unwrap_or(false);

        LayerRenderState {
            has_buffer,
            has_view,
            initial_configure_sent,
            buffer_size,
            surface_size,
        }
    })
}

pub(crate) type LayerRenderData = (LayerSurface, Rectangle<i32, Logical>);

fn is_upper_layer(namespace: &str, layer: WlrLayer) -> bool {
    namespace == "meridian-launcher" || matches!(layer, WlrLayer::Top | WlrLayer::Overlay)
}

pub(super) fn collect_layer_data(
    output: &Output,
    lower: &mut Vec<LayerRenderData>,
    upper: &mut Vec<LayerRenderData>,
) {
    let layer_map = layer_map_for_output(output);
    let layer_count = layer_map.len();
    debug!(
        "Layer map: output={} surfaces={}",
        output.name(),
        layer_count
    );

    lower.clear();
    upper.clear();
    for layer_surface in layer_map.layers() {
        let render_state = layer_render_state(layer_surface.wl_surface());
        let geo = match layer_map.layer_geometry(layer_surface) {
            Some(g) => Some(g),
            None => {
                warn!(
                    "Layer surface without geometry: namespace={} layer={:?} mapped={}",
                    layer_surface.namespace(),
                    layer_surface.layer(),
                    render_state.mapped()
                );
                None
            }
        };
        debug!(
            "Layer surface: namespace={} layer={:?} mapped={} has_buffer={} has_view={} initial_configure_sent={} geometry={:?} buffer_size={:?} surface_size={:?}",
            layer_surface.namespace(),
            layer_surface.layer(),
            render_state.mapped(),
            render_state.has_buffer,
            render_state.has_view,
            render_state.initial_configure_sent,
            geo,
            render_state.buffer_size,
            render_state.surface_size
        );
        let Some(geo) = geo else {
            continue;
        };

        let layer = layer_surface.layer();
        if is_upper_layer(layer_surface.namespace(), layer) {
            if layer_surface.namespace() == "meridian-launcher"
                && matches!(layer, WlrLayer::Background | WlrLayer::Bottom)
            {
                debug!(
                    "launcher layer bucket override: namespace={} reported_layer={:?} bucket=upper",
                    layer_surface.namespace(),
                    layer
                );
            }
            upper.push((layer_surface.clone(), geo));
        } else {
            lower.push((layer_surface.clone(), geo));
        }
    }
    // Render Overlay surfaces before Top so they appear above the panel.
    // Lower index in the render elements slice = drawn on top (first element wins in DRM z-order).
    upper.sort_by_key(|(s, _)| match s.layer() {
        WlrLayer::Overlay => 0u8,
        _ => 1,
    });
}

pub(super) fn render_layer_elements(
    renderer: &mut GlesRenderer,
    layer_data: &[LayerRenderData],
    scale: Scale<f64>,
    out: &mut Vec<MeridianRenderElements>,
) {
    out.clear();
    for (layer, geo) in layer_data {
        let loc = geo.loc.to_f64().to_physical(scale).to_i32_round();
        let layer_elements =
            render_elements_from_surface_tree::<GlesRenderer, MeridianRenderElements>(
                renderer,
                layer.wl_surface(),
                loc,
                scale,
                1.0,
                Kind::Unspecified,
            );
        debug!(
            "Layer render elements: namespace={} layer={:?} elements={}",
            layer.namespace(),
            layer.layer(),
            layer_elements.len()
        );
        out.extend(layer_elements);
    }
}

pub(super) fn send_layer_frames(
    output: &Output,
    time: Duration,
    lower_layer_data: &[LayerRenderData],
    upper_layer_data: &[LayerRenderData],
) {
    for (layer, _) in lower_layer_data.iter().chain(upper_layer_data.iter()) {
        layer.send_frame(output, time, Some(Duration::ZERO), |_, _| {
            Some(output.clone())
        });
    }
}

#[cfg(test)]
mod tests {
    use smithay::wayland::shell::wlr_layer::Layer as WlrLayer;

    use super::is_upper_layer;

    #[test]
    fn launcher_namespace_forces_upper_bucket() {
        assert!(is_upper_layer("meridian-launcher", WlrLayer::Background));
        assert!(is_upper_layer("meridian-launcher", WlrLayer::Bottom));
    }

    #[test]
    fn non_launcher_uses_layer_role() {
        assert!(is_upper_layer("other", WlrLayer::Top));
        assert!(is_upper_layer("other", WlrLayer::Overlay));
        assert!(!is_upper_layer("other", WlrLayer::Background));
        assert!(!is_upper_layer("other", WlrLayer::Bottom));
    }
}
