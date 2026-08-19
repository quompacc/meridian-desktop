use std::{
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};

use meridian_config::OutputModeConfig;
use smithay::{
    backend::{
        allocator::gbm::GbmAllocator,
        drm::{compositor::DrmCompositor, exporter::gbm::GbmFramebufferExporter, DrmDeviceFd},
        renderer::{element::memory::MemoryRenderBuffer, gles::GlesRenderer},
    },
    desktop::Window,
    output::Output,
    reexports::drm::control::{connector, crtc, Mode},
    utils::{Logical, Point},
};

use crate::state::{OutputGeometry, OutputId};

use crate::{cursor::CursorImage, wallpaper::WallpaperGpuCache};

pub(crate) mod glass;
mod gpu;
pub(crate) mod init;
mod init_diagnostics;
pub(crate) mod init_env;
pub(super) mod login_ipc;
pub(crate) mod mode_selection;
#[cfg(target_os = "openbsd")]
mod openbsd_privsep;
mod render;
#[cfg(target_os = "openbsd")]
mod wscons;

pub use init::init_drm;
pub(crate) use render::render_outputs_from_idle;
pub use render::{layer_role, render_stack_order, RenderStackRole};

pub type GbmDrmCompositor =
    DrmCompositor<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RenderPassMetrics {
    pub rendered_frames: u64,
    pub empty_frames: u64,
    pub outputs_processed: u64,
    pub outputs_skipped_clean: u64,
    pub outputs_skipped_in_flight: u64,
    pub outputs_skipped_power_off: u64,
    pub queued_frames: u64,
    pub queue_failures: u64,
    pub rendered_outputs_with_layers: u64,
    pub rendered_outputs_with_space: u64,
    pub rendered_outputs_with_layers_only: u64,
    pub render_elements: u64,
    pub layer_surfaces: u64,
    pub output_pass_duration: Duration,
    pub wallpaper_duration: Duration,
    pub scene_compose_duration: Duration,
    pub capture_duration: Duration,
    pub glass_duration: Duration,
    pub render_frame_duration: Duration,
    pub frame_feedback_duration: Duration,
    pub queue_duration: Duration,
}

include!("stats.rs");
pub struct DrmOutput {
    pub output_id: OutputId,
    pub output: Output,
    pub compositor: GbmDrmCompositor,
    pub crtc: crtc::Handle,
    pub connector: connector::Handle,
    pub modes: Vec<Mode>,
    pub wallpaper: Option<WallpaperGpuCache>,
    pub frame_in_flight: bool,
    pub needs_repaint: bool,
    pub scratch_normal: Vec<render::MeridianRenderElements>,
    pub scratch_cursor: Vec<render::MeridianRenderElements>,
    pub scratch_final: Vec<render::MeridianRenderElements>,
    pub scratch_windows: Vec<Window>,
    pub scratch_lower_layer_data: Vec<render::LayerRenderData>,
    pub scratch_upper_layer_data: Vec<render::LayerRenderData>,
    pub scratch_lower_layer_elements: Vec<render::MeridianRenderElements>,
    pub scratch_upper_layer_elements: Vec<render::MeridianRenderElements>,
}

#[derive(Debug, Clone)]
pub struct DisabledDrmOutput {
    pub name: String,
    pub connector: connector::Handle,
    pub crtc: crtc::Handle,
    pub reserved_geometry_hint: OutputGeometry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrmCursorIcon {
    Default,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
}

pub struct DrmBackend {
    pub device_fd: DrmDeviceFd,
    pub kms_node_path: std::sync::Arc<str>,
    pub kms_is_primary_node: bool,
    pub kms_master_lock_ok: bool,
    pub kms_first_commit_verified: bool,
    pub renderer: GlesRenderer,
    pub outputs: Vec<DrmOutput>,
    pub disabled_outputs: Vec<DisabledDrmOutput>,
    pub cursor_image: CursorImage,
    pub cursor_buffer: MemoryRenderBuffer,
    pub compositor_cursor_cache: HashMap<DrmCursorIcon, (CursorImage, MemoryRenderBuffer)>,
    pub named_cursor_cache: HashMap<String, (MemoryRenderBuffer, Point<i32, Logical>)>,
    pub cursor_icon: DrmCursorIcon,
    pub dirty_stats: DrmDirtyStats,
    pub last_pointer_location: Option<(f64, f64)>,
    pub last_connector_scan: Instant,
    pub timing_stats: DrmTimingStats,
    pub repaint_idle_scheduled: bool,
}

impl DrmBackend {
    pub fn disable_output(&mut self, name: &str) -> Option<DisabledDrmOutput> {
        let idx = self
            .outputs
            .iter()
            .position(|drm_output| drm_output.output.name() == name)?;
        let removed = self.outputs.remove(idx);
        self.dirty_stats.unregister_output(removed.output_id);
        let geometry = removed.output.current_mode().map_or(
            OutputGeometry {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            |mode| OutputGeometry {
                x: 0,
                y: 0,
                width: mode.size.w,
                height: mode.size.h,
            },
        );
        self.disabled_outputs
            .retain(|disabled| disabled.name != name);
        let disabled = DisabledDrmOutput {
            name: name.to_string(),
            connector: removed.connector,
            crtc: removed.crtc,
            reserved_geometry_hint: geometry,
        };
        self.disabled_outputs.push(disabled.clone());
        tracing::info!(
            "output {} disabled live: compositor disposed, connector retained",
            name
        );
        Some(disabled)
    }

    pub fn enable_output_pull_pending(&mut self, name: &str) -> Option<DisabledDrmOutput> {
        let idx = self
            .disabled_outputs
            .iter()
            .position(|disabled| disabled.name == name)?;
        Some(self.disabled_outputs.remove(idx))
    }

    pub fn rebuild_compositor_for_mode(
        &mut self,
        _state_display_handle: &smithay::reexports::wayland_server::DisplayHandle,
        output_name: &str,
        new_mode_override: &OutputModeConfig,
    ) -> Option<(i32, i32, i32)> {
        let Some(idx) = self
            .outputs
            .iter()
            .position(|drm_output| drm_output.output.name() == output_name)
        else {
            tracing::warn!(
                "rebuild_compositor_for_mode: output {} not found",
                output_name
            );
            return None;
        };

        let modes = self.outputs[idx].modes.as_slice();
        let (mode, reason) = match mode_selection::select_mode_with_override(
            modes,
            Some(new_mode_override),
            output_name,
        ) {
            Some(pair) => pair,
            None => {
                tracing::warn!(
                    "rebuild compositor: no mode found for output {} (override={:?})",
                    output_name,
                    new_mode_override
                );
                return None;
            }
        };
        tracing::info!(
            "rebuild compositor: output={} new_mode={}x{} reason={}",
            output_name,
            mode.size().0,
            mode.size().1,
            reason
        );

        let smithay_output = self.outputs[idx].output.clone();
        if let Err(err) = self.outputs[idx].compositor.use_mode(mode) {
            tracing::warn!("rebuild compositor: live mode switch failed: {}", err);
            return None;
        }

        self.outputs[idx].frame_in_flight = false;
        self.outputs[idx].needs_repaint = true;

        let output_mode = smithay::output::Mode {
            size: (mode.size().0 as i32, mode.size().1 as i32).into(),
            refresh: mode_selection::mode_refresh_millihz_with_fallback(mode),
        };
        smithay_output.change_current_state(Some(output_mode), None, None, None);
        smithay_output.set_preferred(output_mode);
        Some((
            mode.size().0 as i32,
            mode.size().1 as i32,
            mode_selection::mode_refresh_millihz_with_fallback(mode),
        ))
    }
}
