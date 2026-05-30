use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AudioSnapshot {
    pub(crate) service: AudioServiceState,
    pub(crate) default_output: Option<AudioDevice>,
    pub(crate) default_input: Option<AudioDevice>,
    pub(crate) outputs: Vec<AudioDevice>,
    pub(crate) inputs: Vec<AudioDevice>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AudioServiceState {
    Running,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AudioDevice {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) volume_percent: Option<u8>,
    pub(crate) muted: bool,
    pub(crate) is_default: bool,
}

impl AudioSnapshot {
    pub(crate) fn poll() -> Self {
        let Some(output) = run_wpctl_status() else {
            return Self::unavailable();
        };
        parse_wpctl_status(&output)
    }

    pub(crate) fn unavailable() -> Self {
        Self {
            service: AudioServiceState::Unavailable,
            default_output: None,
            default_input: None,
            outputs: Vec::new(),
            inputs: Vec::new(),
        }
    }

    pub(crate) fn panel_label(&self) -> String {
        let Some(output) = self.default_output.as_ref() else {
            return "AUD".to_string();
        };
        if output.muted {
            return "MUT".to_string();
        }
        output
            .volume_percent
            .map(|volume| format!("{}%", volume))
            .unwrap_or_else(|| "AUD".to_string())
    }

    pub(crate) fn icon_name(&self) -> &'static str {
        let Some(output) = self.default_output.as_ref() else {
            return "audio-volume-muted-symbolic";
        };
        if output.muted {
            return "audio-volume-muted-symbolic";
        }
        match output.volume_percent {
            Some(0) => "audio-volume-muted-symbolic",
            Some(value) if value < 35 => "audio-volume-low-symbolic",
            Some(value) if value < 70 => "audio-volume-medium-symbolic",
            Some(_) => "audio-volume-high-symbolic",
            None => "audio-volume-medium-symbolic",
        }
    }
}

pub(crate) fn parse_wpctl_status(output: &str) -> AudioSnapshot {
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
    let trimmed = line.trim_start_matches(['|', '`', '-', ' ', '\t']).trim();
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

/// Build the `wpctl set-volume` argv for the default sink at `percent`
/// (clamped to 0..=100). Pure so the argument shape can be unit-tested without
/// spawning a process. wpctl takes a fractional level, e.g. `0.75`.
fn set_volume_args(percent: u8) -> Vec<String> {
    let level = (percent.min(100) as f32) / 100.0;
    vec![
        "set-volume".to_string(),
        DEFAULT_SINK.to_string(),
        format!("{:.2}", level),
    ]
}

/// Build the `wpctl set-mute` argv for the default sink. `toggle` flips the
/// current state; this is the only mute operation the UI needs.
fn set_mute_args() -> Vec<String> {
    vec![
        "set-mute".to_string(),
        DEFAULT_SINK.to_string(),
        "toggle".to_string(),
    ]
}

/// Build the `wpctl set-default <id>` argv. wpctl makes the object the default
/// of its kind (sink or source) from the numeric id reported by `status`.
fn set_default_args(id: u32) -> Vec<String> {
    vec!["set-default".to_string(), id.to_string()]
}

/// Set the default sink volume to `percent` (0..=100). Best-effort: logs and
/// returns on failure so a missing/unhappy wpctl never breaks the caller.
pub(crate) fn set_default_sink_volume(percent: u8) {
    run_wpctl(&set_volume_args(percent));
}

/// Toggle mute on the default sink. Best-effort, same contract as above.
pub(crate) fn toggle_default_sink_mute() {
    run_wpctl(&set_mute_args());
}

/// Make the device with the given wpctl id the default of its kind (sink or
/// source). Best-effort, same contract as above.
pub(crate) fn set_default_device(id: u32) {
    run_wpctl(&set_default_args(id));
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
    use super::{
        parse_wpctl_status, set_default_args, set_mute_args, set_volume_args, AudioSnapshot,
    };

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
    fn unavailable_snapshot_uses_muted_panel_fallback() {
        let snapshot = AudioSnapshot::unavailable();
        assert_eq!(snapshot.panel_label(), "AUD");
        assert_eq!(snapshot.icon_name(), "audio-volume-muted-symbolic");
    }

    #[test]
    fn set_volume_args_targets_default_sink_with_fractional_level() {
        assert_eq!(
            set_volume_args(75),
            vec!["set-volume", "@DEFAULT_AUDIO_SINK@", "0.75"]
        );
        // 0% and 100% map to the fractional bounds.
        assert_eq!(set_volume_args(0)[2], "0.00");
        assert_eq!(set_volume_args(100)[2], "1.00");
        // Out-of-range input is clamped to 100% (never amplifies past unity).
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
