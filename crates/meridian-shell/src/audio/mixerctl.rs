//! OpenBSD audio backend: the base `mixerctl(8)` over `/dev/audioN`/
//! `/dev/audioctlN` (P1-3, AUDIT_2026-08-19).
//!
//! Neither PipeWire nor FreeBSD's `mixer(8)` exist on OpenBSD; `mixerctl(8)`
//! reads and writes the kernel audio mixer controls directly. The shell user
//! needs access to the audio devices (group `_sndiop`), otherwise every call
//! fails and the snapshot reports "unavailable" — the same graceful path as a
//! missing backend.

use std::process::Command;

use super::{AudioDevice, AudioServiceState, AudioSnapshot};

/// Hard deadline for mixerctl calls on the event loop (P2-1 helper).
const MIXERCTL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Controls tried in order before falling back to the first numeric
/// `outputs.*` control. Hardware usually exposes `outputs.master`; some
/// codecs only provide e.g. `outputs.dac`.
const PREFERRED_VOLUME_CONTROLS: &[&str] = &[
    "outputs.master",
    "outputs.dac",
    "outputs.speaker",
    "outputs.spkr",
    "outputs.volume",
];

/// Mixer value range assumed when `mixerctl -v` does not print one. The
/// azalia(4) hardware on the reference laptop uses 0..255 and out-of-range
/// writes are clamped by the kernel, so the assumption is safe.
const FALLBACK_MIXER_MAX: u32 = 255;

pub(super) fn snapshot() -> AudioSnapshot {
    let Some(output) = run(&["mixerctl"]) else {
        return AudioSnapshot::unavailable();
    };
    let controls = parse_controls(&output);
    let Some(volume_control) = select_volume_control(&controls).map(str::to_string) else {
        return AudioSnapshot::unavailable();
    };
    let max = control_max(&volume_control).unwrap_or(FALLBACK_MIXER_MAX);
    let volume_percent = controls
        .iter()
        .find(|(name, _)| *name == volume_control)
        .and_then(|(_, value)| first_channel_value(value))
        .map(|value| scale_percent(value, max));
    let device = AudioDevice {
        id: 0,
        name: display_name(&volume_control),
        volume_percent,
        muted: is_muted(&controls, &volume_control),
        is_default: true,
    };
    AudioSnapshot {
        service: AudioServiceState::Running,
        default_output: Some(device.clone()),
        default_input: None,
        outputs: vec![device],
        inputs: Vec::new(),
    }
}

pub(super) fn set_volume(percent: u8) {
    let Some(control) = find_volume_control() else {
        tracing::debug!("mixerctl: no playback volume control available");
        return;
    };
    let max = control_max(&control).unwrap_or(FALLBACK_MIXER_MAX);
    let value = ((percent.min(100) as u64 * max as u64) / 100).min(max as u64);
    // A single value is applied to every channel.
    let _ = run(&["mixerctl", &format!("{control}={value}")]);
}

pub(super) fn toggle_mute() {
    let Some(output) = run(&["mixerctl"]) else {
        return;
    };
    let controls = parse_controls(&output);
    let Some(control) = select_volume_control(&controls).map(str::to_string) else {
        return;
    };
    let mute_control = format!("{control}.mute");
    if !controls.iter().any(|(name, _)| *name == mute_control) {
        tracing::debug!("mixerctl: no mute control for {control}");
        return;
    }
    let next = if is_muted(&controls, &control) {
        "off"
    } else {
        "on"
    };
    let _ = run(&["mixerctl", &format!("{mute_control}={next}")]);
}

pub(super) fn set_default(_id: u32) {
    // OpenBSD routes playback through the audio(4) device node (/dev/audioN)
    // or sndiod(8); there is no mixerctl-level "default output" switch.
    tracing::debug!("mixerctl: choosing a default output is not supported on OpenBSD");
}

/// Re-read the control list and pick the playback volume control.
fn find_volume_control() -> Option<String> {
    let output = run(&["mixerctl"])?;
    let controls = parse_controls(&output);
    select_volume_control(&controls).map(str::to_string)
}

/// Query the control's value range via `mixerctl -v`; `None` when it is not
/// printed (the caller falls back to [`FALLBACK_MIXER_MAX`]).
fn control_max(control: &str) -> Option<u32> {
    let output = run(&["mixerctl", "-v", control])?;
    parse_max_from_verbose(&output)
}

/// Parse `mixerctl` output into `(name, value)` pairs. Lines look like
/// `outputs.master=126,126`; control names may contain dots, colons and
/// dashes, values may contain commas.
fn parse_controls(output: &str) -> Vec<(String, String)> {
    output
        .lines()
        .filter_map(|line| {
            let (name, value) = line.trim().split_once('=')?;
            let name = name.trim();
            let value = value.trim();
            if name.is_empty() || value.is_empty() {
                return None;
            }
            Some((name.to_string(), value.to_string()))
        })
        .collect()
}

/// Pick the control that carries the playback volume: the preferred names
/// first, then the first `outputs.*` control with a numeric value (this skips
/// enums like `outputs.hp_source=mix` and the `*.mute`/`*.slaves` siblings).
fn select_volume_control(controls: &[(String, String)]) -> Option<&str> {
    for preferred in PREFERRED_VOLUME_CONTROLS {
        if controls
            .iter()
            .any(|(name, value)| name == preferred && is_numeric_value(value))
        {
            return Some(preferred);
        }
    }
    controls
        .iter()
        .find(|(name, value)| {
            name.starts_with("outputs.") && is_numeric_value(value) && !name.contains(".mute")
        })
        .map(|(name, _)| name.as_str())
}

/// `126,126` -> the first channel's value; `off`/`mic` -> `None`.
fn first_channel_value(value: &str) -> Option<u32> {
    value.split(',').next()?.trim().parse::<u32>().ok()
}

fn is_numeric_value(value: &str) -> bool {
    first_channel_value(value).is_some()
}

/// Scale a raw mixer value (0..=max) to 0..=100 percent.
fn scale_percent(value: u32, max: u32) -> u8 {
    if max == 0 {
        return 0;
    }
    ((value as f64 * 100.0) / max as f64)
        .round()
        .clamp(0.0, 100.0) as u8
}

/// True when `<control>.mute` exists and its first channel reads "on".
fn is_muted(controls: &[(String, String)], control: &str) -> bool {
    let mute = format!("{control}.mute");
    controls
        .iter()
        .find(|(name, _)| *name == mute)
        .map(|(_, value)| value.split(',').next().map(str::trim) == Some("on"))
        .unwrap_or(false)
}

/// `outputs.master` -> "Master" for the settings UI.
fn display_name(control: &str) -> String {
    let tail = control.rsplit('.').next().filter(|s| !s.is_empty());
    let Some(tail) = tail else {
        return "Audio".to_string();
    };
    let mut chars = tail.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Audio".to_string(),
    }
}

/// Parse the trailing `[0..255]` range from `mixerctl -v` output; many
/// drivers print only the current value, in which case this returns `None`.
fn parse_max_from_verbose(output: &str) -> Option<u32> {
    let open = output.rfind('[')?;
    let close = output.rfind(']')?;
    if close <= open {
        return None;
    }
    let (min_text, max_text) = output[open + 1..close].split_once("..")?;
    let _min: i64 = min_text.trim().parse().ok()?;
    max_text.trim().parse().ok()
}

fn run(args: &[&str]) -> Option<String> {
    let (program, rest) = args.split_first()?;
    let mut cmd = Command::new(program);
    cmd.env("LC_ALL", "C").args(rest);
    let output = crate::process::output_with_timeout(&mut cmd, MIXERCTL_TIMEOUT)?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        display_name, first_channel_value, is_muted, parse_controls, parse_max_from_verbose,
        scale_percent, select_volume_control,
    };

    /// Real azalia(4) output shape from the reference laptop (2026-08-20).
    const AZALIA_OUTPUT: &str = "\
inputs.dac-2:3=126,126
inputs.dac-0:1=126,126
record.adc-0:1_mute=off
record.adc-0:1=124,124
inputs.mix_source=dac-2:3
inputs.mix2_source=dac-0:1
inputs.mic=85,85
outputs.spkr_source=mix
outputs.spkr_mute=off
outputs.spkr_eapd=on
outputs.hp_source=mix2
outputs.hp_mute=off
outputs.hp_boost=off
outputs.hp_eapd=on
record.adc-0:1_source=mic
outputs.hp_sense=unplugged
outputs.spkr_muters=hp
outputs.master=126,126
outputs.master.mute=off
outputs.master.slaves=dac-2:3,dac-0:1,spkr,hp
";

    #[test]
    fn parses_real_azalia_control_list() {
        let controls = parse_controls(AZALIA_OUTPUT);
        assert!(controls
            .iter()
            .any(|(name, value)| name == "outputs.master" && value == "126,126"));
        assert!(controls
            .iter()
            .any(|(name, value)| name == "outputs.master.mute" && value == "off"));
        // Names with colons/dashes survive too.
        assert!(controls
            .iter()
            .any(|(name, value)| name == "record.adc-0:1" && value == "124,124"));
    }

    #[test]
    fn selects_master_over_other_output_controls() {
        let controls = parse_controls(AZALIA_OUTPUT);
        assert_eq!(select_volume_control(&controls), Some("outputs.master"));
    }

    #[test]
    fn falls_back_to_first_numeric_output_control_without_master() {
        let controls =
            parse_controls("outputs.dac=200,200\noutputs.hp_source=mix\noutputs.dac.mute=off\n");
        assert_eq!(select_volume_control(&controls), Some("outputs.dac"));
    }

    #[test]
    fn skips_enum_and_mute_controls_when_falling_back() {
        let controls = parse_controls("outputs.hp_source=mix\noutputs.hp_mute=off\n");
        assert_eq!(select_volume_control(&controls), None);
    }

    #[test]
    fn empty_control_list_selects_nothing() {
        assert_eq!(select_volume_control(&[]), None);
    }
    #[test]
    fn first_channel_value_reads_left_channel_and_rejects_enums() {
        assert_eq!(first_channel_value("126,126"), Some(126));
        assert_eq!(first_channel_value("0"), Some(0));
        assert_eq!(first_channel_value("off"), None);
        assert_eq!(first_channel_value("mic"), None);
    }

    #[test]
    fn scales_percent_against_control_max() {
        assert_eq!(scale_percent(126, 255), 49);
        assert_eq!(scale_percent(0, 255), 0);
        assert_eq!(scale_percent(255, 255), 100);
        assert_eq!(scale_percent(9, 0), 0);
    }

    #[test]
    fn muted_reads_first_channel_of_mute_control() {
        let controls = parse_controls("outputs.master=1,1\noutputs.master.mute=on\n");
        assert!(is_muted(&controls, "outputs.master"));
        let controls = parse_controls("outputs.master=1,1\noutputs.master.mute=off\n");
        assert!(!is_muted(&controls, "outputs.master"));
        let controls = parse_controls("outputs.master=1,1\n");
        assert!(!is_muted(&controls, "outputs.master"));
    }

    #[test]
    fn display_name_capitalises_control_tail() {
        assert_eq!(display_name("outputs.master"), "Master");
        assert_eq!(display_name("outputs.dac"), "Dac");
    }

    #[test]
    fn parses_verbose_range_when_present() {
        assert_eq!(
            parse_max_from_verbose("outputs.master=126,126 [0..255]\n"),
            Some(255)
        );
        assert_eq!(parse_max_from_verbose("outputs.master=126,126\n"), None);
    }
}
