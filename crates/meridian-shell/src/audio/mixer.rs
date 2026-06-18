//! FreeBSD audio backend: the base `mixer(8)` over OSS (`/dev/mixerN`).
//!
//! The FreeBSD PipeWire package ships no ALSA/OSS SPA plugin, so `wpctl` sees no
//! devices. mixer(8) instead reads and writes channel volume on each sound unit
//! directly, and `hw.snd.default_unit` selects the active playback device.

use std::process::Command;

use super::{AudioDevice, AudioServiceState, AudioSnapshot};

pub(super) fn snapshot() -> AudioSnapshot {
    let Some(stat) = run(&["cat", "/dev/sndstat"]).or_else(read_sndstat) else {
        return AudioSnapshot::unavailable();
    };
    let units = parse_sndstat_units(&stat);
    if units.is_empty() {
        return AudioSnapshot::unavailable();
    }
    let default = default_unit();
    let outputs: Vec<AudioDevice> = units
        .iter()
        .map(|&unit| read_device(unit, default))
        .collect();
    let default_output = outputs.iter().find(|d| d.is_default).cloned();
    AudioSnapshot {
        service: AudioServiceState::Running,
        default_output,
        default_input: None,
        outputs,
        inputs: Vec::new(),
    }
}

pub(super) fn set_volume(percent: u8) {
    let unit = default_unit();
    let level = (percent.min(100) as f32) / 100.0;
    let _ = run_mixer(unit, &[&format!("vol={:.2}", level)]);
}

pub(super) fn toggle_mute() {
    let unit = default_unit();
    let (_, muted) = read_volume(unit);
    let next = if muted { "vol.mute=off" } else { "vol.mute=on" };
    let _ = run_mixer(unit, &[next]);
}

pub(super) fn set_default(id: u32) {
    // The OSS default playback device is a sysctl; the session runs with enough
    // privilege to set it. Best-effort.
    match Command::new("sysctl")
        .arg(format!("hw.snd.default_unit={id}"))
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => tracing::warn!("sysctl hw.snd.default_unit={id} exited with {status}"),
        Err(err) => tracing::warn!("failed to set default audio unit {id}: {err}"),
    }
}

fn read_device(unit: u32, default: u32) -> AudioDevice {
    let name = device_name(unit).unwrap_or_else(|| format!("pcm{unit}"));
    let (volume_percent, muted) = read_volume(unit);
    AudioDevice {
        id: unit,
        name,
        volume_percent,
        muted,
        is_default: unit == default,
    }
}

/// Read `vol` volume/mute for `unit` non-destructively via `mixer -f`.
fn read_volume(unit: u32) -> (Option<u8>, bool) {
    run_mixer(unit, &["vol"])
        .as_deref()
        .map(parse_mixer_vol)
        .unwrap_or((None, false))
}

/// Parse `mixer ... vol` output:
/// ```text
/// vol.volume=0.85:0.85
/// vol.mute=off
/// ```
/// The volume is the left channel scaled to a 0..=100 percent.
fn parse_mixer_vol(output: &str) -> (Option<u8>, bool) {
    let mut volume = None;
    let mut muted = false;
    for line in output.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("vol.volume=") {
            if let Some(level) = value
                .split(':')
                .next()
                .and_then(|v| v.trim().parse::<f32>().ok())
            {
                volume = Some((level * 100.0).round().clamp(0.0, 100.0) as u8);
            }
        } else if let Some(state) = line.strip_prefix("vol.mute=") {
            muted = state.trim() == "on";
        }
    }
    (volume, muted)
}

/// Parse the playback units from `/dev/sndstat`:
/// `pcm0: <Realtek ALC255 (Internal Analog)> (play/rec) default`
fn parse_sndstat_units(output: &str) -> Vec<u32> {
    let mut units = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("pcm") else {
            continue;
        };
        let Some((num, tail)) = rest.split_once(':') else {
            continue;
        };
        let Ok(unit) = num.trim().parse::<u32>() else {
            continue;
        };
        if tail.contains("play") && !units.contains(&unit) {
            units.push(unit);
        }
    }
    units
}

fn default_unit() -> u32 {
    run(&["sysctl", "-n", "hw.snd.default_unit"])
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0)
}

fn device_name(unit: u32) -> Option<String> {
    run(&["sysctl", "-n", &format!("dev.pcm.{unit}.%desc")]).map(|s| s.trim().to_string())
}

fn run_mixer(unit: u32, args: &[&str]) -> Option<String> {
    let device = format!("/dev/mixer{unit}");
    let mut cmd = Command::new("mixer");
    cmd.env("LC_ALL", "C").arg("-f").arg(&device).args(args);
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn run(args: &[&str]) -> Option<String> {
    let (program, rest) = args.split_first()?;
    let output = Command::new(program)
        .env("LC_ALL", "C")
        .args(rest)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn read_sndstat() -> Option<String> {
    std::fs::read_to_string("/dev/sndstat").ok()
}

#[cfg(test)]
mod tests {
    use super::{parse_mixer_vol, parse_sndstat_units};

    #[test]
    fn mixer_vol_parses_level_and_mute() {
        assert_eq!(
            parse_mixer_vol("vol.volume=0.85:0.85\nvol.mute=off\n"),
            (Some(85), false)
        );
        assert_eq!(
            parse_mixer_vol("vol.volume=0.50:0.40\nvol.mute=on\n"),
            (Some(50), true)
        );
        // Tolerates the `default_unit: X -> Y` noise line from the -d form.
        assert_eq!(
            parse_mixer_vol("default_unit: 0 -> 1\nvol.volume=1.00:1.00\nvol.mute=off"),
            (Some(100), false)
        );
    }

    #[test]
    fn mixer_vol_missing_fields_are_safe() {
        assert_eq!(parse_mixer_vol("garbage\n"), (None, false));
    }

    #[test]
    fn sndstat_lists_playback_units() {
        let stat = "\
Installed devices:
pcm0: <Realtek ALC255 (Internal Analog)> (play/rec) default
pcm1: <Realtek ALC255 (Front Analog Headphones)> (play)
pcm2: <Intel Kaby Lake (HDMI/DP 8ch)> (play)
No devices installed from userspace.
";
        assert_eq!(parse_sndstat_units(stat), vec![0, 1, 2]);
    }

    #[test]
    fn sndstat_skips_record_only_and_dedups() {
        let stat = "\
pcm0: <Mic> (rec)
pcm3: <Speakers> (play)
pcm3: <Speakers> (play)
";
        assert_eq!(parse_sndstat_units(stat), vec![3]);
    }
}
