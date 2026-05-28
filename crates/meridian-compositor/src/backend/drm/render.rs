use std::time::{Duration, Instant};

use smithay::backend::renderer::element::{
    render_elements, surface::render_elements_from_surface_tree,
    surface::WaylandSurfaceRenderElement, AsRenderElements, Wrap,
};
use smithay::{
    backend::renderer::{
        element::{
            memory::MemoryRenderBufferRenderElement, solid::SolidColorRenderElement,
            texture::TextureRenderElement, Kind,
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
    backend::clipped_surface::ClippedSurfaceRenderElement,
    state::{LockPhase, MeridianState, OutputPowerMode},
    wallpaper::WallpaperGpuCache,
};

use super::{DrmBackend, RenderPassMetrics};

mod layers;
mod stack;

pub(super) use self::layers::LayerRenderData;
pub use stack::{layer_role, render_stack_order, RenderStackRole};

use self::layers::{collect_layer_data, render_layer_elements, send_layer_frames};

render_elements! {
    pub MeridianRenderElements<=GlesRenderer>;
    Cursor=MemoryRenderBufferRenderElement<GlesRenderer>,
    Space=SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>,
    Decoration=SolidColorRenderElement,
    DecorationIcon=Wrap<MemoryRenderBufferRenderElement<GlesRenderer>>,
    Shadow=PixelShaderElement,
    ClippedSurface=ClippedSurfaceRenderElement,
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

fn render_window_popup_elements<C>(
    renderer: &mut GlesRenderer,
    window: &Window,
    window_loc: smithay::utils::Point<i32, smithay::utils::Logical>,
    scale: Scale<f64>,
    out: &mut Vec<C>,
) where
    C: From<SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>>,
{
    let WindowSurface::Wayland(toplevel) = window.underlying_surface() else {
        return;
    };

    let surface = toplevel.wl_surface();
    let location = window_loc.to_physical_precise_round(scale);
    out.extend(
        PopupManager::popups_for_surface(surface)
            .flat_map(|(popup, popup_offset)| {
                let offset = (window.geometry().loc + popup_offset - popup.geometry().loc)
                    .to_physical_precise_round(scale);
                render_elements_from_surface_tree::<
                    GlesRenderer,
                    WaylandSurfaceRenderElement<GlesRenderer>,
                >(
                    renderer,
                    popup.wl_surface(),
                    location + offset,
                    scale,
                    1.0,
                    Kind::Unspecified,
                )
            })
            .map(SpaceRenderElements::from)
            .map(C::from),
    );
}

fn render_window_toplevel_elements<C>(
    renderer: &mut GlesRenderer,
    window: &Window,
    window_loc: smithay::utils::Point<i32, smithay::utils::Logical>,
    scale: Scale<f64>,
    clip: Option<(
        smithay::backend::renderer::gles::GlesTexProgram,
        smithay::utils::Rectangle<f64, smithay::utils::Logical>,
        [u8; 4],
    )>,
    out: &mut Vec<C>,
) where
    C: From<SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>>
        + From<ClippedSurfaceRenderElement>,
{
    match window.underlying_surface() {
        WindowSurface::Wayland(toplevel) => {
            let elements = render_elements_from_surface_tree::<
                GlesRenderer,
                WaylandSurfaceRenderElement<GlesRenderer>,
            >(
                renderer,
                toplevel.wl_surface(),
                window_loc.to_physical_precise_round(scale),
                scale,
                1.0,
                Kind::Unspecified,
            );
            match clip {
                Some((prog, geo, radius)) => {
                    out.extend(elements.into_iter().map(|e| {
                        C::from(ClippedSurfaceRenderElement::new(
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
                            .map(C::from),
                    );
                }
            }
        }
        WindowSurface::X11(_) => {
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
    }
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

    // Drive the login->desktop compass zoom-out. The intro is armed at
    // setup (the wallpaper already shows the compass at login size); start
    // the countdown on the first committed frame, then shrink the compass
    // to wallpaper size and settle onto the static image.
    if state.wallpaper_manager.intro_active() {
        const INTRO_SECS: f32 = 0.7;
        const START_RF: f32 = 0.32;
        const END_RF: f32 = 0.19;
        if state.intro_start.is_none() && kms_first_commit_verified {
            state.intro_start = Some(Instant::now());
        }
        if let Some(started) = state.intro_start {
            let p = (started.elapsed().as_secs_f32() / INTRO_SECS).clamp(0.0, 1.0);
            let eased = 1.0 - (1.0 - p).powi(3);
            state
                .wallpaper_manager
                .set_intro_radius(START_RF + (END_RF - START_RF) * eased);
            if p >= 1.0 {
                state.wallpaper_manager.end_intro();
                state.intro_start = None;
            }
            for out in outputs.iter_mut() {
                out.needs_repaint = true;
            }
        }
    }

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
        #[cfg(debug_assertions)]
        let mut cursor_count = 0usize;
        if !state.lock_manager.is_locked_or_pending() {
            for window in out.scratch_windows.iter().rev() {
                let loc = match space.element_location(window) {
                    Some(l) => l,
                    None => continue,
                };
                let geometry = window.geometry();
                let render_loc =
                    smithay::utils::Point::from((loc.x - geometry.loc.x, loc.y - geometry.loc.y));

                render_window_popup_elements(
                    renderer,
                    window,
                    render_loc,
                    scale,
                    &mut out.scratch_normal,
                );

                let mut content_clip = None;
                if let Some(wl_surf) = window.wl_surface().map(|s| s.into_owned()) {
                    let metrics = state.decoration_manager.ssd_render_metrics(
                        &wl_surf,
                        loc,
                        geometry.size,
                        &theme.decorations,
                    );
                    let window_deco_elements = state.decoration_manager.render_elements(
                        renderer,
                        &wl_surf,
                        metrics.frame_origin,
                        metrics.client_size,
                        &theme.decorations,
                        &theme.colors,
                        scale,
                    );
                    decoration_element_count += window_deco_elements.len();
                    out.scratch_normal
                        .extend(
                            window_deco_elements
                                .into_iter()
                                .map(|element| match element {
                                    crate::decoration::DecorationRenderElement::Solid(solid) => {
                                        MeridianRenderElements::Decoration(solid)
                                    }
                                    crate::decoration::DecorationRenderElement::Icon(icon) => {
                                        MeridianRenderElements::DecorationIcon(icon.into())
                                    }
                                    crate::decoration::DecorationRenderElement::PixelShader(s) => {
                                        MeridianRenderElements::Shadow(s)
                                    }
                                }),
                        );

                    // Round the client content's bottom corners to match the
                    // rounded border/titlebar (top corners sit under the
                    // titlebar, so only the bottom two need clipping).
                    if let Some(r) = state
                        .decoration_manager
                        .content_corner_radius(&wl_surf, &theme.decorations)
                    {
                        if let Some(prog) =
                            crate::backend::clipped_surface::clip_shader(renderer)
                        {
                            let r8 = r.min(255) as u8;
                            content_clip =
                                Some((prog, metrics.client_rect.to_f64(), [r8, 0, r8, 0]));
                        }
                    }
                }

                let space_start = out.scratch_normal.len();
                render_window_toplevel_elements(
                    renderer,
                    window,
                    render_loc,
                    scale,
                    content_clip,
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
                        match &state.cursor_status {
                            CursorImageStatus::Hidden => {}
                            CursorImageStatus::Named(icon_name) => {
                                // Preserve compositor-managed resize cursors for SSD/X11 edge hit-tests.
                                if !matches!(cursor_icon, super::DrmCursorIcon::Default) {
                                    let mut cursor_loc = cursor_pos;
                                    cursor_loc.x -= cursor_image.xhot as i32;
                                    cursor_loc.y -= cursor_image.yhot as i32;
                                    if let Ok(element) =
                                        MemoryRenderBufferRenderElement::from_buffer(
                                            renderer,
                                            cursor_loc.to_f64(),
                                            cursor_buffer,
                                            None,
                                            None,
                                            None,
                                            Kind::Cursor,
                                        )
                                    {
                                        out.scratch_cursor
                                            .push(MeridianRenderElements::Cursor(element));
                                    }
                                } else {
                                    let cursor_cfg = &state.theme_manager.current().config.cursor;
                                    let (named_buffer, hotspot) = named_cursor_cache
                                        .entry(icon_name.name().to_string())
                                        .or_insert_with(|| {
                                            let cursor =
                                                crate::cursor::CursorImage::load_theme_cursor_icon(
                                                    &cursor_cfg.theme,
                                                    cursor_cfg.size,
                                                    *icon_name,
                                                );
                                            (
                                                cursor.to_memory_buffer(),
                                                smithay::utils::Point::from((
                                                    cursor.xhot as i32,
                                                    cursor.yhot as i32,
                                                )),
                                            )
                                        });
                                    let mut cursor_loc = cursor_pos;
                                    cursor_loc.x -= hotspot.x;
                                    cursor_loc.y -= hotspot.y;
                                    if let Ok(element) =
                                        MemoryRenderBufferRenderElement::from_buffer(
                                            renderer,
                                            cursor_loc.to_f64(),
                                            named_buffer,
                                            None,
                                            None,
                                            None,
                                            Kind::Cursor,
                                        )
                                    {
                                        out.scratch_cursor
                                            .push(MeridianRenderElements::Cursor(element));
                                    }
                                }
                            }
                            CursorImageStatus::Surface(surface) => {
                                let hotspot = with_states(surface, |states| {
                                    states
                                        .data_map
                                        .get::<CursorImageSurfaceData>()
                                        .map(|attrs| attrs.lock().unwrap().hotspot)
                                        .unwrap_or_default()
                                });
                                let hotspot =
                                    hotspot.to_f64().to_physical(scale).to_i32_round::<i32>();
                                let cursor_loc = smithay::utils::Point::from((
                                    cursor_pos.x - hotspot.x,
                                    cursor_pos.y - hotspot.y,
                                ));
                                out.scratch_cursor
                                    .extend(render_elements_from_surface_tree::<
                                        GlesRenderer,
                                        MeridianRenderElements,
                                    >(
                                        renderer,
                                        surface,
                                        cursor_loc,
                                        scale,
                                        1.0,
                                        Kind::Cursor,
                                    ));
                            }
                        }
                    }
                }
            }

            collect_layer_data(
                &out.output,
                &mut out.scratch_lower_layer_data,
                &mut out.scratch_upper_layer_data,
            );
            render_layer_elements(
                renderer,
                &out.scratch_lower_layer_data,
                scale,
                &mut out.scratch_lower_layer_elements,
            );
            render_layer_elements(
                renderer,
                &out.scratch_upper_layer_data,
                scale,
                &mut out.scratch_upper_layer_elements,
            );

            let wallpaper_elem = out
                .wallpaper
                .as_ref()
                .map(WallpaperGpuCache::render_element);

            #[cfg(debug_assertions)]
            {
                cursor_count = out.scratch_cursor.len();
            }
            {
                let (
                    scratch_final,
                    scratch_cursor,
                    scratch_normal,
                    scratch_lower_layer_elements,
                    scratch_upper_layer_elements,
                ) = (
                    &mut out.scratch_final,
                    &mut out.scratch_cursor,
                    &mut out.scratch_normal,
                    &mut out.scratch_lower_layer_elements,
                    &mut out.scratch_upper_layer_elements,
                );
                scratch_final.append(scratch_cursor);
                scratch_final.append(scratch_upper_layer_elements);
                scratch_final.append(scratch_normal);
                scratch_final.append(scratch_lower_layer_elements);
                scratch_final.extend(
                    wallpaper_elem
                        .into_iter()
                        .map(MeridianRenderElements::Wallpaper),
                );
            }
        } else {
            out.scratch_cursor.clear();
            out.scratch_lower_layer_data.clear();
            out.scratch_upper_layer_data.clear();
            out.scratch_lower_layer_elements.clear();
            out.scratch_upper_layer_elements.clear();
            let output_name = out.output.name();
            if let Some(lock_surface) = state.lock_manager.surface_for_output(&output_name) {
                out.scratch_normal
                    .extend(render_elements_from_surface_tree::<
                        GlesRenderer,
                        MeridianRenderElements,
                    >(
                        renderer,
                        lock_surface.wl_surface(),
                        (0, 0),
                        scale,
                        1.0,
                        Kind::Unspecified,
                    ));
            }
            out.scratch_final.append(&mut out.scratch_normal);
            if matches!(state.lock_manager.phase(), LockPhase::Pending) {
                let maybe_ready_locker = state.lock_manager.record_pending_frame(&output_name);
                if let Some(locker) = maybe_ready_locker {
                    locker.lock();
                    let _ = state.lock_manager.confirm_locked();
                    state.refresh_lock_focus();
                    tracing::info!("session lock confirmed after cleared frames");
                }
            }
        }

        // Serve screencopy BEFORE render_frame so all Wayland surface textures
        // are still fresh and not yet assigned to KMS hardware planes (which
        // bypasses the GLES import path and makes draw() silently skip them).
        serve_screencopy_frames(state, renderer, out, out_size);
        process_thumbnail_requests(state, renderer, out, out_size);

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
                        // Phase 8: tell meridian-login the screen is ours
                        // now, so it can close its login framebuffer fd.
                        super::login_ipc::send_first_frame();
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
            send_layer_frames(
                &out_clone,
                time,
                &out.scratch_lower_layer_data,
                &out.scratch_upper_layer_data,
            );
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

/// Render pending screencopy frames for a single output into the client's SHM buffers.
fn serve_screencopy_frames(
    state: &mut MeridianState,
    renderer: &mut GlesRenderer,
    out: &super::DrmOutput,
    out_size: (u32, u32),
) {
    use smithay::{
        backend::{
            allocator::Fourcc,
            renderer::{
                element::{Element, RenderElement},
                Bind, ExportMem, Frame as RendererFrame, Offscreen, Renderer,
            },
        },
        utils::{Rectangle, Scale, Transform},
        wayland::{image_copy_capture::CaptureFailureReason, shm::with_buffer_contents_mut},
    };

    let w = out_size.0 as i32;
    let h = out_size.1 as i32;
    // create_buffer() wants Buffer coords; render() wants Physical coords.
    let buf_size = smithay::utils::Size::<i32, smithay::utils::Buffer>::from((w, h));
    let phys_size = smithay::utils::Size::<i32, smithay::utils::Physical>::from((w, h));
    let buf_region = Rectangle::from_size(buf_size);
    let phys_region = Rectangle::from_size(phys_size);

    let mut i = 0;
    while i < state.pending_screencopy_frames.len() {
        if state.pending_screencopy_frames[i].1 != out.output {
            i += 1;
            continue;
        }
        let (frame, _) = state.pending_screencopy_frames.remove(i);

        let pixels: Option<Vec<u8>> = (|| {
            let mut tex = <GlesRenderer as Offscreen<
                smithay::backend::renderer::gles::GlesTexture,
            >>::create_buffer(renderer, Fourcc::Xrgb8888, buf_size)
            .ok()?;
            let mut target = renderer.bind(&mut tex).ok()?;
            let mut gles_frame = renderer
                .render(&mut target, phys_size, Transform::Normal)
                .ok()?;
            let _ = gles_frame.clear([0.0f32, 0.0, 0.0, 1.0].into(), &[phys_region]);
            for element in out.scratch_final.iter().rev() {
                let src = element.src();
                let dst = element.geometry(Scale::from(1.0f64));
                // damage must be in element-local coords (origin at 0,0 within dst),
                // not absolute physical coords — passing dst directly would clamp y≥dst.size.h to 0.
                let element_damage = [smithay::utils::Rectangle::from_size(dst.size)];
                if let Err(e) = element.draw(&mut gles_frame, src, dst, &element_damage, &[], None)
                {
                    tracing::warn!(
                        "screencopy: draw error for {:?}: {:?}",
                        std::mem::discriminant(element),
                        e
                    );
                }
            }
            drop(gles_frame);
            let mapping = renderer
                .copy_framebuffer(&target, buf_region, Fourcc::Xrgb8888)
                .ok()?;
            let raw = renderer.map_texture(&mapping).ok()?;
            Some(raw.to_vec())
        })();

        match pixels {
            Some(pixels) => {
                let wl_buf = frame.buffer();
                let ok = with_buffer_contents_mut(&wl_buf, |ptr, len, _| {
                    let dst = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
                    let n = pixels.len().min(len);
                    dst[..n].copy_from_slice(&pixels[..n]);
                })
                .is_ok();
                if ok {
                    frame.success(Transform::Normal, None, state.start_time.elapsed());
                } else {
                    frame.fail(CaptureFailureReason::Unknown);
                }
            }
            None => {
                frame.fail(CaptureFailureReason::Unknown);
            }
        }
    }
}

/// Capture window thumbnails requested via IPC.
fn process_thumbnail_requests(
    state: &mut MeridianState,
    renderer: &mut GlesRenderer,
    out: &super::DrmOutput,
    out_size: (u32, u32),
) {
    use crate::state::window_list_entry;
    use meridian_ipc::ShellEvent;
    use smithay::{
        backend::{
            allocator::Fourcc,
            renderer::{
                element::{Element, RenderElement},
                Bind, ExportMem, Frame as RendererFrame, Offscreen, Renderer,
            },
        },
        utils::{Rectangle, Scale, Size, Transform},
    };

    if state.pending_thumbnail_requests.is_empty() {
        return;
    }

    // Render the full output once; crop per-window regions from the result.
    // This is identical to serve_screencopy_frames and guarantees correct pixel
    // content regardless of how individual window render_elements are positioned.
    let out_w = out_size.0 as i32;
    let out_h = out_size.1 as i32;
    let buf_size = Size::<i32, smithay::utils::Buffer>::from((out_w, out_h));
    let phys_size = Size::<i32, smithay::utils::Physical>::from((out_w, out_h));
    let phys_region = Rectangle::from_size(phys_size);
    let buf_region = Rectangle::from_size(buf_size);

    let full_pixels: Option<Vec<u8>> = (|| {
        let mut tex = <GlesRenderer as Offscreen<smithay::backend::renderer::gles::GlesTexture>>::create_buffer(
            renderer, Fourcc::Xrgb8888, buf_size,
        ).ok()?;
        let mut target = renderer.bind(&mut tex).ok()?;
        let mut gles_frame = renderer
            .render(&mut target, phys_size, Transform::Normal)
            .ok()?;
        let _ = gles_frame.clear([0.0f32, 0.0, 0.0, 1.0].into(), &[phys_region]);
        for element in out.scratch_final.iter().rev() {
            let src = element.src();
            let dst = element.geometry(Scale::from(1.0f64));
            let element_damage = [Rectangle::from_size(dst.size)];
            if let Err(e) = element.draw(&mut gles_frame, src, dst, &element_damage, &[], None) {
                tracing::warn!("thumbnail: output draw error: {:?}", e);
            }
        }
        drop(gles_frame);
        let mapping = renderer
            .copy_framebuffer(&target, buf_region, Fourcc::Xrgb8888)
            .ok()?;
        let raw = renderer.map_texture(&mapping).ok()?;
        Some(raw.to_vec())
    })();

    let Some(full_pixels) = full_pixels else {
        tracing::warn!(
            "thumbnail: full output render failed, dropping {} requests",
            state.pending_thumbnail_requests.len()
        );
        state.pending_thumbnail_requests.clear();
        return;
    };

    let requests = std::mem::take(&mut state.pending_thumbnail_requests);

    for req in requests {
        let window_id_for_debug = req.window_id.clone();
        let result: Option<()> = (|| {
            let idx = state.current_workspace_index();
            let window = state
                .workspaces
                .space_at(idx)
                .elements()
                .find(|w| {
                    window_list_entry(w)
                        .map(|(wid, _)| wid == req.window_id)
                        .unwrap_or(false)
                })
                .cloned()?;

            // Client geometry from space, then expand by Meridian SSD frame
            // (titlebar + border) so the thumbnail captures the whole window
            // chrome — not just the client surface region.
            let geo = state.workspaces.space_at(idx).element_geometry(&window)?;
            let (inset_l, inset_t, inset_r, inset_b) = if let Some(s) = window.wl_surface() {
                state
                    .decoration_manager
                    .decoration_inset(&s, &state.theme_manager.current().config.decorations)
            } else {
                (0, 0, 0, 0)
            };
            let frame_x = geo.loc.x - inset_l;
            let frame_y = geo.loc.y - inset_t;
            let frame_w = geo.size.w + inset_l + inset_r;
            let frame_h = geo.size.h + inset_t + inset_b;
            let wx = frame_x.clamp(0, out_w - 1) as u32;
            let wy = frame_y.clamp(0, out_h - 1) as u32;
            let wx2 = (frame_x + frame_w).clamp(0, out_w) as u32;
            let wy2 = (frame_y + frame_h).clamp(0, out_h) as u32;
            let cw = wx2.saturating_sub(wx).max(1);
            let ch = wy2.saturating_sub(wy).max(1);

            let cropped = crop_xrgb(&full_pixels, out_size.0, wx, wy, cw, ch);

            // Scale down maintaining aspect ratio
            let sx = req.max_width as f64 / cw as f64;
            let sy = req.max_height as f64 / ch as f64;
            let s = sx.min(sy).min(1.0);
            let thumb_w = ((cw as f64 * s).round() as u32).max(1);
            let thumb_h = ((ch as f64 * s).round() as u32).max(1);
            let thumb_pixels = scale_down_xrgb(&cropped, cw, ch, thumb_w, thumb_h);

            let path = format!(
                "/tmp/meridian-thumb-{}.rgba",
                sanitize_window_id(&req.window_id)
            );
            if let Err(e) = std::fs::write(&path, &thumb_pixels) {
                tracing::warn!("thumbnail: write failed {}: {}", path, e);
                return None;
            }

            state.ipc.broadcast(&ShellEvent::WindowThumbnail {
                id: req.window_id,
                path,
                width: thumb_w,
                height: thumb_h,
            });
            Some(())
        })();

        if result.is_none() {
            tracing::debug!(
                "thumbnail capture failed for window: {}",
                window_id_for_debug
            );
        }
    }
}

fn crop_xrgb(src: &[u8], src_w: u32, x: u32, y: u32, w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0u8; (w * h * 4) as usize];
    for row in 0..h {
        let si = ((y + row) * src_w + x) as usize * 4;
        let di = (row * w) as usize * 4;
        let len = (w * 4) as usize;
        if si + len <= src.len() {
            out[di..di + len].copy_from_slice(&src[si..si + len]);
        }
    }
    out
}

fn sanitize_window_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn scale_down_xrgb(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut out = vec![0u8; (dst_w * dst_h * 4) as usize];
    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let sx = (dx * src_w / dst_w) as usize;
            let sy = (dy * src_h / dst_h) as usize;
            let si = (sy * src_w as usize + sx) * 4;
            let di = (dy * dst_w + dx) as usize * 4;
            if si + 4 <= src.len() && di + 4 <= out.len() {
                out[di..di + 4].copy_from_slice(&src[si..si + 4]);
            }
        }
    }
    out
}
