use std::env;

use super::init_env::env_flag_enabled;

pub(super) fn select_add_mode(
    modes: &[smithay::reexports::drm::control::Mode],
) -> Option<(smithay::reexports::drm::control::Mode, String)> {
    if env_flag_enabled("MERIDIAN_DRM_SAFE_MODE") {
        if let Some(mode) = select_safe_mode(modes) {
            return Some((mode, "safe-mode".to_string()));
        }
    }

    if let Some(index) = forced_mode_index_from_env() {
        if let Some(mode) = modes.get(index).copied() {
            return Some((mode, format!("forced-index({})", index)));
        }
        tracing::warn!(
            "MERIDIAN_DRM_MODE_INDEX={} out of range (available modes={})",
            index,
            modes.len()
        );
    }

    if let Some((force_w, force_h)) = forced_mode_size_from_env() {
        let same_size: Vec<_> = modes
            .iter()
            .copied()
            .filter(|mode| mode.size() == (force_w, force_h))
            .collect();
        if same_size.len() > 1 {
            tracing::info!(
                "drm mode candidates for forced-size {}x{}: count={} candidates={:?}",
                force_w,
                force_h,
                same_size.len(),
                same_size
                    .iter()
                    .map(|mode| mode_brief(*mode))
                    .collect::<Vec<_>>()
            );
        }
        if let Some(forced_mode) = select_best_mode_for_size(modes, force_w, force_h) {
            return Some((forced_mode, format!("forced-size({}x{})", force_w, force_h)));
        }
        tracing::warn!(
            "requested drm mode {}x{} not found in connector mode list",
            force_w,
            force_h
        );
    }
    if let Some(preferred) = modes.iter().copied().find(|mode| {
        mode.mode_type()
            .contains(smithay::reexports::drm::control::ModeTypeFlags::PREFERRED)
    }) {
        let (pref_w, pref_h) = preferred.size();
        let same_size: Vec<_> = modes
            .iter()
            .copied()
            .filter(|mode| mode.size() == (pref_w, pref_h))
            .collect();
        if same_size.len() > 1 {
            tracing::debug!(
                "drm preferred-size candidates {}x{}: count={} candidates={:?}",
                pref_w,
                pref_h,
                same_size.len(),
                same_size
                    .iter()
                    .map(|mode| mode_brief(*mode))
                    .collect::<Vec<_>>()
            );
        }
        if let Some(selected_mode) = select_best_mode_for_size(modes, pref_w, pref_h) {
            if !same_mode(preferred, selected_mode) {
                tracing::debug!(
                    "drm preferred mode adjusted to best same-size refresh candidate: preferred={} selected={}",
                    mode_brief(preferred),
                    mode_brief(selected_mode)
                );
            }
            return Some((selected_mode, "preferred-size-best-refresh".to_string()));
        }
        return Some((preferred, "preferred".to_string()));
    }
    modes
        .first()
        .copied()
        .map(|mode| (mode, "safe-fallback-first".to_string()))
}

fn mode_flags_weird_penalty(mode: smithay::reexports::drm::control::Mode) -> u8 {
    let flags = format!("{:?}", mode.flags()).to_ascii_uppercase();
    let mut penalty = 0_u8;
    for token in ["INTERLACE", "DBLSCAN", "DOUBLESCAN", "3D"] {
        if flags.contains(token) {
            penalty = penalty.saturating_add(1);
        }
    }
    penalty
}

pub(super) fn calculate_mode_refresh_millihz(
    mode: smithay::reexports::drm::control::Mode,
) -> Option<i32> {
    let clock_khz = mode.clock();
    let htotal = mode.hsync().2 as i64;
    let vtotal = mode.vsync().2 as i64;
    if clock_khz == 0 || htotal <= 0 || vtotal <= 0 {
        return None;
    }

    let mut refresh_millihz = u128::from(clock_khz)
        .checked_mul(1_000_000)?
        .checked_div(htotal as u128)?
        .checked_div(vtotal as u128)?;

    let flags = mode.flags();
    if flags.contains(smithay::reexports::drm::control::ModeFlags::INTERLACE) {
        refresh_millihz = refresh_millihz.checked_mul(2)?;
    }
    if flags.contains(smithay::reexports::drm::control::ModeFlags::DBLSCAN) {
        refresh_millihz /= 2;
    }

    let vscan = mode.vscan() as u128;
    if vscan > 1 {
        refresh_millihz /= vscan;
    }

    i32::try_from(refresh_millihz).ok()
}

pub(super) fn mode_refresh_millihz_with_fallback(
    mode: smithay::reexports::drm::control::Mode,
) -> i32 {
    calculate_mode_refresh_millihz(mode).unwrap_or_else(|| mode.vrefresh() as i32 * 1000)
}

pub(super) fn mode_conservative_key(
    mode: smithay::reexports::drm::control::Mode,
) -> (u8, u32, u32, u8) {
    let refresh = mode.vrefresh();
    let exact_60_penalty = if refresh == 60 { 0 } else { 1 };
    let refresh_distance = refresh.abs_diff(60);
    (
        exact_60_penalty,
        refresh_distance,
        mode.clock(),
        mode_flags_weird_penalty(mode),
    )
}

fn same_mode(
    a: smithay::reexports::drm::control::Mode,
    b: smithay::reexports::drm::control::Mode,
) -> bool {
    a.size() == b.size()
        && a.clock() == b.clock()
        && a.vrefresh() == b.vrefresh()
        && a.flags() == b.flags()
        && a.mode_type() == b.mode_type()
}

fn mode_brief(mode: smithay::reexports::drm::control::Mode) -> String {
    let (w, h) = mode.size();
    let calc_refresh_millihz = calculate_mode_refresh_millihz(mode);
    format!(
        "{}x{}@{}Hz mclock_khz={} calc_refresh_millihz={:?} flags={:?} mode_type={:?}",
        w,
        h,
        mode.vrefresh(),
        mode.clock(),
        calc_refresh_millihz,
        mode.flags(),
        mode.mode_type()
    )
}

fn select_best_mode_for_size(
    modes: &[smithay::reexports::drm::control::Mode],
    width: u16,
    height: u16,
) -> Option<smithay::reexports::drm::control::Mode> {
    modes
        .iter()
        .copied()
        .filter(|mode| mode.size() == (width, height))
        .max_by_key(|mode| {
            (
                mode_refresh_millihz_with_fallback(*mode),
                u8::MAX - mode_flags_weird_penalty(*mode),
                mode.clock(),
            )
        })
}

fn select_safe_mode(
    modes: &[smithay::reexports::drm::control::Mode],
) -> Option<smithay::reexports::drm::control::Mode> {
    let mut candidates: Vec<_> = modes
        .iter()
        .copied()
        .filter(|mode| {
            let (w, h) = mode.size();
            w >= 1920 && h >= 1080
        })
        .collect();
    if candidates.is_empty() {
        candidates.extend(modes.iter().copied());
    }
    candidates.into_iter().min_by_key(|mode| {
        let (w, h) = mode.size();
        (
            w as u32 * h as u32,
            mode_conservative_key(*mode),
            mode.vrefresh(),
            mode.clock(),
        )
    })
}

fn parse_mode_size(value: &str) -> Option<(u16, u16)> {
    let trimmed = value.trim().to_ascii_lowercase();
    let (w, h) = trimmed.split_once('x')?;
    let width = w.parse::<u16>().ok()?;
    let height = h.parse::<u16>().ok()?;
    Some((width, height))
}

pub(super) fn forced_mode_size_from_env() -> Option<(u16, u16)> {
    if let Ok(value) = env::var("MERIDIAN_DRM_MODE") {
        return parse_mode_size(&value);
    }
    env::var("MERIDIAN_DRM_FORCE_MODE")
        .ok()
        .and_then(|value| parse_mode_size(&value))
}

pub(super) fn forced_mode_index_from_env() -> Option<usize> {
    env::var("MERIDIAN_DRM_MODE_INDEX")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
}
