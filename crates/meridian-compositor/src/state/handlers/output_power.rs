use std::sync::Mutex;

use smithay::reexports::wayland_server::{
    backend::ClientId, Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use wayland_protocols_wlr::output_power_management::v1::server::{
    zwlr_output_power_manager_v1::{self, ZwlrOutputPowerManagerV1},
    zwlr_output_power_v1::{self, ZwlrOutputPowerV1},
};

use crate::state::{MeridianState, OutputPowerMode};

#[derive(Debug)]
pub struct OutputPowerData {
    pub output_name: Mutex<Option<String>>,
}

impl GlobalDispatch<ZwlrOutputPowerManagerV1, ()> for MeridianState {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<ZwlrOutputPowerManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ZwlrOutputPowerManagerV1, ()> for MeridianState {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &ZwlrOutputPowerManagerV1,
        request: zwlr_output_power_manager_v1::Request,
        _data: &(),
        _handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwlr_output_power_manager_v1::Request::GetOutputPower { id, output } => {
                let output_name = state
                    .outputs
                    .iter()
                    .find(|o| o.owns(&output))
                    .map(|o| o.name());

                let data = OutputPowerData {
                    output_name: Mutex::new(output_name.clone()),
                };
                let power = data_init.init(id, data);

                let Some(name) = output_name else {
                    tracing::warn!(
                        "wlr-output-power: get_output_power for unknown WlOutput -> failed"
                    );
                    power.failed();
                    return;
                };

                let mode = state.output_power_manager.mode_for(&name);
                state
                    .output_power_resources
                    .entry(name.clone())
                    .or_default()
                    .push(power.clone());
                power.mode(power_mode_to_wire(mode));
                tracing::debug!(
                    "wlr-output-power: bound power object for output={} mode={:?}",
                    name,
                    mode
                );
            }
            zwlr_output_power_manager_v1::Request::Destroy => {}
            _ => {}
        }
    }
}

impl Dispatch<ZwlrOutputPowerV1, OutputPowerData> for MeridianState {
    fn request(
        state: &mut Self,
        _client: &Client,
        resource: &ZwlrOutputPowerV1,
        request: zwlr_output_power_v1::Request,
        data: &OutputPowerData,
        _handle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwlr_output_power_v1::Request::SetMode { mode } => {
                let Some(output_name) = data.output_name.lock().unwrap().clone() else {
                    tracing::warn!("wlr-output-power: set_mode on already-failed object -> ignore");
                    return;
                };

                let Some(new_mode) = mode.into_result().ok().and_then(power_mode_from_wire) else {
                    resource.post_error(
                        zwlr_output_power_v1::Error::InvalidMode,
                        "invalid power mode value".to_string(),
                    );
                    return;
                };

                let known = state
                    .output_registry
                    .list()
                    .iter()
                    .map(|info| info.name.clone())
                    .collect::<Vec<_>>();
                let projected =
                    state
                        .output_power_manager
                        .projected_on_count(&known, &output_name, new_mode);
                if projected == 0 {
                    tracing::warn!(
                        "wlr-output-power: rejecting set_mode({:?}) for {} - would leave zero outputs On (VM self-lockout guard)",
                        new_mode,
                        output_name
                    );
                    resource.failed();
                    if let Some(resources) = state.output_power_resources.get_mut(&output_name) {
                        resources.retain(|entry| entry.id() != resource.id());
                    }
                    let remove_bucket = state
                        .output_power_resources
                        .get(&output_name)
                        .is_some_and(|resources| resources.is_empty());
                    if remove_bucket {
                        state.output_power_resources.remove(&output_name);
                    }
                    *data.output_name.lock().unwrap() = None;
                    return;
                }

                let changed = state.output_power_manager.set_mode(&output_name, new_mode);
                if let Some(resources) = state.output_power_resources.get(&output_name) {
                    for resource in resources {
                        resource.mode(power_mode_to_wire(new_mode));
                    }
                }

                if changed {
                    tracing::info!(
                        "wlr-output-power: output={} mode set to {:?} (3a: no DRM effect yet, will come in 3b)",
                        output_name,
                        new_mode
                    );
                } else {
                    tracing::debug!(
                        "wlr-output-power: output={} mode set to {:?} (no change)",
                        output_name,
                        new_mode
                    );
                }
                if changed && matches!(new_mode, OutputPowerMode::On) {
                    state.mark_all_outputs_dirty("output-power-on");
                    tracing::debug!(
                        "wlr-output-power: output {} marked dirty after power-on",
                        output_name
                    );
                }
            }
            zwlr_output_power_v1::Request::Destroy => {}
            _ => {}
        }
    }

    fn destroyed(
        state: &mut Self,
        _client: ClientId,
        resource: &ZwlrOutputPowerV1,
        data: &OutputPowerData,
    ) {
        let Some(name) = data.output_name.lock().unwrap().clone() else {
            return;
        };
        let Some(list) = state.output_power_resources.get_mut(&name) else {
            return;
        };
        list.retain(|entry| entry.id() != resource.id());
        if list.is_empty() {
            state.output_power_resources.remove(&name);
        }
    }
}

fn power_mode_to_wire(mode: OutputPowerMode) -> zwlr_output_power_v1::Mode {
    match mode {
        OutputPowerMode::On => zwlr_output_power_v1::Mode::On,
        OutputPowerMode::Off => zwlr_output_power_v1::Mode::Off,
    }
}

fn power_mode_from_wire(mode: zwlr_output_power_v1::Mode) -> Option<OutputPowerMode> {
    match mode {
        zwlr_output_power_v1::Mode::On => Some(OutputPowerMode::On),
        zwlr_output_power_v1::Mode::Off => Some(OutputPowerMode::Off),
        _ => None,
    }
}
