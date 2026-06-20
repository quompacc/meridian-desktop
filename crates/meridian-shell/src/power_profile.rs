//! Power-profile quick switching via `power-profiles-daemon` (`powerprofilesctl`).
//! Maps the user-facing Eco / Standard / Full names to the daemon's
//! power-saver / balanced / performance profiles.

use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerProfile {
    Eco,
    Standard,
    Performance,
}

impl PowerProfile {
    pub const ALL: [PowerProfile; 3] = [
        PowerProfile::Eco,
        PowerProfile::Standard,
        PowerProfile::Performance,
    ];

    /// The power-profiles-daemon identifier.
    pub fn daemon_id(self) -> &'static str {
        match self {
            PowerProfile::Eco => "power-saver",
            PowerProfile::Standard => "balanced",
            PowerProfile::Performance => "performance",
        }
    }

    /// User-facing label for the popup.
    pub fn label(self) -> &'static str {
        match self {
            PowerProfile::Eco => "Eco",
            PowerProfile::Standard => "Standard",
            PowerProfile::Performance => "Volle Leistung",
        }
    }

    pub fn from_daemon(value: &str) -> Option<Self> {
        match value.trim() {
            "power-saver" => Some(PowerProfile::Eco),
            "balanced" => Some(PowerProfile::Standard),
            "performance" => Some(PowerProfile::Performance),
            _ => None,
        }
    }
}

/// Current active profile, or `None` if the daemon is unavailable.
pub fn current() -> Option<PowerProfile> {
    let output = Command::new("powerprofilesctl").arg("get").output().ok()?;
    if !output.status.success() {
        return None;
    }
    PowerProfile::from_daemon(&String::from_utf8_lossy(&output.stdout))
}

/// Switch profile. Returns true on success.
pub fn set(profile: PowerProfile) -> bool {
    Command::new("powerprofilesctl")
        .arg("set")
        .arg(profile.daemon_id())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_user_names_to_daemon_ids() {
        assert_eq!(PowerProfile::Eco.daemon_id(), "power-saver");
        assert_eq!(PowerProfile::Standard.daemon_id(), "balanced");
        assert_eq!(PowerProfile::Performance.daemon_id(), "performance");
    }

    #[test]
    fn round_trips_through_daemon_id() {
        for p in PowerProfile::ALL {
            assert_eq!(PowerProfile::from_daemon(p.daemon_id()), Some(p));
        }
        assert_eq!(PowerProfile::from_daemon("nonsense"), None);
    }
}
