//! Linux audio backend: PipeWire via the `wpctl` CLI.

use std::process::Command;

use super::{AudioDevice, AudioServiceState, AudioSnapshot};

pub(super) fn snapshot() -> AudioSnapshot {
    let Some(output) = run_wpctl_status() else {
        return AudioSnapshot::unavailable();
    };
    parse_wpctl_status(&output)
}

pub(super) fn set_volume(percent: u8) {
    run_wpctl(&set_volume_args(percent));
}

pub(super) fn toggle_mute() {
    run_wpctl(&set_mute_args());
}

pub(super) fn set_default(id: u32) {
    run_wpctl(&set_default_args(id));
}

fn parse_wpctl_status(output: &str) -> AudioSnapshot {
    let outputs = parse_section_devices(output, "Sinks:");
    let inputs = parse_section_devices(output, "Sources:");
    let default_output = outputs.iter().find(|device| device.is_default).cloned();
    let default_input = inputs.iter().find(|device| device.is_default).cloned();

    AudioSnapshot {
        service: AudioServiceState::Running,
        default_output,
        default_input,
        outputs,
        inputs,
    }
}

fn parse_section_devices(output: &str, section: &str) -> Vec<AudioDevice> {
    let mut in_section = false;
    let mut devices = Vec::new();
    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.ends_with(section) {
            in_section = true;
            continue;
        }
        if !in_section {
            continue;
        }
        if line.ends_with(':') {
            break;
        }
        if let Some(device) = parse_device_line(line) {
            devices.push(device);
        }
    }
    devices
}

fn parse_device_line(line: &str) -> Option<AudioDevice> {
    // `wpctl status` draws its tree with Unicode box characters (│ ├ └ ─,
    // U+2500..U+2502) as well as the ASCII variants older builds used. Strip
    // both so the leading marker collapses to the `*` default flag / numeric id.
    let trimmed = line
        .trim_start_matches(['|', '`', '-', ' ', '\t', '│', '├', '└', '─'])
        .trim();
    let is_default = trimmed.starts_with('*');
    let trimmed = trimmed.trim_start_matches('*').trim();
    let (id_text, rest) = trimmed.split_once('.')?;
    let id = id_text.trim().parse::<u32>().ok()?;
    let rest = rest.trim();
    let (name, meta) = if let Some((name, meta)) = rest.rsplit_once('[') {
        (name.trim(), Some(meta.trim_end_matches(']').trim()))
    } else {
        (rest, None)
    };
    if name.is_empty() {
        return None;
    }
    let volume_percent = meta.and_then(parse_volume_percent);
    let muted = meta
        .map(|meta| meta.to_ascii_lowercase().contains("muted"))
        .unwrap_or(false);
    Some(AudioDevice {
        id,
        name: name.to_string(),
        volume_percent,
        muted,
        is_default,
    })
}

fn parse_volume_percent(meta: &str) -> Option<u8> {
    let (_, tail) = meta.split_once("vol:")?;
    let number = tail.split_whitespace().next()?;
    let value = number.trim().parse::<f32>().ok()?;
    Some((value * 100.0).round().clamp(0.0, 150.0) as u8)
}

fn run_wpctl_status() -> Option<String> {
    let output = Command::new("wpctl")
        .env("LC_ALL", "C")
        .arg("status")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// The default-sink target understood by wpctl. Targeting the symbolic default
/// (rather than a numeric id baked into a widget id) keeps the write robust
/// across device hotplug and re-poll.
const DEFAULT_SINK: &str = "@DEFAULT_AUDIO_SINK@";

fn set_volume_args(percent: u8) -> Vec<String> {
    let level = (percent.min(100) as f32) / 100.0;
    vec![
        "set-volume".to_string(),
        DEFAULT_SINK.to_string(),
        format!("{:.2}", level),
    ]
}

fn set_mute_args() -> Vec<String> {
    vec![
        "set-mute".to_string(),
        DEFAULT_SINK.to_string(),
        "toggle".to_string(),
    ]
}

fn set_default_args(id: u32) -> Vec<String> {
    vec!["set-default".to_string(), id.to_string()]
}

fn run_wpctl(args: &[String]) {
    match Command::new("wpctl").env("LC_ALL", "C").args(args).status() {
        Ok(status) if status.success() => {}
        Ok(status) => tracing::warn!("wpctl {:?} exited with {}", args, status),
        Err(err) => tracing::warn!("failed to run wpctl {:?}: {}", args, err),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_wpctl_status, set_default_args, set_mute_args, set_volume_args};

    #[test]
    fn parse_wpctl_status_extracts_default_sink_and_source() {
        let snapshot = parse_wpctl_status(
            "Audio\n  Sinks:\n  *   43. Built-in Audio Analog Stereo [vol: 0.58]\n      44. HDMI Audio [vol: 0.18 MUTED]\n  Sources:\n  *   55. Built-in Microphone [vol: 1.00]\n  Filters:\n",
        );

        let output = snapshot.default_output.as_ref().unwrap();
        assert_eq!(output.id, 43);
        assert_eq!(output.name, "Built-in Audio Analog Stereo");
        assert_eq!(output.volume_percent, Some(58));
        assert!(!output.muted);
        assert_eq!(snapshot.outputs.len(), 2);
        assert_eq!(snapshot.outputs[1].volume_percent, Some(18));
        assert!(snapshot.outputs[1].muted);

        let input = snapshot.default_input.as_ref().unwrap();
        assert_eq!(input.name, "Built-in Microphone");
        assert_eq!(input.volume_percent, Some(100));
    }

    #[test]
    fn parse_wpctl_status_handles_real_unicode_tree_output() {
        // Verbatim shape of `wpctl status` (WirePlumber 1.x): the device tree is
        // drawn with Unicode box characters (│ ├ └ ─) and a blank `│` separator
        // line between each section. Earlier the leading `│` defeated id parsing,
        // so every device dropped and the panel reported no audio.
        let snapshot = parse_wpctl_status(
            "Audio\n \u{251c}\u{2500} Devices:\n \u{2502}      42. Internes Audio  [alsa]\n \u{2502}  \n \u{251c}\u{2500} Sinks:\n \u{2502}  *   49. Internes Audio Analoges Stereo      [vol: 0.55]\n \u{2502}  \n \u{251c}\u{2500} Sources:\n \u{2502}  *   50. Internes Audio Analoges Stereo      [vol: 1.00]\n \u{2502}  \n \u{2514}\u{2500} Streams:\n",
        );

        let output = snapshot.default_output.as_ref().unwrap();
        assert_eq!(output.id, 49);
        assert_eq!(output.name, "Internes Audio Analoges Stereo");
        assert_eq!(output.volume_percent, Some(55));
        assert!(output.is_default);
        assert!(!output.muted);
        assert_eq!(snapshot.outputs.len(), 1);

        let input = snapshot.default_input.as_ref().unwrap();
        assert_eq!(input.id, 50);
        assert_eq!(input.volume_percent, Some(100));
    }

    #[test]
    fn set_volume_args_targets_default_sink_with_fractional_level() {
        assert_eq!(
            set_volume_args(75),
            vec!["set-volume", "@DEFAULT_AUDIO_SINK@", "0.75"]
        );
        assert_eq!(set_volume_args(0)[2], "0.00");
        assert_eq!(set_volume_args(100)[2], "1.00");
        assert_eq!(set_volume_args(250)[2], "1.00");
    }

    #[test]
    fn set_mute_args_toggles_default_sink() {
        assert_eq!(
            set_mute_args(),
            vec!["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]
        );
    }

    #[test]
    fn set_default_args_passes_numeric_id() {
        assert_eq!(set_default_args(55), vec!["set-default", "55"]);
    }
}
