use smithay::{
    backend::renderer::gles::GlesRenderer,
    desktop::{space::space_render_elements, Space, Window},
    output::Output,
    utils::Scale,
    wayland::seat::WaylandFocus,
};

use crate::{state::MeridianState, wallpaper::WallpaperGpuCache};

use super::{layers::render_layer_elements, layers::LayerRenderData, WinitRenderElements};

fn render_window_space_elements(
    renderer: &mut GlesRenderer,
    output: &Output,
    window: &Window,
    window_loc: smithay::utils::Point<i32, smithay::utils::Logical>,
) -> Vec<
    smithay::desktop::space::SpaceRenderElements<
        GlesRenderer,
        smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement<GlesRenderer>,
    >,
> {
    let mut window_space = Space::<Window>::default();
    window_space.map_output(output, (0, 0));
    window_space.map_element(window.clone(), window_loc, false);
    space_render_elements::<GlesRenderer, Window, _>(renderer, [&window_space], output, 1.0)
        .unwrap_or_default()
}

// Keep explicit render inputs to make frame wiring and ordering dependencies obvious.
// A context struct here would be mostly mechanical churn on a hot render path.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_elements_for_output(
    state: &mut MeridianState,
    renderer: &mut GlesRenderer,
    output: &Output,
    lower_layer_data: &[LayerRenderData],
    upper_layer_data: &[LayerRenderData],
    wallpaper_cache: &mut Option<WallpaperGpuCache>,
    out_w: u32,
    out_h: u32,
) -> Vec<WinitRenderElements> {
    state
        .wallpaper_manager
        .apply_theme(state.theme_manager.current());

    WallpaperGpuCache::update(
        renderer,
        wallpaper_cache,
        &mut state.wallpaper_manager,
        out_w,
        out_h,
    );

    let theme = &state.theme_manager.current().config;
    let scale = Scale::from(1.0f64);
    let mut normal_window_elements: Vec<WinitRenderElements> = Vec::new();
    for window in state
        .workspaces
        .active_space()
        .elements()
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        let loc = match state.workspaces.active_space().element_location(&window) {
            Some(l) => l,
            None => continue,
        };

        if let Some(wl_surf) = window.wl_surface().map(|s| s.into_owned()) {
            let geo = window.geometry();
            let metrics = state.decoration_manager.ssd_render_metrics(
                &wl_surf,
                loc,
                geo.size,
                &theme.decorations,
            );
            let window_deco_elements = state.decoration_manager.render_elements(
                &wl_surf,
                metrics.frame_origin,
                metrics.client_size,
                &theme.decorations,
                &theme.colors,
                scale,
            );
            normal_window_elements.extend(
                window_deco_elements
                    .into_iter()
                    .map(WinitRenderElements::Decoration),
            );
        }

        normal_window_elements.extend(
            render_window_space_elements(renderer, output, &window, loc)
                .into_iter()
                .map(WinitRenderElements::Space),
        );
    }

    let lower_layer_elems = render_layer_elements(renderer, lower_layer_data, scale);
    let upper_layer_elems = render_layer_elements(renderer, upper_layer_data, scale);
    let wallpaper_elem = wallpaper_cache
        .as_ref()
        .map(WallpaperGpuCache::render_element);

    wallpaper_elem
        .into_iter()
        .map(WinitRenderElements::Wallpaper)
        .chain(lower_layer_elems)
        .chain(normal_window_elements)
        .chain(upper_layer_elems)
        .collect()
}
