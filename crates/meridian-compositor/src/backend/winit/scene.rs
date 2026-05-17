use smithay::{
    backend::renderer::{
        element::{surface::WaylandSurfaceRenderElement, AsRenderElements},
        gles::GlesRenderer,
    },
    desktop::{space::SpaceRenderElements, Window},
    output::Output,
    utils::Scale,
    wayland::seat::WaylandFocus,
};

use crate::{state::MeridianState, wallpaper::WallpaperGpuCache};

use super::{WinitRenderElements, WinitRenderScratch};

fn render_window_space_elements<C>(
    renderer: &mut GlesRenderer,
    window: &Window,
    window_loc: smithay::utils::Point<i32, smithay::utils::Logical>,
    scale: Scale<f64>,
    out: &mut Vec<C>,
) where
    C: From<SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>>,
{
    out.extend(
        window
            .render_elements::<WaylandSurfaceRenderElement<GlesRenderer>>(
                renderer,
                window_loc.to_physical_precise_round(scale),
                scale,
                1.0,
            )
            .into_iter()
            .map(SpaceRenderElements::from)
            .map(C::from),
    );
}

// Keep explicit render inputs to make frame wiring and ordering dependencies obvious.
// A context struct here would be mostly mechanical churn on a hot render path.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_elements_for_output(
    state: &mut MeridianState,
    renderer: &mut GlesRenderer,
    _output: &Output,
    wallpaper_cache: &mut Option<WallpaperGpuCache>,
    out_w: u32,
    out_h: u32,
    scratch: &mut WinitRenderScratch,
) {
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
    scratch.normal.clear();
    scratch.final_elements.clear();
    scratch.windows.clear();

    if state.lock_manager.is_locked_or_pending() {
        scratch.lower_layer_data.clear();
        scratch.upper_layer_data.clear();
        scratch.lower_layer_elements.clear();
        scratch.upper_layer_elements.clear();
        return;
    }

    scratch
        .windows
        .extend(state.workspaces.active_space().elements().cloned());

    for window in scratch.windows.iter().rev() {
        let loc = match state.workspaces.active_space().element_location(window) {
            Some(l) => l,
            None => continue,
        };
        let geometry = window.geometry();
        let render_loc =
            smithay::utils::Point::from((loc.x - geometry.loc.x, loc.y - geometry.loc.y));

        if let Some(wl_surf) = window.wl_surface().map(|s| s.into_owned()) {
            let metrics = state.decoration_manager.ssd_render_metrics(
                &wl_surf,
                loc,
                geometry.size,
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
            scratch.normal.extend(
                window_deco_elements
                    .into_iter()
                    .map(WinitRenderElements::Decoration),
            );
        }

        render_window_space_elements(renderer, window, render_loc, scale, &mut scratch.normal);
    }

    let wallpaper_elem = wallpaper_cache
        .as_ref()
        .map(WallpaperGpuCache::render_element);

    scratch.final_elements.extend(
        wallpaper_elem
            .into_iter()
            .map(WinitRenderElements::Wallpaper),
    );
    scratch
        .final_elements
        .append(&mut scratch.lower_layer_elements);
    scratch.final_elements.append(&mut scratch.normal);
    scratch
        .final_elements
        .append(&mut scratch.upper_layer_elements);
}
