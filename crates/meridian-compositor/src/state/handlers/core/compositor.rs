use meridian_wm::WorkspaceMode;
use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    desktop::{layer_map_for_output, WindowSurfaceType},
    reexports::wayland_server::{
        protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface},
        Client,
    },
    wayland::{
        buffer::BufferHandler,
        compositor::{
            get_parent, is_sync_subsurface, with_states, CompositorHandler, CompositorState,
        },
        seat::WaylandFocus,
        shell::wlr_layer::LayerSurfaceData,
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
                    .map_or(false, |wl_surface| *wl_surface == root)
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
                map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
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

            if !initial_configure_sent {
                if let Some(layer) = map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL) {
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
