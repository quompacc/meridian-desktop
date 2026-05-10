use smithay::{
    backend::renderer::{element::solid::SolidColorRenderElement, gles::GlesRenderer},
    desktop::{space::space_render_elements, Window},
    output::Output,
    utils::Scale,
    wayland::seat::WaylandFocus,
};

use crate::{state::MeridianState, wallpaper::WallpaperGpuCache};

use super::{layers::render_layer_elements, layers::LayerRenderData, WinitRenderElements};

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

    let space_elems = space_render_elements::<GlesRenderer, Window, _>(
        renderer,
        [state.workspaces.active_space()],
        output,
        1.0,
    )
    .unwrap_or_default();

    let theme = &state.theme_manager.current().config;
    let scale = Scale::from(1.0f64);
    let mut deco_elements: Vec<SolidColorRenderElement> = Vec::new();
    for window in state
        .workspaces
        .active_space()
        .elements()
        .cloned()
        .collect::<Vec<_>>()
    {
        let wl_surf = match window.wl_surface().map(|s| s.into_owned()) {
            Some(s) => s,
            None => continue,
        };
        let loc = match state.workspaces.active_space().element_location(&window) {
            Some(l) => l,
            None => continue,
        };
        let geo = window.geometry();
        deco_elements.extend(state.decoration_manager.render_elements(
            &wl_surf,
            loc,
            geo.size,
            &theme.decorations,
            &theme.colors,
            scale,
        ));
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
        .chain(space_elems.into_iter().map(WinitRenderElements::Space))
        .chain(
            deco_elements
                .into_iter()
                .map(WinitRenderElements::Decoration),
        )
        .chain(upper_layer_elems)
        .collect()
}
