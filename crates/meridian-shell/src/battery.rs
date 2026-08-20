//! Battery + AC state for the panel tray, read from `/sys/class/power_supply`.
//! Pure sysfs reads (no daemon dependency), polled on the panel tick like the
//! audio/network snapshots.

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeState {
    Charging,
    Discharging,
    Full,
    NotCharging,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatterySnapshot {
    /// True when at least one battery device is present (desktops have none).
    pub present: bool,
    /// Charge percentage, 0..=100.
    pub capacity: u8,
    pub state: ChargeState,
    /// True when an AC adapter reports online.
    pub on_ac: bool,
}

impl Default for BatterySnapshot {
    fn default() -> Self {
        Self {
            present: false,
            capacity: 0,
            state: ChargeState::Unknown,
            on_ac: false,
        }
    }
}

const POWER_SUPPLY: &str = "/sys/class/power_supply";

impl BatterySnapshot {
    pub fn poll() -> Self {
        let mut snap = BatterySnapshot::default();
        let Ok(entries) = fs::read_dir(POWER_SUPPLY) else {
            return snap;
        };
        let mut batteries: Vec<(u8, ChargeState)> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let kind = read_trim(&path.join("type"));
            let is_battery = kind.as_deref() == Some("Battery") || name.starts_with("BAT");
            let is_mains = kind.as_deref() == Some("Mains")
                || name.starts_with("AC")
                || name.starts_with("ADP");

            if is_battery {
                let capacity = read_trim(&path.join("capacity")).and_then(|s| s.parse::<u8>().ok());
                let state = match read_trim(&path.join("status")).as_deref() {
                    Some("Charging") => ChargeState::Charging,
                    Some("Discharging") => ChargeState::Discharging,
                    Some("Full") => ChargeState::Full,
                    Some("Not charging") => ChargeState::NotCharging,
                    _ => ChargeState::Unknown,
                };
                if let Some(capacity) = capacity {
                    batteries.push((capacity.min(100), state));
                }
            } else if is_mains && read_trim(&path.join("online")).as_deref() == Some("1") {
                snap.on_ac = true;
            }
        }
        if let Some((capacity, state)) = aggregate_batteries(&batteries) {
            snap.present = true;
            snap.capacity = capacity;
            snap.state = state;
        }
        snap
    }

    /// Freedesktop symbolic icon name (recoloured to the theme text colour by the
    /// icon pipeline, so it always contrasts). Charging variants when on power.
    pub fn icon_name(&self) -> &'static str {
        let charging =
            matches!(self.state, ChargeState::Charging) || (self.on_ac && self.capacity >= 100);
        if charging {
            return match self.capacity {
                c if c >= 90 => "battery-full-charging-symbolic",
                c if c >= 60 => "battery-good-charging-symbolic",
                c if c >= 30 => "battery-low-charging-symbolic",
                _ => "battery-caution-charging-symbolic",
            };
        }
        match self.capacity {
            c if c >= 90 => "battery-full-symbolic",
            c if c >= 60 => "battery-good-symbolic",
            c if c >= 30 => "battery-low-symbolic",
            c if c >= 10 => "battery-caution-symbolic",
            _ => "battery-empty-symbolic",
        }
    }

    /// Short tray label, e.g. "72%".
    pub fn label(&self) -> String {
        format!("{}%", self.capacity)
    }
}

/// Collapse several battery devices into one tray reading (P3-2,
/// AUDIT_2026-08-19 — previously each iteration overwrote the previous
/// battery, "last one wins"). Capacity is the rounded mean of all readable
/// batteries; state prefers Charging > Discharging > Full > NotCharging so a
/// single charging cell is visible even while another discharges.
fn aggregate_batteries(batteries: &[(u8, ChargeState)]) -> Option<(u8, ChargeState)> {
    let count = batteries.len();
    if count == 0 {
        return None;
    }
    let sum: u32 = batteries.iter().map(|(capacity, _)| *capacity as u32).sum();
    let capacity = ((sum + count as u32 / 2) / count as u32).min(100) as u8;
    let state = if batteries
        .iter()
        .any(|(_, state)| matches!(state, ChargeState::Charging))
    {
        ChargeState::Charging
    } else if batteries
        .iter()
        .any(|(_, state)| matches!(state, ChargeState::Discharging))
    {
        ChargeState::Discharging
    } else if batteries
        .iter()
        .any(|(_, state)| matches!(state, ChargeState::Full))
    {
        ChargeState::Full
    } else if batteries
        .iter()
        .any(|(_, state)| matches!(state, ChargeState::NotCharging))
    {
        ChargeState::NotCharging
    } else {
        ChargeState::Unknown
    };
    Some((capacity, state))
}

fn read_trim(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// All symbolic battery icon names used above — warmed at startup so the tray
/// has them ready without a per-frame disk hit.
pub const ICON_NAMES: &[&str] = &[
    "battery-full-symbolic",
    "battery-good-symbolic",
    "battery-low-symbolic",
    "battery-caution-symbolic",
    "battery-empty-symbolic",
    "battery-full-charging-symbolic",
    "battery-good-charging-symbolic",
    "battery-low-charging-symbolic",
    "battery-caution-charging-symbolic",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_buckets_pick_level_and_charging() {
        let full = BatterySnapshot {
            present: true,
            capacity: 95,
            state: ChargeState::Discharging,
            on_ac: false,
        };
        assert_eq!(full.icon_name(), "battery-full-symbolic");

        let low = BatterySnapshot {
            present: true,
            capacity: 15,
            state: ChargeState::Discharging,
            on_ac: false,
        };
        assert_eq!(low.icon_name(), "battery-caution-symbolic");

        let charging = BatterySnapshot {
            present: true,
            capacity: 50,
            state: ChargeState::Charging,
            on_ac: true,
        };
        assert_eq!(charging.icon_name(), "battery-low-charging-symbolic");
    }

    #[test]
    fn label_is_percent() {
        let s = BatterySnapshot {
            present: true,
            capacity: 72,
            state: ChargeState::Discharging,
            on_ac: false,
        };
        assert_eq!(s.label(), "72%");
    }

    #[test]
    fn poll_does_not_panic() {
        // Host-dependent (desktops have no battery); must not panic.
        let _ = BatterySnapshot::poll();
    }

    #[test]
    fn aggregates_multiple_batteries_to_mean_and_charging_priority() {
        let readings = [
            (80u8, ChargeState::Discharging),
            (41u8, ChargeState::Charging),
        ];
        let (capacity, state) = aggregate_batteries(&readings).expect("reading");
        assert_eq!(capacity, 61); // rounded mean of 80 and 41
        assert_eq!(state, ChargeState::Charging);
    }

    #[test]
    fn aggregate_single_battery_passes_through() {
        let readings = [(72u8, ChargeState::Discharging)];
        assert_eq!(
            aggregate_batteries(&readings),
            Some((72, ChargeState::Discharging))
        );
    }

    #[test]
    fn aggregate_prefers_discharging_over_full() {
        let readings = [(100u8, ChargeState::Full), (30u8, ChargeState::Discharging)];
        let (capacity, state) = aggregate_batteries(&readings).expect("reading");
        assert_eq!(capacity, 65);
        assert_eq!(state, ChargeState::Discharging);
    }

    #[test]
    fn aggregate_empty_is_none() {
        assert_eq!(aggregate_batteries(&[]), None);
    }
}
