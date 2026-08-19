fn scan_drm_connectors_for_h5b(state: &mut MeridianState, source: &str) {
    tracing::trace!("drm connector scan triggered: source={}", source);
    let (should_scan, device_fd, known_connectors, known_output_names) =
        if let Some(drm) = state.drm_backend.as_mut() {
            let should_scan = drm.last_connector_scan.elapsed() >= Duration::from_millis(750);
            if should_scan {
                drm.last_connector_scan = std::time::Instant::now();
            } else {
                tracing::trace!(
                    "drm connector scan skipped: source={} reason=throttled",
                    source
                );
            }
            let known_connectors = drm
                .outputs
                .iter()
                .map(|out| (out.connector, out.output.name()))
                .chain(
                    drm.disabled_outputs
                        .iter()
                        .map(|out| (out.connector, out.name.clone())),
                )
                .collect::<Vec<_>>();
            let known_output_names = drm
                .outputs
                .iter()
                .map(|out| out.output.name())
                .chain(drm.disabled_outputs.iter().map(|out| out.name.clone()))
                .collect::<HashSet<_>>();
            (
                should_scan,
                drm.device_fd.clone(),
                known_connectors,
                known_output_names,
            )
        } else {
            return;
        };

    if !should_scan {
        return;
    }

    let resources = match device_fd.resource_handles() {
        Ok(resources) => resources,
        Err(err) => {
            tracing::warn!("drm hotplug scan failed to read resource handles: {}", err);
            return;
        }
    };

    let registry_by_name = state
        .output_registry
        .list()
        .iter()
        .filter(|info| known_output_names.contains(&info.name))
        .map(|info| {
            (
                info.name.clone(),
                (
                    info.id,
                    info.geometry.x,
                    info.geometry.y,
                    info.geometry.width,
                    info.geometry.height,
                    info.refresh_millihz.unwrap_or_default(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    let known_name_by_connector = known_connectors
        .iter()
        .map(|(connector, name)| (*connector, name.as_str()))
        .collect::<HashMap<_, _>>();

    let mut connected_modes = HashMap::new();
    for conn_handle in resources.connectors() {
        let Ok(conn) = device_fd.get_connector(*conn_handle, false) else {
            continue;
        };
        if conn.state() != smithay::reexports::drm::control::connector::State::Connected {
            continue;
        }
        if let Some(name) = known_name_by_connector.get(conn_handle) {
            if let Some((_, _, _, width, height, refresh_millihz)) = registry_by_name.get(*name) {
                connected_modes.insert(*conn_handle, (*width, *height, *refresh_millihz));
                continue;
            }
        }
        let modes = conn.modes();
        if modes.is_empty() {
            continue;
        }
        let Some((mode, _mode_reason)) = select_add_mode(modes) else {
            tracing::trace!(
                "drm hotplug scan skipped connector without selectable mode: connector={:?}",
                conn_handle
            );
            continue;
        };
        let (w, h) = mode.size();
        connected_modes.insert(
            *conn_handle,
            (w as i32, h as i32, mode_refresh_millihz_with_fallback(mode)),
        );
    }

    let changes =
        classify_drm_connector_changes(&known_connectors, &connected_modes, &registry_by_name);

    for candidate in changes.reconfigure {
        tracing::debug!(
            "drm connector reconfigure detected: output_id={} width={} height={} refresh={}",
            candidate.output_id.0,
            candidate.width,
            candidate.height,
            candidate.refresh_millihz
        );
        if state.handle_output_reconfigured(
            candidate.output_id,
            OutputReconfigure {
                geometry: MeridianState::output_geometry_for_registry(
                    candidate.geometry_x,
                    candidate.geometry_y,
                    candidate.width,
                    candidate.height,
                ),
                scale: 1.0,
                transform: Transform::Normal,
                refresh_millihz: Some(candidate.refresh_millihz),
                primary: None,
            },
        ) {
            tracing::debug!(
                "drm output reconfigured via hotplug pipeline: output_id={}",
                candidate.output_id.0
            );
        }
    }

    for connector in changes.add {
        tracing::info!("drm output add detected: connector={:?}", connector);
        if add_drm_output_via_hotplug_pipeline(state, connector) {
            tracing::info!(
                "drm output added via hotplug pipeline: connector={:?}",
                connector
            );
        }
    }

    for candidate in changes.remove {
        tracing::info!(
            "drm output remove detected: connector={:?} output_id={:?}",
            candidate.connector,
            candidate.output_id.map(|id| id.0)
        );
        if remove_drm_output_via_hotplug_pipeline(state, candidate) {
            tracing::info!(
                "drm output removed via hotplug pipeline: connector={:?} output_id={:?}",
                candidate.connector,
                candidate.output_id.map(|id| id.0)
            );
        }
    }
}

fn add_drm_output_via_hotplug_pipeline(
    state: &mut MeridianState,
    connector: smithay::reexports::drm::control::connector::Handle,
) -> bool {
    let (device_fd, occupied_crtcs, renderer_formats) = {
        let Some(drm) = state.drm_backend.as_ref() else {
            tracing::warn!("drm output add skipped reason=drm-backend-missing");
            return false;
        };
        let renderer_formats: HashSet<Format> = drm
            .renderer
            .egl_context()
            .dmabuf_render_formats()
            .iter()
            .cloned()
            .collect();
        let renderer_formats =
            maybe_disable_modifiers(renderer_formats, disable_drm_modifiers_requested());
        (
            drm.device_fd.clone(),
            drm.outputs
                .iter()
                .map(|output| output.crtc)
                .collect::<Vec<_>>(),
            renderer_formats,
        )
    };

    let (mut drm, _notifier) = match DrmDevice::new(device_fd.clone(), false) {
        Ok(pair) => pair,
        Err(err) => {
            tracing::warn!(
                "drm output add skipped reason=device-open-failed connector={:?} err={}",
                connector,
                err
            );
            return false;
        }
    };

    let resources = match drm.resource_handles() {
        Ok(resources) => resources,
        Err(err) => {
            tracing::warn!(
                "drm output add skipped reason=resource-handles-failed connector={:?} err={}",
                connector,
                err
            );
            return false;
        }
    };

    let conn = match drm.get_connector(connector, false) {
        Ok(conn) => conn,
        Err(err) => {
            tracing::warn!(
                "drm output add skipped reason=connector-query-failed connector={:?} err={}",
                connector,
                err
            );
            return false;
        }
    };
    if conn.state() != smithay::reexports::drm::control::connector::State::Connected {
        tracing::warn!(
            "drm output add skipped reason=connector-not-connected connector={:?}",
            connector
        );
        return false;
    }

    let output_name = format!("drm-{}", state.outputs.len());
    let output_mode_override = state
        .output_config_entries
        .iter()
        .find(|entry| entry.name == output_name)
        .and_then(|entry| entry.mode.clone());
    let output_transform_override = state
        .output_config_entries
        .iter()
        .find(|entry| entry.name == output_name)
        .and_then(|entry| entry.transform.clone());
    let output_scale_override = state
        .output_config_entries
        .iter()
        .find(|entry| entry.name == output_name)
        .map(|entry| entry.scale)
        .unwrap_or(1.0);

    let modes = conn.modes();
    log_connector_modes("drm output add connector mode", connector, modes);
    let Some((mode, mode_reason)) =
        select_mode_with_override(modes, output_mode_override.as_ref(), &output_name)
    else {
        tracing::warn!(
            "drm output add skipped reason=no-mode connector={:?}",
            connector
        );
        return false;
    };
    let (width, height) = mode.size();
    let refresh_millihz = mode_refresh_millihz_with_fallback(mode);
    log_mode_details("drm output add selected mode details", connector, mode);
    tracing::debug!(
        "drm output add selected mode: connector={:?} mode={}x{} refresh={} reason={}",
        connector,
        width,
        height,
        refresh_millihz,
        mode_reason
    );

    let Some(crtc_handle) = super::gpu::pick_crtc(&drm, &resources, &conn, &occupied_crtcs) else {
        tracing::warn!(
            "drm output add skipped reason=no-free-crtc connector={:?}",
            connector
        );
        return false;
    };

    let mut pending: Vec<ConnectedOutput> = state
        .output_registry
        .list()
        .iter()
        .map(|info| ConnectedOutput {
            name: info.name.clone(),
            width: info.geometry.width,
            height: info.geometry.height,
        })
        .collect();
    pending.push(ConnectedOutput {
        name: output_name.clone(),
        width: width as i32,
        height: height as i32,
    });
    let resolved = state.resolve_output_layout(&pending);
    let new_resolved = resolved
        .iter()
        .find(|entry| entry.name == output_name)
        .expect("new output in resolver result");
    let (x, y) = (new_resolved.x, new_resolved.y);
    tracing::debug!(
        "resolved layout for hotplug output {}: x={} y={}",
        output_name,
        x,
        y
    );
    if !new_resolved.enabled {
        let disabled = DisabledDrmOutput {
            name: new_resolved.name.clone(),
            connector,
            crtc: crtc_handle,
            reserved_geometry_hint: MeridianState::output_geometry_for_registry(
                new_resolved.x,
                new_resolved.y,
                width as i32,
                height as i32,
            ),
        };
        if let Some(drm) = state.drm_backend.as_mut() {
            drm.disabled_outputs
                .retain(|existing| existing.name != new_resolved.name);
            drm.disabled_outputs.push(disabled);
        }
        tracing::info!(
            "hotplugged connector {:?} is disabled per TOML config; stored as disabled_output",
            connector
        );
        return true;
    }

    let transform = output_transform_override
        .as_deref()
        .map(parse_output_transform)
        .unwrap_or(Transform::Normal);
    if transform != Transform::Normal {
        tracing::debug!(
            "output {} uses non-normal transform {:?}; geometry tracking remains mode-size based",
            output_name,
            transform
        );
    }
    let scale_value = parse_output_scale(output_scale_override, &output_name);
    let output_scale = output_scale_from_value(scale_value);

    let phys_size = conn.size().map_or((0, 0), |s| (s.0 as i32, s.1 as i32));
    let output = Output::new(
        output_name.clone(),
        PhysicalProperties {
            size: phys_size.into(),
            subpixel: Subpixel::Unknown,
            make: "Unknown".into(),
            model: "Unknown".into(),
            serial_number: "Unknown".into(),
        },
    );
    let _global = output.create_global::<MeridianState>(&state.display_handle);
    let out_mode = OutputMode {
        size: (width as i32, height as i32).into(),
        refresh: refresh_millihz,
    };
    output.change_current_state(
        Some(out_mode),
        Some(transform),
        Some(output_scale),
        Some((x, y).into()),
    );
    output.set_preferred(out_mode);

    let compositor = match build_drm_compositor(DrmCompositorBuildParams {
        state_display_handle: &state.display_handle,
        device_fd: device_fd.clone(),
        drm: &mut drm,
        crtc: crtc_handle,
        connector,
        mode,
        renderer_formats: &renderer_formats,
        output: &output,
        gbm: None,
    }) {
        Ok((compositor, _gbm)) => compositor,
        Err(err) => {
            tracing::warn!(
                "drm output add skipped reason=compositor-create-failed connector={:?} err={}",
                connector,
                err
            );
            return false;
        }
    };

    state
        .workspaces
        .active_space_mut()
        .map_output(&output, (x, y));
    state.outputs.push(output.clone());

    let output_id = state.handle_output_added_or_updated(OutputRegistration {
        name: output_name.clone(),
        geometry: MeridianState::output_geometry_for_registry(x, y, width as i32, height as i32),
        scale: scale_value,
        transform,
        refresh_millihz: Some(refresh_millihz),
    });
    state
        .output_registry
        .set_modes_by_id(output_id, output_mode_infos(modes, mode));
    sync_primary_flags_from_resolved_layout(state, &resolved);

    let Some(drm_backend) = state.drm_backend.as_mut() else {
        tracing::warn!(
            "drm output add skipped reason=drm-backend-lost-after-state-update connector={:?}",
            connector
        );
        return false;
    };
    let output_name = output.name();
    drm_backend.outputs.push(DrmOutput {
        output_id,
        output,
        compositor,
        crtc: crtc_handle,
        connector,
        modes: modes.to_vec(),
        wallpaper: None,
        frame_in_flight: false,
        needs_repaint: true,
        scratch_normal: Vec::new(),
        scratch_cursor: Vec::new(),
        scratch_final: Vec::new(),
        scratch_windows: Vec::new(),
        scratch_lower_layer_data: Vec::new(),
        scratch_upper_layer_data: Vec::new(),
        scratch_lower_layer_elements: Vec::new(),
        scratch_upper_layer_elements: Vec::new(),
    });
    drm_backend
        .dirty_stats
        .register_output(output_id, output_name);

    tracing::debug!(
        "drm output added via hotplug pipeline details: connector={:?} output_id={}",
        connector,
        output_id.0
    );
    true
}

fn remove_drm_output_via_hotplug_pipeline(
    state: &mut MeridianState,
    candidate: DrmConnectorRemoveCandidate,
) -> bool {
    let Some((removed_output, removed_output_name)) = detach_drm_output(state, candidate.connector)
    else {
        if let Some(drm) = state.drm_backend.as_mut() {
            if let Some(disabled_idx) = drm
                .disabled_outputs
                .iter()
                .position(|output| output.connector == candidate.connector)
            {
                let removed_disabled = drm.disabled_outputs.remove(disabled_idx);
                tracing::info!(
                    "drm disabled output removed via hotplug pipeline: connector={:?} output={}",
                    candidate.connector,
                    removed_disabled.name
                );
                return true;
            }
        }
        tracing::warn!(
            "drm output remove skipped reason=connector-not-found connector={:?}",
            candidate.connector
        );
        return false;
    };

    for workspace_idx in 0..state.workspaces.count() {
        state
            .workspaces
            .space_at_mut(workspace_idx)
            .unmap_output(&removed_output);
    }
    layer_map_for_output(&removed_output).cleanup();

    if let Some(idx) = state
        .outputs
        .iter()
        .position(|output| output.name() == removed_output_name)
    {
        state.outputs.remove(idx);
    } else {
        tracing::warn!(
            "drm output remove skipped reason=state-output-not-found output={}",
            removed_output_name
        );
    }

    let output_id = candidate.output_id.or_else(|| {
        state
            .output_registry
            .list()
            .iter()
            .find(|info| info.name == removed_output_name)
            .map(|info| info.id)
    });

    let Some(output_id) = output_id else {
        tracing::warn!(
            "drm output remove skipped reason=registry-output-id-missing output={}",
            removed_output_name
        );
        return false;
    };

    if !state.handle_output_removed(output_id) {
        tracing::warn!(
            "drm output remove skipped reason=handle-output-removed-failed output_id={}",
            output_id.0
        );
        return false;
    }

    true
}

fn detach_drm_output(
    state: &mut MeridianState,
    connector: smithay::reexports::drm::control::connector::Handle,
) -> Option<(Output, String)> {
    let drm = state.drm_backend.as_mut()?;
    let idx = drm
        .outputs
        .iter()
        .position(|output| output.connector == connector)?;
    let removed = drm.outputs.remove(idx);
    drm.dirty_stats.unregister_output(removed.output_id);
    let name = removed.output.name();
    Some((removed.output, name))
}
