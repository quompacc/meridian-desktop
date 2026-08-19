fn configure_repaint_interval(
    drm_outputs: &[DrmOutput],
    first_selected_mode_refresh_millihz: Option<i32>,
) -> Duration {
    let mode_refresh_hint_millihz = first_selected_mode_refresh_millihz.or_else(|| {
        drm_outputs
            .first()
            .and_then(|output| output.output.current_mode().map(|mode| mode.refresh))
    });
    let mode_interval_hint = mode_refresh_hint_millihz.and_then(duration_from_millihz);
    let default_repaint_interval = mode_interval_hint.unwrap_or_else(|| Duration::from_millis(16));
    let default_repaint_source = mode_refresh_hint_millihz
        .map(|millihz| format!("default:calculated-mode-refresh({millihz}mHz)"))
        .unwrap_or_else(|| "default:hardcoded-16ms".to_string());
    let (repaint_interval, repaint_source) =
        select_repaint_interval(default_repaint_interval, default_repaint_source);
    tracing::info!(
        "drm repaint scheduler interval configured (timer-only, not KMS mode forcing): interval_ms={} interval_ns={} source={} mode_refresh_hint_millihz={:?} mode_interval_hint_ms={:?}",
        repaint_interval.as_millis(),
        repaint_interval.as_nanos(),
        repaint_source,
        mode_refresh_hint_millihz,
        mode_interval_hint.map(|duration| duration.as_millis())
    );
    repaint_interval
}

fn register_drm_event_source<Source>(
    event_loop: &mut EventLoop<MeridianState>,
    drm_notifier: Source,
) -> Result<(), Box<dyn std::error::Error>>
where
    Source: smithay::reexports::calloop::EventSource<Event = DrmEvent, Ret = ()> + 'static,
    Source::Metadata: 'static,
    Source::Error: std::error::Error + 'static,
{
    event_loop
        .handle()
        .insert_source(drm_notifier, |event, _metadata, state| match event {
            DrmEvent::VBlank(crtc) => {
                let vblank_event_at = std::time::Instant::now();
                if let Some(drm) = &mut state.drm_backend {
                    let handler_started = std::time::Instant::now();
                    let mut frame_submitted_duration = Duration::ZERO;
                    let mut matched_output = false;
                    if let Some(out) = drm.outputs.iter_mut().find(|o| o.crtc == crtc) {
                        matched_output = true;
                        let frame_submitted_started = std::time::Instant::now();
                        if let Err(err) = out.compositor.frame_submitted() {
                            tracing::warn!(
                                "drm frame_submitted failed on output {}: {}",
                                out.output.name(),
                                err
                            );
                            out.frame_in_flight = false;
                        } else {
                            out.frame_in_flight = false;
                        }
                        frame_submitted_duration = frame_submitted_started.elapsed();
                    }
                    drm.timing_stats.record_vblank(
                        vblank_event_at,
                        handler_started.elapsed(),
                        frame_submitted_duration,
                        matched_output,
                    );
                }
                tracing::trace!("drm vblank event received: crtc={:?}", crtc);
                scan_drm_connectors_for_h5b(state, "vblank");
            }
            DrmEvent::Error(err) => {
                tracing::warn!("drm device error event received: err={}", err);
                scan_drm_connectors_for_h5b(state, "error");
            }
        })?;
    Ok(())
}

fn register_repaint_timer_source(
    event_loop: &mut EventLoop<MeridianState>,
    repaint_interval: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    event_loop.handle().insert_source(
        Timer::from_duration(repaint_interval),
        move |timer_fired_at, _metadata, state| {
            let tick_started = std::time::Instant::now();
            let metrics = render_outputs(state);
            if let Some(drm) = state.drm_backend.as_mut() {
                drm.timing_stats.record_render_tick(
                    timer_fired_at,
                    tick_started,
                    tick_started.elapsed(),
                    metrics,
                );
                drm.dirty_stats.report_if_due(tick_started);
            }
            // Idle screen blanking
            if let Some(timeout) = state.idle_timeout {
                if state.idle_blanked {
                    // Keep submitting frames while blanked so the DRM/KMS
                    // display link stays active (virtio-gpu goes to
                    // hardware-DPMS-off if we stop flipping for too long).
                    if let Some(drm) = state.drm_backend.as_mut() {
                        for out in drm.outputs.iter_mut() {
                            out.needs_repaint = true;
                        }
                    }
                } else if !state.idle_inhibitors.is_inhibited()
                    && state.last_activity.elapsed() >= timeout
                {
                    tracing::info!("idle timeout reached, blanking outputs");
                    state.idle_blanked = true;
                    if let Some(drm) = state.drm_backend.as_mut() {
                        for out in drm.outputs.iter_mut() {
                            out.needs_repaint = true;
                        }
                    }
                }
            }
            // Keep a fixed display cadence. Re-arming relative to the end of
            // rendering would add render time to every interval (for example
            // 16 ms render + 16 ms wait = about 30 fps on a 60 Hz output).
            TimeoutAction::ToInstant(next_repaint_deadline(
                timer_fired_at,
                std::time::Instant::now(),
                repaint_interval,
            ))
        },
    )?;
    Ok(())
}

fn next_repaint_deadline(
    previous_deadline: std::time::Instant,
    now: std::time::Instant,
    interval: Duration,
) -> std::time::Instant {
    let mut next = previous_deadline + interval;
    while next <= now {
        next += interval;
    }
    next
}

#[cfg(not(target_os = "openbsd"))]
fn register_libinput_event_source(
    event_loop: &mut EventLoop<MeridianState>,
    libinput: Libinput,
) -> Result<(), Box<dyn std::error::Error>> {
    event_loop
        .handle()
        .insert_source(LibinputInputBackend::new(libinput), |event, _, state| {
            state.process_input_event(event);
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::next_repaint_deadline;
    use std::time::{Duration, Instant};

    #[test]
    fn repaint_deadline_does_not_add_render_time_to_the_interval() {
        let fired = Instant::now();
        let interval = Duration::from_millis(16);

        assert_eq!(
            next_repaint_deadline(fired, fired + Duration::from_millis(15), interval),
            fired + interval
        );
        assert_eq!(
            next_repaint_deadline(fired, fired + Duration::from_millis(17), interval),
            fired + interval * 2
        );
    }
}
