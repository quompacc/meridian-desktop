use std::time::{Duration, Instant};

use meridian_config::ThemeSurface;

use smithay::backend::renderer::element::utils::{
    constrain_render_elements, ConstrainAlign, ConstrainScaleBehavior, CropRenderElement,
    RelocateRenderElement, RescaleRenderElement,
};
use smithay::backend::renderer::element::{
    render_elements, surface::render_elements_from_surface_tree,
    surface::WaylandSurfaceRenderElement, AsRenderElements, Wrap,
};
use smithay::{
    backend::renderer::{
        element::{
            memory::MemoryRenderBufferRenderElement,
            solid::SolidColorRenderElement,
            texture::{TextureBuffer, TextureRenderElement},
            Kind,
        },
        gles::{element::PixelShaderElement, GlesRenderer, GlesTexture},
    },
    desktop::{
        layer_map_for_output, space::SpaceRenderElements, PopupManager, Window, WindowSurface,
    },
    input::pointer::{CursorImageStatus, CursorImageSurfaceData},
    utils::Scale,
    wayland::{compositor::with_states, seat::WaylandFocus},
};
use tracing::{debug, error};

use crate::{
    backend::{
        clipped_surface::ClippedSurfaceRenderElement,
        non_opaque_surface::NonOpaqueSurfaceRenderElement,
    },
    state::{LockPhase, MeridianState, OutputPowerMode},
    wallpaper::WallpaperGpuCache,
};

use super::glass::{glass_shader, GlassElement, GlassTitlebarElement};
use super::{DrmBackend, RenderPassMetrics};

mod layers;
mod stack;

pub(super) use self::layers::LayerRenderData;
pub use stack::{layer_role, render_stack_order, RenderStackRole};

use self::layers::{collect_layer_data, render_layer_elements, send_layer_frames};

// Glass is sampled from a half-resolution scene. Three full-output offscreen
// passes consumed nearly the complete 60 Hz frame budget on Intel HD 620;
// bilinear upsampling preserves the deliberately soft material while reducing
// processed pixels by 75 percent.
const GLASS_SCENE_DOWNSAMPLE: u32 = 2;

render_elements! {
    pub MeridianRenderElements<=GlesRenderer>;
    Cursor=MemoryRenderBufferRenderElement<GlesRenderer>,
    Space=SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>,
    Decoration=SolidColorRenderElement,
    DecorationIcon=Wrap<MemoryRenderBufferRenderElement<GlesRenderer>>,
    Shadow=PixelShaderElement,
    Glass=GlassElement,
    ClippedSurface=ClippedSurfaceRenderElement,
    Wallpaper=TextureRenderElement<GlesTexture>,
    Layer=WaylandSurfaceRenderElement<GlesRenderer>,
    NonOpaqueLayer=NonOpaqueSurfaceRenderElement,
    ResizePreview=CropRenderElement<RelocateRenderElement<RescaleRenderElement<WaylandSurfaceRenderElement<GlesRenderer>>>>,
}

include!("render/scene_helpers.rs");
include!("render/scene_composition.rs");
fn render_window_toplevel_elements(
    renderer: &mut GlesRenderer,
    window: &Window,
    window_loc: smithay::utils::Point<i32, smithay::utils::Logical>,
    scale: Scale<f64>,
    preview_rect: Option<smithay::utils::Rectangle<i32, smithay::utils::Logical>>,
    clip: Option<(
        smithay::backend::renderer::gles::GlesTexProgram,
        smithay::utils::Rectangle<f64, smithay::utils::Logical>,
        [u8; 4],
    )>,
    out: &mut Vec<MeridianRenderElements>,
) {
    let elements = match window.underlying_surface() {
        WindowSurface::Wayland(toplevel) => render_elements_from_surface_tree::<
            GlesRenderer,
            WaylandSurfaceRenderElement<GlesRenderer>,
        >(
            renderer,
            toplevel.wl_surface(),
            window_loc.to_physical_precise_round(scale),
            scale,
            1.0,
            Kind::Unspecified,
        ),
        WindowSurface::X11(_) => window
            .render_elements::<WaylandSurfaceRenderElement<GlesRenderer>>(
                renderer,
                window_loc.to_physical_precise_round(scale),
                scale,
                1.0,
            ),
    };

    if let Some(target) = preview_rect {
        let geometry = window.geometry();
        let committed = smithay::utils::Rectangle::new(
            (window_loc + geometry.loc).to_physical_precise_round(scale),
            geometry.size.to_physical_precise_round(scale),
        );
        let target = target.to_physical_precise_round(scale);
        out.extend(
            constrain_render_elements(
                elements,
                committed.loc,
                target,
                committed,
                ConstrainScaleBehavior::Stretch,
                ConstrainAlign::TOP | ConstrainAlign::LEFT,
                scale,
            )
            .map(MeridianRenderElements::ResizePreview),
        );
    } else {
        match clip {
            Some((prog, geo, radius)) => {
                out.extend(elements.into_iter().map(|e| {
                    MeridianRenderElements::ClippedSurface(ClippedSurfaceRenderElement::new(
                        prog.clone(),
                        e,
                        scale,
                        geo,
                        radius,
                    ))
                }));
            }
            None => {
                out.extend(
                    elements
                        .into_iter()
                        .map(SpaceRenderElements::from)
                        .map(MeridianRenderElements::Space),
                );
            }
        }
    }
}

pub(super) fn render_outputs(state: &mut MeridianState) -> RenderPassMetrics {
    render_outputs_for_crtc(state, None)
}

pub(crate) fn render_outputs_from_idle(state: &mut MeridianState) {
    let tick_started = Instant::now();
    let metrics = render_outputs(state);
    if let Some(drm) = state.drm_backend.as_mut() {
        drm.timing_stats
            .record_idle_repaint(tick_started, tick_started.elapsed(), metrics);
        drm.dirty_stats.report_if_due(tick_started);
    }
}

pub(super) fn render_output_after_vblank(
    state: &mut MeridianState,
    crtc: smithay::reexports::drm::control::crtc::Handle,
) {
    let tick_started = Instant::now();
    let metrics = render_outputs_for_crtc(state, Some(crtc));
    if let Some(drm) = state.drm_backend.as_mut() {
        drm.timing_stats
            .record_vblank_repaint(tick_started, tick_started.elapsed(), metrics);
        drm.dirty_stats.report_if_due(tick_started);
    }
}

fn render_outputs_for_crtc(
    state: &mut MeridianState,
    target_crtc: Option<smithay::reexports::drm::control::crtc::Handle>,
) -> RenderPassMetrics {
    let mut metrics = RenderPassMetrics::default();
    let mut drm = match state.drm_backend.take() {
        Some(d) => d,
        None => return metrics,
    };

    state
        .wallpaper_manager
        .apply_theme(state.theme_manager.current());

    let DrmBackend {
        ref mut renderer,
        ref mut outputs,
        ref cursor_image,
        ref cursor_buffer,
        ref mut named_cursor_cache,
        ref cursor_icon,
        ref mut dirty_stats,
        ref mut last_pointer_location,
        ..
    } = drm;
    let mut kms_first_commit_verified = drm.kms_first_commit_verified;
    let kms_master_lock_ok = drm.kms_master_lock_ok;
    let kms_node_path = drm.kms_node_path.clone();
    let kms_is_primary_node = drm.kms_is_primary_node;

    let pointer_location = state
        .seat
        .get_pointer()
        .map(|pointer| pointer.current_location())
        .map(|loc| (loc.x, loc.y));
    if *last_pointer_location != pointer_location {
        for output in outputs.iter_mut() {
            dirty_stats.record_dirty_mark_event(output.output_id, "pointer_motion");
            if !output.needs_repaint {
                output.needs_repaint = true;
                dirty_stats.record_dirty_set(output.output_id);
            }
        }
    }
    *last_pointer_location = pointer_location;

    for out in outputs.iter_mut() {
        if target_crtc.is_some_and(|crtc| out.crtc != crtc) {
            continue;
        }
        let output_name_for_power = out.output.name();
        if matches!(
            state.output_power_manager.mode_for(&output_name_for_power),
            OutputPowerMode::Off
        ) {
            metrics.outputs_skipped_power_off += 1;
            dirty_stats.record_skipped_power_off(out.output_id);
            continue;
        }
        if out.frame_in_flight {
            metrics.outputs_skipped_in_flight += 1;
            continue;
        }
        if !out.needs_repaint {
            metrics.outputs_skipped_clean += 1;
            dirty_stats.record_skipped_clean(out.output_id);
            continue;
        }
        dirty_stats.record_rendered_dirty(out.output_id);
        let output_pass_started = Instant::now();
        metrics.outputs_processed += 1;
        let out_size = out
            .output
            .current_mode()
            .map(|m| (m.size.w as u32, m.size.h as u32))
            .unwrap_or((1920, 1080));
        let wallpaper_started = Instant::now();
        WallpaperGpuCache::update(
            renderer,
            &mut out.wallpaper,
            &mut state.wallpaper_manager,
            out_size.0,
            out_size.1,
        );
        metrics.wallpaper_duration += wallpaper_started.elapsed();

        let space = state.workspaces.active_space();
        let theme = &state.theme_manager.current().config;
        let scale = Scale::from(1.0f64);

        let scene_compose_started = Instant::now();
        out.scratch_normal.clear();
        out.scratch_cursor.clear();
        out.scratch_final.clear();
        out.scratch_windows.clear();
        out.scratch_windows.extend(space.elements().cloned());

        let mut decoration_element_count = 0usize;
        let mut space_element_count = 0usize;
        #[cfg(debug_assertions)]
        let mut cursor_count = 0usize;
        compose_output_scene!(
            state,
            renderer,
            out,
            space,
            theme,
            scale,
            cursor_icon,
            cursor_image,
            cursor_buffer,
            named_cursor_cache,
            decoration_element_count,
            space_element_count,
            cursor_count
        );
        metrics.scene_compose_duration += scene_compose_started.elapsed();

        // Serve screencopy BEFORE render_frame so all Wayland surface textures
        // are still fresh and not yet assigned to KMS hardware planes (which
        // bypasses the GLES import path and makes draw() silently skip them).
        let capture_started = Instant::now();
        serve_screencopy_frames(state, renderer, out, out_size);
        process_thumbnail_requests(state, renderer, out, out_size);
        process_screenshot_requests(state, renderer, out, out_size);
        metrics.capture_duration += capture_started.elapsed();

        // Liquid-glass: for each placeholder, render only the scene behind
        // that placeholder, blur it, and swap in a real sampling element.
        let glass_started = Instant::now();
        if !state.idle_blanked
            && out
                .scratch_final
                .iter()
                .any(|e| matches!(e, MeridianRenderElements::Glass(_)))
        {
            if let Some(prog) = glass_shader(renderer) {
                let pending: Vec<_> = out
                    .scratch_final
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, el)| match el {
                        MeridianRenderElements::Glass(g) => {
                            g.pending_info().map(|info| (idx, info))
                        }
                        _ => None,
                    })
                    .collect();
                let mut batches: Vec<PendingGlassBatch> = Vec::new();
                for (idx, info) in pending {
                    let first_behind =
                        first_rendered_blur_source(&out.scratch_final, idx.saturating_add(1));
                    let blur_bits = info.blur.to_bits();
                    match batches.last_mut() {
                        Some(batch)
                            if batch.first_behind == first_behind
                                && batch.blur_bits == blur_bits =>
                        {
                            batch.items.push((idx, info));
                        }
                        _ => batches.push(PendingGlassBatch {
                            first_behind,
                            blur_bits,
                            items: vec![(idx, info)],
                        }),
                    }
                }
                for (batch_index, batch) in batches.into_iter().enumerate() {
                    let blur = batch.items[0].1.blur;
                    let glass_size = (
                        out_size.0.div_ceil(GLASS_SCENE_DOWNSAMPLE),
                        out_size.1.div_ceil(GLASS_SCENE_DOWNSAMPLE),
                    );
                    let glass_scale = (
                        glass_size.0 as f64 / out_size.0.max(1) as f64,
                        glass_size.1 as f64 / out_size.1.max(1) as f64,
                    );
                    let Some(buffers) =
                        out.glass_pass_cache
                            .batch(renderer, glass_size, batch_index)
                    else {
                        continue;
                    };
                    if !render_scene_for_blur(
                        renderer,
                        &out.scratch_final,
                        batch.first_behind,
                        glass_size,
                        glass_scale,
                        &mut buffers.scene,
                    ) {
                        continue;
                    }
                    let Some(blurred) = blur_scene_cached(
                        renderer,
                        buffers,
                        glass_size,
                        blur / GLASS_SCENE_DOWNSAMPLE as f32,
                    ) else {
                        continue;
                    };
                    let buffer = TextureBuffer::from_texture(
                        renderer,
                        blurred,
                        1,
                        smithay::utils::Transform::Normal,
                        None,
                    );
                    for (idx, info) in batch.items {
                        let ready = GlassTitlebarElement::new(
                            prog.clone(),
                            &buffer,
                            info,
                            (out_size.0 as i32, out_size.1 as i32),
                            glass_scale,
                            scale,
                        );
                        if let Some(MeridianRenderElements::Glass(g)) =
                            out.scratch_final.get_mut(idx)
                        {
                            *g = GlassElement::Ready(ready);
                        }
                    }
                }
            }
        }
        metrics.glass_duration += glass_started.elapsed();

        let elements: &[MeridianRenderElements] = if state.idle_blanked {
            &[]
        } else {
            out.scratch_final.as_slice()
        };

        let layer_surface_count =
            out.scratch_lower_layer_data.len() + out.scratch_upper_layer_data.len();
        let render_element_count = elements.len();
        let logged_element_count = render_element_count + layer_surface_count;
        #[cfg(debug_assertions)]
        {
            let render_order = render_stack_order(
                cursor_count,
                out.scratch_upper_layer_data.len(),
                elements
                    .iter()
                    .filter(|element| {
                        matches!(
                            element,
                            MeridianRenderElements::Decoration(_)
                                | MeridianRenderElements::DecorationIcon(_)
                        )
                    })
                    .count()
                    .saturating_sub(cursor_count),
                elements
                    .iter()
                    .filter(|element| matches!(element, MeridianRenderElements::Space(_)))
                    .count(),
                out.scratch_lower_layer_data.len(),
                elements
                    .iter()
                    .filter(|element| matches!(element, MeridianRenderElements::Wallpaper(_)))
                    .count(),
            );
            debug_assert!(
                !render_order.contains(&RenderStackRole::Cursor)
                    || render_order.first() == Some(&RenderStackRole::Cursor)
            );
        }

        let bg = if state.idle_blanked {
            [0.0_f32, 0.0, 0.0, 1.0]
        } else {
            [0.0_f32; 4]
        };
        let render_frame_started = Instant::now();
        let mut frame_queued = false;
        match out
            .compositor
            .render_frame::<GlesRenderer, MeridianRenderElements>(
                renderer,
                elements,
                bg,
                smithay::backend::drm::compositor::FrameFlags::empty(),
            ) {
            Ok(frame) if !frame.is_empty => {
                metrics.render_frame_duration += render_frame_started.elapsed();
                metrics.rendered_frames += 1;
                metrics.render_elements += render_element_count as u64;
                metrics.layer_surfaces += layer_surface_count as u64;
                if layer_surface_count > 0 {
                    metrics.rendered_outputs_with_layers += 1;
                }
                if space_element_count > 0 {
                    metrics.rendered_outputs_with_space += 1;
                }
                if layer_surface_count > 0
                    && space_element_count == 0
                    && decoration_element_count == 0
                {
                    metrics.rendered_outputs_with_layers_only += 1;
                }
                let mode_str = out.output.current_mode().map_or_else(
                    || "?".to_string(),
                    |m| format!("{}x{}@{}Hz", m.size.w, m.size.h, m.refresh / 1000),
                );
                debug!(
                    "frame rendered: output={} mode={} elements={} render_elements={} layer_surfaces={}",
                    out.output.name(),
                    mode_str,
                    logged_element_count,
                    render_element_count,
                    layer_surface_count
                );
                let queue_started = Instant::now();
                if let Err(err) = out.compositor.queue_frame(()) {
                    metrics.queue_failures += 1;
                    error!("DRM queue_frame error on {}: {}", out.output.name(), err);
                    if !kms_first_commit_verified {
                        panic!(
                            "fatal drm startup failure: first KMS commit failed on output={} node={} primary_node={} master_lock_ok={}: {}",
                            out.output.name(),
                            kms_node_path,
                            kms_is_primary_node,
                            kms_master_lock_ok,
                            err
                        );
                    }
                } else {
                    out.frame_in_flight = true;
                    clear_output_dirty(out, dirty_stats, "queue_frame_success");
                    frame_queued = true;
                    metrics.queued_frames += 1;
                    if !kms_first_commit_verified {
                        kms_first_commit_verified = true;
                        if !kms_master_lock_ok {
                            tracing::info!(
                                "diagnostic drm master lock check failed earlier, but functional KMS gate succeeded (first commit ok); continuing"
                            );
                        } else {
                            tracing::info!(
                                "initial KMS commit succeeded: output={} node={}",
                                out.output.name(),
                                kms_node_path
                            );
                        }
                        // Phase 8: tell meridian-login the screen is ours
                        // now, so it can close its login framebuffer fd.
                        super::login_ipc::send_first_frame();
                    }
                }
                metrics.queue_duration += queue_started.elapsed();
            }
            Ok(_) => {
                metrics.empty_frames += 1;
                metrics.render_frame_duration += render_frame_started.elapsed();
                clear_output_dirty(out, dirty_stats, "empty_frame");
            }
            Err(err) => {
                metrics.render_frame_duration += render_frame_started.elapsed();
                error!("DRM render error on {}: {}", out.output.name(), err);
                if !kms_first_commit_verified {
                    panic!(
                        "fatal drm startup failure: first KMS render/commit preparation failed on output={} node={} primary_node={} master_lock_ok={}: {}",
                        out.output.name(),
                        kms_node_path,
                        kms_is_primary_node,
                        kms_master_lock_ok,
                        err
                    );
                }
            }
        }

        if frame_queued {
            let frame_feedback_started = Instant::now();
            let time = state.start_time.elapsed();
            let out_clone = out.output.clone();
            state.workspaces.active_space().elements().for_each(|w| {
                w.send_frame(&out_clone, time, Some(Duration::ZERO), |_, _| {
                    Some(out_clone.clone())
                });
            });
            send_layer_frames(
                &out_clone,
                time,
                &out.scratch_lower_layer_data,
                &out.scratch_upper_layer_data,
            );
            metrics.frame_feedback_duration += frame_feedback_started.elapsed();
        }
        metrics.output_pass_duration += output_pass_started.elapsed();
    }

    state.workspaces.active_space_mut().refresh();
    state.popups.cleanup();
    for output in &state.outputs {
        layer_map_for_output(output).cleanup();
    }
    let _ = state.display_handle.flush_clients();
    drm.kms_first_commit_verified = kms_first_commit_verified;
    state.drm_backend = Some(drm);
    metrics
}

// Render pending screencopy frames for a single output into the client's SHM buffers.
include!("render/capture.rs");
