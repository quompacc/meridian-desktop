use std::time::Duration;

use smithay::{
    backend::{
        renderer::{
            damage::OutputDamageTracker,
            element::{
                render_elements, solid::SolidColorRenderElement,
                surface::WaylandSurfaceRenderElement, texture::TextureRenderElement,
            },
            gles::{GlesRenderer, GlesTexture},
        },
        winit::{self, WinitEvent},
    },
    desktop::{layer_map_for_output, space::render_output, space::SpaceRenderElements, Window},
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::calloop::EventLoop,
    utils::{Rectangle, Scale, Transform},
};

use crate::{
    state::{MeridianState, OutputReconfigure, OutputRegistration},
    wallpaper::WallpaperGpuCache,
};

mod layers;
mod scene;

use layers::{collect_layer_data, render_layer_elements, send_layer_frames};

render_elements! {
    pub WinitRenderElements<=GlesRenderer>;
    Space=SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>,
    Decoration=SolidColorRenderElement,
    Wallpaper=TextureRenderElement<GlesTexture>,
    Layer=WaylandSurfaceRenderElement<GlesRenderer>,
}

#[derive(Default)]
pub(super) struct WinitRenderScratch {
    pub normal: Vec<WinitRenderElements>,
    pub final_elements: Vec<WinitRenderElements>,
    pub windows: Vec<Window>,
    pub lower_layer_data: Vec<layers::LayerRenderData>,
    pub upper_layer_data: Vec<layers::LayerRenderData>,
    pub lower_layer_elements: Vec<WinitRenderElements>,
    pub upper_layer_elements: Vec<WinitRenderElements>,
}

pub fn init_winit(
    event_loop: &mut EventLoop<MeridianState>,
    state: &mut MeridianState,
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut backend, winit_event_loop) = winit::init::<GlesRenderer>()?;

    let mode = Mode {
        size: backend.window_size(),
        refresh: 60_000,
    };

    let output = Output::new(
        "winit".to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Meridian".into(),
            model: "Winit".into(),
            serial_number: "Unknown".into(),
        },
    );
    let _global = output.create_global::<MeridianState>(&state.display_handle);
    output.change_current_state(
        Some(mode),
        Some(Transform::Flipped180),
        None,
        Some((0, 0).into()),
    );
    output.set_preferred(mode);

    state
        .workspaces
        .active_space_mut()
        .map_output(&output, (0, 0));
    state.outputs.push(output.clone());
    let output_id = state.register_output_info(OutputRegistration {
        name: output.name(),
        geometry: MeridianState::output_geometry_for_registry(0, 0, mode.size.w, mode.size.h),
        scale: 1.0,
        transform: Transform::Flipped180,
        refresh_millihz: Some(mode.refresh),
    });

    let mut damage_tracker = OutputDamageTracker::from_output(&output);
    let mut wallpaper_cache: Option<WallpaperGpuCache> = None;
    let mut render_scratch = WinitRenderScratch::default();

    event_loop
        .handle()
        .insert_source(winit_event_loop, move |event, _, state| match event {
            WinitEvent::Resized { size, .. } => {
                output.change_current_state(
                    Some(Mode {
                        size,
                        refresh: 60_000,
                    }),
                    None,
                    None,
                    None,
                );
                tracing::debug!(
                    "winit output resized: output_id={} output_name={} width={} height={} refresh={}",
                    output_id.0,
                    output.name(),
                    size.w,
                    size.h,
                    60_000
                );
                let reconfigured = state.handle_output_reconfigured(
                    output_id,
                    OutputReconfigure {
                        geometry: MeridianState::output_geometry_for_registry(0, 0, size.w, size.h),
                        scale: 1.0,
                        transform: Transform::Flipped180,
                        refresh_millihz: Some(60_000),
                        primary: None,
                    },
                );
                if !reconfigured {
                    tracing::warn!(
                        "winit output reconfigure failed for output_id={}, falling back to upsert by name",
                        output_id.0
                    );
                    state.register_output_info(OutputRegistration {
                        name: output.name(),
                        geometry: MeridianState::output_geometry_for_registry(0, 0, size.w, size.h),
                        scale: 1.0,
                        transform: Transform::Flipped180,
                        refresh_millihz: Some(60_000),
                    });
                }
            }
            WinitEvent::Redraw => {
                let size = backend.window_size();
                let damage = Rectangle::from_size(size);
                let age = backend.buffer_age().unwrap_or(0);
                collect_layer_data(
                    &output,
                    &mut render_scratch.lower_layer_data,
                    &mut render_scratch.upper_layer_data,
                );

                {
                    let (renderer, mut framebuffer) = backend.bind().unwrap();
                    let scale = Scale::from(1.0f64);
                    render_layer_elements(
                        renderer,
                        &render_scratch.lower_layer_data,
                        scale,
                        &mut render_scratch.lower_layer_elements,
                    );
                    render_layer_elements(
                        renderer,
                        &render_scratch.upper_layer_data,
                        scale,
                        &mut render_scratch.upper_layer_elements,
                    );
                    scene::render_elements_for_output(
                        state,
                        renderer,
                        &output,
                        &mut wallpaper_cache,
                        size.w as u32,
                        size.h as u32,
                        &mut render_scratch,
                    );

                    let bg = [0.0_f32; 4];
                    render_output::<_, WinitRenderElements, Window, _>(
                        &output,
                        renderer,
                        &mut framebuffer,
                        1.0,
                        age,
                        std::iter::empty::<&smithay::desktop::Space<Window>>(),
                        &render_scratch.final_elements,
                        &mut damage_tracker,
                        bg,
                    )
                    .unwrap();
                }

                backend.submit(Some(&[damage])).unwrap();

                let time = state.start_time.elapsed();
                state
                    .workspaces
                    .active_space()
                    .elements()
                    .for_each(|window| {
                        window.send_frame(&output, time, Some(Duration::ZERO), |_, _| {
                            Some(output.clone())
                        });
                    });
                send_layer_frames(
                    &output,
                    time,
                    &render_scratch.lower_layer_data,
                    &render_scratch.upper_layer_data,
                );

                state.workspaces.active_space_mut().refresh();
                state.popups.cleanup();
                layer_map_for_output(&output).cleanup();
                let _ = state.display_handle.flush_clients();
                backend.window().request_redraw();
            }
            WinitEvent::Input(event) => state.process_input_event(event),
            WinitEvent::CloseRequested => {
                state.loop_signal.stop();
            }
            _ => {}
        })?;

    Ok(())
}
