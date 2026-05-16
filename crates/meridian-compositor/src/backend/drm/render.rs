use std::time::{Duration, Instant};

use smithay::backend::renderer::element::{render_elements, surface::WaylandSurfaceRenderElement};
use smithay::{
    backend::renderer::{
        element::{
            memory::MemoryRenderBufferRenderElement, solid::SolidColorRenderElement,
            texture::TextureRenderElement, Kind,
        },
        gles::{GlesRenderer, GlesTexture},
    },
    desktop::{
        layer_map_for_output,
        space::{space_render_elements, SpaceRenderElements},
        Space, Window,
    },
    output::Output,
    utils::Scale,
    wayland::seat::WaylandFocus,
};
use tracing::{debug, error};

use crate::{state::MeridianState, wallpaper::WallpaperGpuCache};

use super::{DrmBackend, RenderPassMetrics};

mod layers;
mod stack;

pub use stack::{layer_role, render_stack_order, RenderStackRole};

use self::layers::{collect_layer_data, render_layer_elements, send_layer_frames};

render_elements! {
    pub MeridianRenderElements<=GlesRenderer>;
    Cursor=MemoryRenderBufferRenderElement<GlesRenderer>,
    Space=SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>,
    Decoration=SolidColorRenderElement,
    Wallpaper=TextureRenderElement<GlesTexture>,
    Layer=WaylandSurfaceRenderElement<GlesRenderer>,
}

fn clear_output_dirty(
    output: &mut super::DrmOutput,
    dirty_stats: &mut super::DrmDirtyStats,
    reason: &str,
) {
    if output.needs_repaint {
        output.needs_repaint = false;
        dirty_stats.record_dirty_clear(output.output_id);
        tracing::trace!(
            "output repaint clean set: reason={} output_id={} output={}",
            reason,
            output.output_id.0,
            output.output.name()
        );
    }
}

fn render_window_space_elements<C>(
    renderer: &mut GlesRenderer,
    output: &Output,
    window: &Window,
    window_loc: smithay::utils::Point<i32, smithay::utils::Logical>,
    out: &mut Vec<C>,
) where
    C: From<SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>>,
{
    let mut window_space = Space::<Window>::default();
    window_space.map_output(output, (0, 0));
    window_space.map_element(window.clone(), window_loc, false);
    out.extend(
        space_render_elements::<GlesRenderer, Window, _>(renderer, [&window_space], output, 1.0)
            .unwrap_or_default()
            .into_iter()
            .map(C::from),
    );
}

pub(super) fn render_outputs(state: &mut MeridianState) -> RenderPassMetrics {
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
        WallpaperGpuCache::update(
            renderer,
            &mut out.wallpaper,
            &mut state.wallpaper_manager,
            out_size.0,
            out_size.1,
        );

        let space = state.workspaces.active_space();
        let theme = &state.theme_manager.current().config;
        let scale = Scale::from(1.0f64);

        out.scratch_normal.clear();
        out.scratch_cursor.clear();
        out.scratch_final.clear();
        out.scratch_windows.clear();
        out.scratch_windows.extend(space.elements().cloned());

        let mut decoration_element_count = 0usize;
        let mut space_element_count = 0usize;
        for window in out.scratch_windows.iter().rev() {
            let loc = match space.element_location(window) {
                Some(l) => l,
                None => continue,
            };

            if let Some(wl_surf) = window.wl_surface().map(|s| s.into_owned()) {
                let geo = window.geometry();
                let metrics = state.decoration_manager.ssd_render_metrics(
                    &wl_surf,
                    loc,
                    geo.size,
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
                decoration_element_count += window_deco_elements.len();
                out.scratch_normal.extend(
                    window_deco_elements
                        .into_iter()
                        .map(MeridianRenderElements::Decoration),
                );
            }

            let space_start = out.scratch_normal.len();
            render_window_space_elements(
                renderer,
                &out.output,
                window,
                loc,
                &mut out.scratch_normal,
            );
            let appended_space = out.scratch_normal.len().saturating_sub(space_start);
            space_element_count += appended_space;
        }

        if let Some(pointer) = state.seat.get_pointer() {
            let pointer_location = pointer.current_location();
            if let Some(output_geo) = space.output_geometry(&out.output) {
                if output_geo.to_f64().contains(pointer_location) {
                    let cursor_pos = (pointer_location - output_geo.loc.to_f64())
                        .to_physical(scale)
                        .to_i32_round::<i32>();
                    let mut cursor_loc = cursor_pos;
                    cursor_loc.x -= cursor_image.xhot as i32;
                    cursor_loc.y -= cursor_image.yhot as i32;
                    if let Ok(element) = MemoryRenderBufferRenderElement::from_buffer(
                        renderer,
                        cursor_loc.to_f64(),
                        cursor_buffer,
                        None,
                        None,
                        None,
                        Kind::Cursor,
                    ) {
                        out.scratch_cursor
                            .push(MeridianRenderElements::Cursor(element));
                    }
                }
            }
        }

        let (lower_layer_data, upper_layer_data) = collect_layer_data(&out.output);
        let lower_layer_elements = render_layer_elements(renderer, &lower_layer_data, scale);
        let upper_layer_elements = render_layer_elements(renderer, &upper_layer_data, scale);

        let wallpaper_elem = out
            .wallpaper
            .as_ref()
            .map(WallpaperGpuCache::render_element);

        let cursor_count = out.scratch_cursor.len();
        {
            let (scratch_final, scratch_cursor, scratch_normal) = (
                &mut out.scratch_final,
                &mut out.scratch_cursor,
                &mut out.scratch_normal,
            );
            scratch_final.append(scratch_cursor);
            scratch_final.extend(upper_layer_elements);
            scratch_final.append(scratch_normal);
            scratch_final.extend(lower_layer_elements);
            scratch_final.extend(
                wallpaper_elem
                    .into_iter()
                    .map(MeridianRenderElements::Wallpaper),
            );
        }

        let elements = out.scratch_final.as_slice();

        let layer_surface_count = lower_layer_data.len() + upper_layer_data.len();
        let render_element_count = elements.len();
        let logged_element_count = render_element_count + layer_surface_count;
        let render_order = render_stack_order(
            cursor_count,
            upper_layer_data.len(),
            elements
                .iter()
                .filter(|element| matches!(element, MeridianRenderElements::Decoration(_)))
                .count()
                .saturating_sub(cursor_count),
            elements
                .iter()
                .filter(|element| matches!(element, MeridianRenderElements::Space(_)))
                .count(),
            lower_layer_data.len(),
            elements
                .iter()
                .filter(|element| matches!(element, MeridianRenderElements::Wallpaper(_)))
                .count(),
        );
        debug_assert!(
            !render_order.contains(&RenderStackRole::Cursor)
                || render_order.first() == Some(&RenderStackRole::Cursor)
        );

        let bg = [0.0_f32; 4];
        let commit_started = Instant::now();
        let mut frame_queued = false;
        match out
            .compositor
            .render_frame::<GlesRenderer, MeridianRenderElements>(
                renderer,
                elements,
                bg,
                smithay::backend::drm::compositor::FrameFlags::DEFAULT,
            ) {
            Ok(frame) if !frame.is_empty => {
                metrics.commit_duration += commit_started.elapsed();
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
                    }
                }
                metrics.queue_duration += queue_started.elapsed();
            }
            Ok(_) => {
                metrics.empty_frames += 1;
                metrics.commit_duration += commit_started.elapsed();
                clear_output_dirty(out, dirty_stats, "empty_frame");
            }
            Err(err) => {
                metrics.commit_duration += commit_started.elapsed();
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
            let time = state.start_time.elapsed();
            let out_clone = out.output.clone();
            state.workspaces.active_space().elements().for_each(|w| {
                w.send_frame(&out_clone, time, Some(Duration::ZERO), |_, _| {
                    Some(out_clone.clone())
                });
            });
            send_layer_frames(&out_clone, time, &lower_layer_data, &upper_layer_data);
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
