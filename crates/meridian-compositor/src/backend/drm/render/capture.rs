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

/// Fulfil queued (policy-allowed) screenshot bridge requests: render the full
/// output once, encode it as a PNG under XDG_RUNTIME_DIR, and reply to each
/// requester with the file path token. Mirrors `process_thumbnail_requests`'
/// full-output render path. Runs in the render loop where the renderer is live.
fn process_screenshot_requests(
    state: &mut MeridianState,
    renderer: &mut GlesRenderer,
    out: &super::DrmOutput,
    out_size: (u32, u32),
) {
    use meridian_ipc::{ScreenshotBridgeError, ScreenshotBridgeResponse, ScreenshotBridgeResult};
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

    if state.pending_screenshot_requests.is_empty() {
        return;
    }

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
                tracing::warn!("screenshot: output draw error: {:?}", e);
            }
        }
        drop(gles_frame);
        let mapping = renderer
            .copy_framebuffer(&target, buf_region, Fourcc::Xrgb8888)
            .ok()?;
        let raw = renderer.map_texture(&mapping).ok()?;
        Some(raw.to_vec())
    })();

    let requests = std::mem::take(&mut state.pending_screenshot_requests);

    let Some(full_pixels) = full_pixels else {
        tracing::warn!(
            "screenshot: full output render failed, failing {} request(s)",
            requests.len()
        );
        for req in requests {
            state.ipc.send_screenshot_bridge_response(
                req.client_id,
                req.request.request_id,
                ScreenshotBridgeResult::Error {
                    error: ScreenshotBridgeError::Internal("output render failed".to_string()),
                },
            );
        }
        return;
    };

    for req in requests {
        let request_id = req.request.request_id.clone();
        // Per-request region crop: if the request carries a region, crop the
        // full-output framebuffer to it and encode at the cropped size. The
        // region was validated upstream (width/height > 0); clamping below
        // is a defence-in-depth against a bad geometry slipping through.
        let (encoded_pixels, encoded_w, encoded_h);
        let cropped_storage: Vec<u8>;
        if let Some(region) = req.request.region {
            let rx = region.x.max(0) as u32;
            let ry = region.y.max(0) as u32;
            let rw = region.width.min(out_size.0.saturating_sub(rx));
            let rh = region.height.min(out_size.1.saturating_sub(ry));
            if rw == 0 || rh == 0 {
                tracing::warn!(
                    "screenshot: region {:?} clamps to empty against output {}x{} for {}",
                    region,
                    out_size.0,
                    out_size.1,
                    request_id
                );
                state.ipc.send_screenshot_bridge_response(
                    req.client_id,
                    request_id,
                    ScreenshotBridgeResult::Error {
                        error: ScreenshotBridgeError::InvalidRequest(
                            "region falls outside the output".to_string(),
                        ),
                    },
                );
                continue;
            }
            cropped_storage = crop_xrgb(&full_pixels, out_size.0, rx, ry, rw, rh);
            encoded_pixels = cropped_storage.as_slice();
            encoded_w = rw;
            encoded_h = rh;
        } else {
            encoded_pixels = full_pixels.as_slice();
            encoded_w = out_size.0;
            encoded_h = out_size.1;
        }
        let result = match encode_screenshot_png(encoded_pixels, encoded_w, encoded_h, &request_id)
        {
            Ok(path) => ScreenshotBridgeResult::Success {
                response: ScreenshotBridgeResponse {
                    request_id: request_id.clone(),
                    file_descriptor_token: Some(path),
                },
            },
            Err(e) => {
                tracing::warn!("screenshot: encode/write failed for {}: {}", request_id, e);
                ScreenshotBridgeResult::Error {
                    error: ScreenshotBridgeError::Internal(e),
                }
            }
        };
        state
            .ipc
            .send_screenshot_bridge_response(req.client_id, request_id, result);
    }
}

/// Encode XRGB8888 framebuffer pixels as PNG bytes. The framebuffer is
/// little-endian Xrgb8888 (bytes B,G,R,X), so R and B are swapped for the RGB
/// PNG; alpha is forced opaque. Pure — unit-tested without a GPU.
fn screenshot_png_bytes(xrgb: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};

    let mut rgba: Vec<u8> = Vec::with_capacity((width as usize) * (height as usize) * 4);
    let stride = (width * 4) as usize;
    for y in 0..height as usize {
        for x in 0..width as usize {
            let i = y * stride + x * 4;
            if i + 3 >= xrgb.len() {
                return Err("pixel buffer shorter than width*height*4".to_string());
            }
            // Xrgb8888 little-endian: [B, G, R, X] -> RGBA (R, G, B, 255).
            rgba.push(xrgb[i + 2]);
            rgba.push(xrgb[i + 1]);
            rgba.push(xrgb[i]);
            rgba.push(255);
        }
    }

    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(&rgba, width, height, ExtendedColorType::Rgba8)
        .map_err(|e| format!("png encode failed: {}", e))?;
    Ok(out)
}

/// Encode the framebuffer to PNG and write it under XDG_RUNTIME_DIR, returning
/// the file path used as the response token.
fn encode_screenshot_png(
    xrgb: &[u8],
    width: u32,
    height: u32,
    request_id: &str,
) -> Result<String, String> {
    let png = screenshot_png_bytes(xrgb, width, height)?;
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    let path = format!(
        "{}/meridian-screenshot-{}.png",
        dir,
        sanitize_window_id(request_id)
    );
    std::fs::write(&path, &png).map_err(|e| format!("png write failed: {}", e))?;
    Ok(path)
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

#[cfg(test)]
mod screenshot_tests {
    use super::screenshot_png_bytes;

    #[test]
    fn encodes_xrgb_with_red_blue_swapped() {
        // One pixel, Xrgb8888 little-endian bytes [B, G, R, X] = pure red.
        let xrgb = [0u8, 0, 255, 0];
        let png = screenshot_png_bytes(&xrgb, 1, 1).expect("encode");
        // Decode it back and confirm the pixel is red (R=255,G=0,B=0).
        let img = image::load_from_memory(&png).expect("decode").to_rgba8();
        assert_eq!(img.dimensions(), (1, 1));
        let px = img.get_pixel(0, 0).0;
        assert_eq!(px, [255, 0, 0, 255]);
    }

    #[test]
    fn dimensions_and_blue_channel_round_trip() {
        // 2x1: pixel0 = blue (B=255), pixel1 = green (G=255).
        let xrgb = [
            255u8, 0, 0, 0, /* px0 blue */ 0, 255, 0, 0, /* px1 green */
        ];
        let png = screenshot_png_bytes(&xrgb, 2, 1).expect("encode");
        let img = image::load_from_memory(&png).expect("decode").to_rgba8();
        assert_eq!(img.dimensions(), (2, 1));
        assert_eq!(img.get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(img.get_pixel(1, 0).0, [0, 255, 0, 255]);
    }

    #[test]
    fn short_buffer_is_an_error_not_a_panic() {
        assert!(screenshot_png_bytes(&[0, 0, 0, 0], 4, 4).is_err());
    }
}
