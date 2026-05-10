use std::time::Duration;

use smithay::backend::renderer::{
    element::{surface::render_elements_from_surface_tree, Kind},
    gles::GlesRenderer,
};
use smithay::{
    desktop::{layer_map_for_output, LayerSurface},
    output::Output,
    utils::{Logical, Rectangle, Scale},
    wayland::shell::wlr_layer::Layer as WlrLayer,
};

use super::WinitRenderElements;

pub(super) type LayerRenderData = (LayerSurface, Rectangle<i32, Logical>);

pub(super) fn collect_layer_data(output: &Output) -> (Vec<LayerRenderData>, Vec<LayerRenderData>) {
    let layer_map = layer_map_for_output(output);
    let mut lower = Vec::new();
    let mut upper = Vec::new();

    for layer_surface in layer_map.layers() {
        let geo = match layer_map.layer_geometry(layer_surface) {
            Some(geo) => geo,
            None => continue,
        };
        match layer_surface.layer() {
            WlrLayer::Background | WlrLayer::Bottom => lower.push((layer_surface.clone(), geo)),
            WlrLayer::Top | WlrLayer::Overlay => upper.push((layer_surface.clone(), geo)),
        }
    }

    (lower, upper)
}

pub(super) fn render_layer_elements(
    renderer: &mut GlesRenderer,
    layer_data: &[LayerRenderData],
    scale: Scale<f64>,
) -> Vec<WinitRenderElements> {
    layer_data
        .iter()
        .flat_map(|(layer, geo)| {
            let loc = geo.loc.to_f64().to_physical(scale).to_i32_round();
            render_elements_from_surface_tree::<GlesRenderer, WinitRenderElements>(
                renderer,
                layer.wl_surface(),
                loc,
                scale,
                1.0,
                Kind::Unspecified,
            )
        })
        .collect()
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
