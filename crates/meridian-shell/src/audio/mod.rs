//! Audio status and control for the panel and settings.
//!
//! The backend is platform-specific. Linux drives PipeWire through `wpctl`.
//! FreeBSD uses the base `mixer(8)`/OSS instead: the FreeBSD PipeWire build
//! ships no ALSA/OSS SPA plugin, so PipeWire cannot reach the sound hardware
//! there, whereas mixer(8) talks to `/dev/mixerN` directly.

#[cfg(not(target_os = "linux"))]
mod mixer;
#[cfg(target_os = "linux")]
mod wpctl;

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
        backend_poll()
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

/// Set the default sink volume to `percent` (0..=100). Best-effort: a failing
/// backend never breaks the caller.
pub(crate) fn set_default_sink_volume(percent: u8) {
    backend_set_volume(percent);
}

/// Toggle mute on the default sink. Best-effort.
pub(crate) fn toggle_default_sink_mute() {
    backend_toggle_mute();
}

/// Make the device with the given id the default of its kind. Best-effort.
pub(crate) fn set_default_device(id: u32) {
    backend_set_default(id);
}

#[cfg(target_os = "linux")]
fn backend_poll() -> AudioSnapshot {
    wpctl::snapshot()
}
#[cfg(target_os = "linux")]
fn backend_set_volume(percent: u8) {
    wpctl::set_volume(percent);
}
#[cfg(target_os = "linux")]
fn backend_toggle_mute() {
    wpctl::toggle_mute();
}
#[cfg(target_os = "linux")]
fn backend_set_default(id: u32) {
    wpctl::set_default(id);
}

#[cfg(not(target_os = "linux"))]
fn backend_poll() -> AudioSnapshot {
    mixer::snapshot()
}
#[cfg(not(target_os = "linux"))]
fn backend_set_volume(percent: u8) {
    mixer::set_volume(percent);
}
#[cfg(not(target_os = "linux"))]
fn backend_toggle_mute() {
    mixer::toggle_mute();
}
#[cfg(not(target_os = "linux"))]
fn backend_set_default(id: u32) {
    mixer::set_default(id);
}

#[cfg(test)]
mod tests {
    use super::{AudioServiceState, AudioSnapshot};

    #[test]
    fn unavailable_snapshot_uses_muted_panel_fallback() {
        let snapshot = AudioSnapshot::unavailable();
        assert_eq!(snapshot.service, AudioServiceState::Unavailable);
        assert_eq!(snapshot.panel_label(), "AUD");
        assert_eq!(snapshot.icon_name(), "audio-volume-muted-symbolic");
    }
}
