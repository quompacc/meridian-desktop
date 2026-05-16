use meridian_wm::WorkspaceMode;
use smithay::{
    backend::renderer::utils::{on_commit_buffer_handler, RendererSurfaceStateUserData},
    desktop::{layer_map_for_output, WindowSurfaceType},
    reexports::wayland_server::{
        protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface},
        Client,
    },
    utils::SERIAL_COUNTER,
    wayland::{
        buffer::BufferHandler,
        compositor::{
            get_parent, is_sync_subsurface, with_states, CompositorHandler, CompositorState,
        },
        seat::WaylandFocus,
        shell::wlr_layer::{KeyboardInteractivity, Layer as WlrLayer, LayerSurfaceData},
    },
};

use crate::protocols::xdg_shell::handle_commit;

use super::super::super::{client_compositor_state, MeridianState};

impl BufferHandler for MeridianState {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl CompositorHandler for MeridianState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(
        &self,
        client: &'a Client,
    ) -> &'a smithay::wayland::compositor::CompositorClientState {
        client_compositor_state(client)
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        self.mark_all_outputs_dirty("surface-commit");

        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(window) = self.workspaces.active_space().elements().find(|window| {
                window
                    .wl_surface()
                    .is_some_and(|wl_surface| *wl_surface == root)
            }) {
                window.on_commit();
            }
        }

        handle_commit(&mut self.popups, self.workspaces.active_space(), surface);
        crate::grabs::resize_grab::handle_commit(self.workspaces.active_space_mut(), surface);

        if let Some(output) = self
            .outputs
            .iter()
            .find(|output| {
                let map = layer_map_for_output(output);
                map.layer_for_surface(surface, WindowSurfaceType::ALL)
                    .is_some()
            })
            .cloned()
        {
            let output_name = output.name();
            let initial_configure_sent = with_states(surface, |states| {
                states
                    .data_map
                    .get::<LayerSurfaceData>()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .initial_configure_sent
            });

            if initial_configure_sent {
                tracing::trace!(
                    "Layer surface commit: output={}, initial_configure_sent={}",
                    output_name,
                    initial_configure_sent
                );
            } else {
                tracing::debug!(
                    "Layer surface commit: output={}, initial_configure_sent={}",
                    output_name,
                    initial_configure_sent
                );
            }

            let mut map = layer_map_for_output(&output);
            map.arrange();
            self.mark_output_dirty_by_name(&output_name, "layer-surface-commit");
            let focus_target =
                map.layer_for_surface(surface, WindowSurfaceType::ALL)
                    .map(|layer| {
                        let cached = layer.cached_state();
                        let layer_geometry = map.layer_geometry(layer);
                        let has_buffer = with_states(surface, |states| {
                            states
                                .data_map
                                .get::<RendererSurfaceStateUserData>()
                                .map(|renderer_state| {
                                    renderer_state.lock().unwrap().buffer().is_some()
                                })
                                .unwrap_or(false)
                        });
                        (
                            layer.namespace().to_string(),
                            layer.layer(),
                            cached.keyboard_interactivity,
                            cached.anchor,
                            cached.margin,
                            cached.exclusive_zone,
                            cached.size,
                            layer_geometry.map(|geo| format!("{:?}", geo)),
                            has_buffer,
                        )
                    });

            if let Some((
                namespace,
                layer_kind,
                keyboard_interactivity,
                anchor,
                margin,
                exclusive_zone,
                requested_size,
                layer_geometry,
                has_buffer,
            )) = focus_target
            {
                let wants_focus = namespace == "meridian-launcher"
                    && keyboard_interactivity != KeyboardInteractivity::None
                    && has_buffer;
                if namespace == "meridian-launcher" {
                    tracing::debug!(
                        "launcher layer cached state: output={} layer={:?} anchor={:?} margin={:?} exclusive_zone={:?} requested_size={:?} geometry={:?} keyboard_interactivity={:?} has_buffer={}",
                        output_name,
                        layer_kind,
                        anchor,
                        margin,
                        exclusive_zone,
                        requested_size,
                        layer_geometry,
                        keyboard_interactivity,
                        has_buffer
                    );
                }
                if wants_focus && has_buffer {
                    if matches!(layer_kind, WlrLayer::Background) {
                        tracing::debug!(
                            "launcher focus using namespace fallback because cached layer is Background: namespace={} output={} keyboard_interactivity={:?}",
                            namespace,
                            output_name,
                            keyboard_interactivity
                        );
                    }
                    tracing::debug!(
                        "layer keyboard focus requested: namespace={} layer={:?} output={} keyboard_interactivity={:?}",
                        namespace,
                        layer_kind,
                        output_name,
                        keyboard_interactivity
                    );
                    let should_set_focus = self
                        .seat
                        .get_keyboard()
                        .map(|keyboard| keyboard.current_focus().as_ref() != Some(surface))
                        .unwrap_or(false);
                    if should_set_focus {
                        let serial = SERIAL_COUNTER.next_serial();
                        self.set_keyboard_focus_with_decorations(Some(surface.clone()), serial);
                        tracing::debug!(
                            "layer keyboard focus set: namespace={} layer={:?} output={}",
                            namespace,
                            layer_kind,
                            output_name
                        );
                    }
                } else if namespace == "meridian-launcher" && !has_buffer {
                    let should_clear_focus = self
                        .seat
                        .get_keyboard()
                        .map(|keyboard| keyboard.current_focus().as_ref() == Some(surface))
                        .unwrap_or(false);
                    if should_clear_focus {
                        let serial = SERIAL_COUNTER.next_serial();
                        self.set_keyboard_focus_with_decorations(Option::<WlSurface>::None, serial);
                        self.broadcast_toplevel_focus_cleared();
                        tracing::debug!(
                            "layer keyboard focus cleared: namespace={} layer={:?} output={} reason=no-buffer-commit",
                            namespace,
                            layer_kind,
                            output_name
                        );
                    }
                } else if namespace == "meridian-launcher" {
                    tracing::debug!(
                        "layer keyboard focus skipped: namespace={} layer={:?} output={} keyboard_interactivity={:?} has_buffer={} wants_focus={}",
                        namespace,
                        layer_kind,
                        output_name,
                        keyboard_interactivity,
                        has_buffer,
                        wants_focus
                    );
                }
            }

            if !initial_configure_sent {
                if let Some(layer) = map.layer_for_surface(surface, WindowSurfaceType::ALL) {
                    tracing::info!("Sending layer surface configure: output={}", output_name);
                    layer.layer_surface().send_configure();
                    self.mark_output_dirty_by_name(&output_name, "layer-surface-configure");
                } else {
                    tracing::warn!(
                        "Layer surface commit matched output={} but layer_for_surface returned None",
                        output_name
                    );
                }
            }
        }

        let active = self.workspaces.active;
        if self.wm_workspaces[active].mode == WorkspaceMode::Tiling {
            self.tile_workspace(active);
        }
    }
}
