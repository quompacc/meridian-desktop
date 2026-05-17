use std::{collections::HashSet, env, time::Duration};

use smithay::backend::allocator::{Format, Fourcc, Modifier};

pub(super) fn env_flag_enabled(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

pub(super) fn force_drm_legacy_requested() -> bool {
    env_flag_enabled("MERIDIAN_DRM_FORCE_LEGACY")
}

pub(crate) fn disable_drm_modifiers_requested() -> bool {
    env_flag_enabled("MERIDIAN_DRM_DISABLE_MODIFIERS")
}

pub(super) fn forced_scanout_format_from_env() -> Option<Fourcc> {
    let value = env::var("MERIDIAN_DRM_FORCE_FORMAT").ok()?;
    let normalized = value.trim().to_ascii_uppercase();
    match normalized.as_str() {
        "XRGB8888" => Some(Fourcc::Xrgb8888),
        "ARGB8888" => Some(Fourcc::Argb8888),
        _ => None,
    }
}

pub(crate) fn maybe_disable_modifiers(
    formats: HashSet<Format>,
    disable_modifiers: bool,
) -> HashSet<Format> {
    if !disable_modifiers {
        return formats;
    }
    formats
        .into_iter()
        .map(|format| Format {
            code: format.code,
            modifier: Modifier::Invalid,
        })
        .collect()
}

pub(super) fn selected_scanout_formats(force_format: Option<Fourcc>) -> Vec<Fourcc> {
    match force_format {
        Some(format) => vec![format],
        None => vec![Fourcc::Argb8888, Fourcc::Xrgb8888],
    }
}

fn duration_from_hz(hz: u32) -> Option<Duration> {
    if hz == 0 {
        return None;
    }
    Some(Duration::from_nanos((1_000_000_000u64 / hz as u64).max(1)))
}

pub(super) fn duration_from_millihz(millihz: i32) -> Option<Duration> {
    if millihz <= 0 {
        return None;
    }
    Some(Duration::from_nanos(
        ((1_000_000_000u128 * 1000) / millihz as u128) as u64,
    ))
}

pub(super) fn select_repaint_interval(
    default: Duration,
    default_source: String,
) -> (Duration, String) {
    if let Ok(value) = env::var("MERIDIAN_DRM_FRAME_INTERVAL_MS") {
        match value.trim().parse::<u64>() {
            Ok(ms) if ms > 0 => {
                return (
                    Duration::from_millis(ms),
                    format!(
                        "env:MERIDIAN_DRM_FRAME_INTERVAL_MS({})[repaint-timer-interval-override]",
                        ms
                    ),
                );
            }
            _ => {
                tracing::warn!(
                    "invalid MERIDIAN_DRM_FRAME_INTERVAL_MS value {:?}; using default {:?}",
                    value,
                    default
                );
            }
        }
    }

    if let Ok(value) = env::var("MERIDIAN_DRM_FORCE_REFRESH_HZ") {
        match value.trim().parse::<u32>() {
            Ok(hz) if hz > 0 => {
                if let Some(interval) = duration_from_hz(hz) {
                    return (
                        interval,
                        format!(
                            "env:MERIDIAN_DRM_FORCE_REFRESH_HZ({})[repaint-timer-hz-override]",
                            hz
                        ),
                    );
                }
            }
            _ => {
                tracing::warn!(
                    "invalid MERIDIAN_DRM_FORCE_REFRESH_HZ value {:?}; using default {:?}",
                    value,
                    default
                );
            }
        }
    }

    (default, default_source)
}

pub(super) fn env_value_or_unset(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| "<unset>".to_string())
}
