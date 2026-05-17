use std::env;

use meridian_config::OutputModeConfig;

use super::init_env::env_flag_enabled;

pub(crate) fn select_add_mode(
    modes: &[smithay::reexports::drm::control::Mode],
) -> Option<(smithay::reexports::drm::control::Mode, String)> {
    tracing::info!(
        "drm mode-selection start: candidates={} env_safe_mode={} env_forced_index={:?} env_forced_size={:?}",
        modes.len(),
        env_flag_enabled("MERIDIAN_DRM_SAFE_MODE"),
        forced_mode_index_from_env(),
        forced_mode_size_from_env(),
    );
    for (i, mode) in modes.iter().enumerate() {
        let (w, h) = mode.size();
        let pref = mode
            .mode_type()
            .contains(smithay::reexports::drm::control::ModeTypeFlags::PREFERRED);
        tracing::info!(
            "drm mode[{}]: {}x{}@{}Hz preferred={} clock_khz={} flags={:?} mode_type={:?}",
            i,
            w,
            h,
            mode.vrefresh(),
            pref,
            mode.clock(),
            mode.flags(),
            mode.mode_type(),
        );
    }

    if env_flag_enabled("MERIDIAN_DRM_SAFE_MODE") {
        if let Some(mode) = select_safe_mode(modes) {
            let reason = "safe-mode".to_string();
            log_selected(&reason, mode);
            return Some((mode, reason));
        }
    }

    if let Some(index) = forced_mode_index_from_env() {
        if let Some(mode) = modes.get(index).copied() {
            let reason = format!("forced-index({})", index);
            log_selected(&reason, mode);
            return Some((mode, reason));
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
            let reason = format!("forced-size({}x{})", force_w, force_h);
            log_selected(&reason, forced_mode);
            return Some((forced_mode, reason));
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
            let reason = "preferred-size-best-refresh".to_string();
            log_selected(&reason, selected_mode);
            return Some((selected_mode, reason));
        }
        let reason = "preferred".to_string();
        log_selected(&reason, preferred);
        return Some((preferred, reason));
    }
    let fallback = modes.first().copied().map(|mode| {
        let reason = "safe-fallback-first".to_string();
        log_selected(&reason, mode);
        (mode, reason)
    });
    if fallback.is_none() {
        tracing::info!("drm mode-selection result: none reason=no-candidates");
    }
    fallback
}

pub(crate) fn select_mode_with_override(
    modes: &[smithay::reexports::drm::control::Mode],
    override_mode: Option<&OutputModeConfig>,
    output_name: &str,
) -> Option<(smithay::reexports::drm::control::Mode, String)> {
    if let Some(requested) = override_mode {
        let req_w = match u16::try_from(requested.width) {
            Ok(value) => value,
            Err(_) => {
                tracing::warn!(
                    "output {} TOML mode width {} invalid for drm mode matching — falling back to auto selection",
                    output_name,
                    requested.width
                );
                return select_add_mode(modes);
            }
        };
        let req_h = match u16::try_from(requested.height) {
            Ok(value) => value,
            Err(_) => {
                tracing::warn!(
                    "output {} TOML mode height {} invalid for drm mode matching — falling back to auto selection",
                    output_name,
                    requested.height
                );
                return select_add_mode(modes);
            }
        };

        let candidates = modes
            .iter()
            .map(|mode| {
                let (width, height) = mode.size();
                (width, height, mode_refresh_millihz_with_fallback(*mode))
            })
            .collect::<Vec<_>>();

        if let Some(index) =
            pick_best_mode_for_request(&candidates, (req_w, req_h, requested.refresh_millihz))
        {
            let selected = modes[index];
            return Some((
                selected,
                format!("toml-override({}x{})", requested.width, requested.height),
            ));
        }

        tracing::warn!(
            "output {} TOML mode {}x{}@{:?} not found in connector modes — falling back to auto selection",
            output_name,
            requested.width,
            requested.height,
            requested.refresh_millihz
        );
    }

    select_add_mode(modes)
}

fn pick_best_mode_for_request(
    candidates: &[(u16, u16, i32)],
    request: (u16, u16, Option<i32>),
) -> Option<usize> {
    let (req_w, req_h, req_refresh) = request;
    let matching = candidates
        .iter()
        .enumerate()
        .filter(|(_, (width, height, _))| *width == req_w && *height == req_h)
        .collect::<Vec<_>>();

    if matching.is_empty() {
        return None;
    }

    if let Some(target_refresh) = req_refresh {
        return matching
            .into_iter()
            .min_by_key(|(_, (_, _, refresh))| (*refresh - target_refresh).abs())
            .map(|(index, _)| index);
    }

    matching
        .into_iter()
        .max_by_key(|(_, (_, _, refresh))| *refresh)
        .map(|(index, _)| index)
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

pub(crate) fn calculate_mode_refresh_millihz(
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

pub(crate) fn mode_refresh_millihz_with_fallback(
    mode: smithay::reexports::drm::control::Mode,
) -> i32 {
    calculate_mode_refresh_millihz(mode).unwrap_or_else(|| mode.vrefresh() as i32 * 1000)
}

pub(super) fn mode_conservative_key(
    mode: smithay::reexports::drm::control::Mode,
) -> (i64, i64, i64, i64) {
    let refresh_millihz = mode_refresh_millihz_with_fallback(mode) as i64;
    (
        i64::MAX - refresh_millihz,
        mode_flags_weird_penalty(mode) as i64,
        -(mode.clock() as i64),
        0,
    )
}

fn log_selected(tag: &str, mode: smithay::reexports::drm::control::Mode) {
    let (w, h) = mode.size();
    tracing::info!(
        "drm mode-selection result: {}x{}@{}Hz reason={}",
        w,
        h,
        mode.vrefresh(),
        tag,
    );
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

#[cfg(test)]
mod tests {
    use meridian_config::OutputModeConfig;

    use super::{pick_best_mode_for_request, select_add_mode, select_mode_with_override};

    #[test]
    fn toml_mode_override_picks_matching_size() {
        let candidates = vec![(1920, 1080, 60_000), (2560, 1440, 144_000)];
        let index = pick_best_mode_for_request(&candidates, (2560, 1440, None));
        assert_eq!(index, Some(1));
    }

    #[test]
    fn toml_mode_override_picks_closest_refresh_when_specified() {
        let candidates = vec![
            (1920, 1080, 60_000),
            (1920, 1080, 143_000),
            (1920, 1080, 165_000),
        ];
        let index = pick_best_mode_for_request(&candidates, (1920, 1080, Some(144_000)));
        assert_eq!(index, Some(1));
    }

    #[test]
    fn toml_mode_override_falls_back_when_size_unavailable() {
        let candidates = vec![(1920, 1080, 60_000), (2560, 1440, 144_000)];
        let index = pick_best_mode_for_request(&candidates, (1280, 720, Some(60_000)));
        assert_eq!(index, None);
    }

    #[test]
    fn toml_mode_override_none_delegates_to_select_add_mode() {
        let modes: Vec<smithay::reexports::drm::control::Mode> = Vec::new();
        let delegated = select_add_mode(&modes);
        let selected = select_mode_with_override(&modes, None, "drm-0");
        assert!(delegated.is_none());
        assert!(selected.is_none());
    }

    #[test]
    fn toml_mode_override_invalid_dimensions_delegate_to_auto_selection() {
        let modes: Vec<smithay::reexports::drm::control::Mode> = Vec::new();
        let override_mode = OutputModeConfig {
            width: -1,
            height: 1080,
            refresh_millihz: None,
        };
        assert!(select_add_mode(&modes).is_none());
        assert!(select_mode_with_override(&modes, Some(&override_mode), "drm-0").is_none());
    }
}
