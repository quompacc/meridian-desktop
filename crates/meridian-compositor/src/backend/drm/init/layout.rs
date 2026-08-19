fn sync_primary_flags_from_resolved_layout(state: &mut MeridianState, resolved: &[ResolvedOutput]) {
    let current_by_name: HashMap<_, _> = state
        .output_registry
        .list()
        .iter()
        .map(|info| {
            (
                info.name.clone(),
                (
                    info.geometry,
                    info.scale,
                    info.transform,
                    info.refresh_millihz,
                ),
            )
        })
        .collect();

    for output in resolved {
        let Some((geometry, scale, transform, refresh_millihz)) = current_by_name.get(&output.name)
        else {
            continue;
        };
        let _ = state.output_registry.reconfigure_by_name(
            &output.name,
            OutputReconfigure {
                geometry: *geometry,
                scale: *scale,
                transform: *transform,
                refresh_millihz: *refresh_millihz,
                primary: Some(output.primary),
            },
        );
    }
}

fn parse_output_scale(scale_value: f64, output_name: &str) -> f64 {
    if !scale_value.is_finite() || scale_value <= 0.0 {
        tracing::warn!(
            "output {} scale {:?} invalid — using 1.0",
            output_name,
            scale_value
        );
        return 1.0;
    }

    scale_value
}

fn output_scale_from_value(scale_value: f64) -> Scale {
    if (scale_value - scale_value.round()).abs() < f64::EPSILON {
        return Scale::Integer(scale_value as i32);
    }

    Scale::Fractional(scale_value)
}

fn classify_drm_connector_changes(
    known_connectors: &[(smithay::reexports::drm::control::connector::Handle, String)],
    connected_modes: &HashMap<smithay::reexports::drm::control::connector::Handle, (i32, i32, i32)>,
    registry_by_name: &HashMap<String, (crate::state::OutputId, i32, i32, i32, i32, i32)>,
) -> DrmConnectorChangeSet {
    let mut changes = DrmConnectorChangeSet::default();
    let known_set: HashSet<_> = known_connectors.iter().map(|(conn, _)| *conn).collect();

    for (connector, name) in known_connectors {
        let Some((new_w, new_h, new_refresh)) = connected_modes.get(connector).copied() else {
            changes.remove.push(DrmConnectorRemoveCandidate {
                connector: *connector,
                output_id: registry_by_name.get(name).map(|(id, _, _, _, _, _)| *id),
            });
            continue;
        };
        let Some((id, x, y, old_w, old_h, old_refresh)) = registry_by_name.get(name).copied()
        else {
            continue;
        };
        if old_w != new_w || old_h != new_h || old_refresh != new_refresh {
            changes.reconfigure.push(DrmConnectorReconfigureCandidate {
                output_id: id,
                geometry_x: x,
                geometry_y: y,
                width: new_w,
                height: new_h,
                refresh_millihz: new_refresh,
            });
        }
    }

    for connector in connected_modes.keys() {
        if !known_set.contains(connector) {
            changes.add.push(*connector);
        }
    }

    changes
}
