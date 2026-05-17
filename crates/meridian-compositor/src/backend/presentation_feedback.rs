use std::time::Duration;

use smithay::{
    backend::renderer::element::{default_primary_scanout_output_compare, RenderElementStates},
    desktop::{
        layer_map_for_output, utils::surface_presentation_feedback_flags_from_states,
        utils::surface_primary_scanout_output, utils::update_surface_primary_scanout_output,
        utils::OutputPresentationFeedback, Space, Window,
    },
    output::Output,
    reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback,
    utils::{Clock, Monotonic, Time},
    wayland::presentation::Refresh,
};

pub(crate) fn update_primary_scanout_output_for_output(
    output: &Output,
    space: &Space<Window>,
    render_element_states: &RenderElementStates,
) {
    space.elements().for_each(|window| {
        if !space.outputs_for_element(window).contains(output) {
            return;
        }

        window.with_surfaces(|surface, states| {
            update_surface_primary_scanout_output(
                surface,
                output,
                states,
                None,
                render_element_states,
                default_primary_scanout_output_compare,
            );
        });
    });

    let map = layer_map_for_output(output);
    for layer_surface in map.layers() {
        layer_surface.with_surfaces(|surface, states| {
            update_surface_primary_scanout_output(
                surface,
                output,
                states,
                None,
                render_element_states,
                default_primary_scanout_output_compare,
            );
        });
    }
}

pub(crate) fn take_presentation_feedback_for_output(
    output: &Output,
    space: &Space<Window>,
    render_element_states: &RenderElementStates,
) -> OutputPresentationFeedback {
    let mut feedback = OutputPresentationFeedback::new(output);

    for window in space.elements() {
        if space.outputs_for_element(window).contains(output) {
            window.take_presentation_feedback(
                &mut feedback,
                surface_primary_scanout_output,
                |surface, _| {
                    surface_presentation_feedback_flags_from_states(
                        surface,
                        None,
                        render_element_states,
                    )
                },
            );
        }
    }

    let map = layer_map_for_output(output);
    for layer_surface in map.layers() {
        layer_surface.take_presentation_feedback(
            &mut feedback,
            surface_primary_scanout_output,
            |surface, _| {
                surface_presentation_feedback_flags_from_states(
                    surface,
                    None,
                    render_element_states,
                )
            },
        );
    }

    feedback
}

pub(crate) fn present_feedback_on_vblank(
    mut feedback: OutputPresentationFeedback,
    output: &Output,
    seq: u64,
) {
    feedback.presented(
        Clock::<Monotonic>::new().now(),
        output_refresh(output),
        seq,
        wp_presentation_feedback::Kind::Vsync,
    );
}

pub(crate) fn monotonic_now() -> Time<Monotonic> {
    Clock::<Monotonic>::new().now()
}

pub(crate) fn output_refresh(output: &Output) -> Refresh {
    output
        .current_mode()
        .and_then(|mode| duration_from_refresh_millihz(mode.refresh))
        .map(Refresh::fixed)
        .unwrap_or(Refresh::fixed(Duration::from_millis(16)))
}

fn duration_from_refresh_millihz(refresh_millihz: i32) -> Option<Duration> {
    if refresh_millihz <= 0 {
        return None;
    }

    let nanos_per_frame = 1_000_000_000_000u64 / refresh_millihz as u64;
    Some(Duration::from_nanos(nanos_per_frame))
}
