use std::time::Duration;

use smithay::{
    backend::{
        renderer::{
            damage::OutputDamageTracker,
            element::surface::WaylandSurfaceRenderElement,
            gles::GlesRenderer,
        },
        winit::{self, WinitEvent},
    },
    desktop::space::render_output,
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::calloop::EventLoop,
    utils::{Rectangle, Transform},
};

use crate::state::MeridianState;

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

    state.workspaces.active_space_mut().map_output(&output, (0, 0));
    state.outputs.push(output.clone());

    let mut damage_tracker = OutputDamageTracker::from_output(&output);

    event_loop
        .handle()
        .insert_source(winit_event_loop, move |event, _, state| match event {
            WinitEvent::Resized { size, .. } => {
                output.change_current_state(
                    Some(Mode { size, refresh: 60_000 }),
                    None,
                    None,
                    None,
                );
            }
            WinitEvent::Redraw => {
                let size = backend.window_size();
                let damage = Rectangle::from_size(size);
                let age = backend.buffer_age().unwrap_or(0);

                {
                    let (renderer, mut framebuffer) = backend.bind().unwrap();
                    let bg = state.theme_manager.current().config.colors.background.as_f32_array();
                    render_output::<_, WaylandSurfaceRenderElement<GlesRenderer>, _, _>(
                        &output,
                        renderer,
                        &mut framebuffer,
                        1.0,
                        age,
                        [state.workspaces.active_space()],
                        &[],
                        &mut damage_tracker,
                        bg,
                    )
                    .unwrap();
                }

                backend.submit(Some(&[damage])).unwrap();

                let time = state.start_time.elapsed();
                state.workspaces.active_space().elements().for_each(|window| {
                    window.send_frame(
                        &output,
                        time,
                        Some(Duration::ZERO),
                        |_, _| Some(output.clone()),
                    );
                });

                state.workspaces.active_space_mut().refresh();
                state.popups.cleanup();
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
