pub(crate) fn build_drm_compositor(
    params: DrmCompositorBuildParams<'_>,
) -> Result<(super::GbmDrmCompositor, GbmDevice<DrmDeviceFd>), Box<dyn std::error::Error>> {
    let DrmCompositorBuildParams {
        device_fd,
        drm,
        crtc,
        connector,
        mode,
        renderer_formats,
        output,
        gbm,
        state_display_handle: _state_display_handle,
    } = params;

    let surface = drm.create_surface(crtc, mode, &[connector])?;
    let (mode_w, mode_h) = mode.size();
    info!(
        "drm kms surface created: connector={:?} crtc={:?} mode={}x{}@{}Hz calc_refresh_millihz={}",
        connector,
        crtc,
        mode_w,
        mode_h,
        mode.vrefresh(),
        mode_refresh_millihz_with_fallback(mode)
    );

    let gbm = match gbm {
        Some(existing) => existing,
        None => GbmDevice::new(device_fd.clone())?,
    };
    let allocator = GbmAllocator::new(
        gbm.clone(),
        GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
    );
    let exporter = GbmFramebufferExporter::new(gbm.clone(), NodeFilter::All);
    let force_format = forced_scanout_format_from_env();
    let color_formats = selected_scanout_formats(force_format);
    info!(
        "drm compositor format assumptions: forced_format={:?} color_formats={:?}",
        force_format, color_formats
    );

    let compositor = DrmCompositor::new(
        output,
        surface,
        None,
        allocator,
        exporter,
        color_formats,
        renderer_formats.iter().cloned(),
        drm.cursor_size(),
        Some(gbm.clone()),
    )?;

    Ok((compositor, gbm))
}
