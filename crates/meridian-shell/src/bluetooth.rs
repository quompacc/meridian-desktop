//! Bluetooth status + control for the Settings "Bluetooth" page.
//!
//! Reads adapter state and the device list from `bluetoothctl` (BlueZ). The
//! read-only snapshot (adapter present, powered, scanning, devices) is cheap
//! and safe on the event loop. Mutations — power on/off, scan on/off, and
//! pair/connect/remove by address — block for seconds, so they run on a
//! background thread (mirrors the nmcli pattern). Parsers and argv builders are
//! pure and unit-tested; the runtime paths are verified against a virtual
//! `btvirt` adapter on the dev VM (see the bluetooth-test-setup memory).

use std::process::Command;

/// A Bluetooth device as listed by `bluetoothctl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BluetoothDevice {
    pub address: String,
    pub name: String,
    pub paired: bool,
    pub connected: bool,
}

/// Snapshot of the default adapter and its known devices.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BluetoothSnapshot {
    /// False when no adapter / bluetoothctl is present.
    pub adapter_present: bool,
    pub powered: bool,
    pub scanning: bool,
    pub devices: Vec<BluetoothDevice>,
}

impl BluetoothSnapshot {
    /// Poll the current state. Best-effort and read-only; returns an
    /// adapter-absent snapshot if bluetoothctl is missing or errors.
    pub fn poll() -> Self {
        let Some(show) = run_bluetoothctl(&["show"]) else {
            return Self::default();
        };
        // `show` without a controller prints nothing useful; treat an output
        // without a "Controller" line as no adapter.
        if !show.contains("Controller") {
            return Self::default();
        }
        let powered = parse_show_flag(&show, "Powered:");
        let scanning = parse_show_flag(&show, "Discovering:");
        let devices = run_bluetoothctl(&["devices"])
            .as_deref()
            .map(|list| parse_devices(list, &paired_addresses(), &connected_addresses()))
            .unwrap_or_default();
        Self {
            adapter_present: true,
            powered,
            scanning,
            devices,
        }
    }
}

fn paired_addresses() -> Vec<String> {
    run_bluetoothctl(&["devices", "Paired"])
        .as_deref()
        .map(parse_device_addresses)
        .unwrap_or_default()
}

fn connected_addresses() -> Vec<String> {
    run_bluetoothctl(&["devices", "Connected"])
        .as_deref()
        .map(parse_device_addresses)
        .unwrap_or_default()
}

/// Parse a `Key: value` flag from `bluetoothctl show`, true iff value is "yes".
pub(crate) fn parse_show_flag(show: &str, key: &str) -> bool {
    show.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(key))
        .map(|v| v.trim().eq_ignore_ascii_case("yes"))
        .unwrap_or(false)
}

/// Parse `Device <ADDR> <NAME>` lines into devices, flagging paired/connected
/// from the supplied address sets.
pub(crate) fn parse_devices(
    list: &str,
    paired: &[String],
    connected: &[String],
) -> Vec<BluetoothDevice> {
    list.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("Device ")?;
            let (address, name) = rest.split_once(' ')?;
            let address = address.trim().to_string();
            if address.is_empty() {
                return None;
            }
            Some(BluetoothDevice {
                paired: paired.iter().any(|a| a == &address),
                connected: connected.iter().any(|a| a == &address),
                address,
                name: name.trim().to_string(),
            })
        })
        .collect()
}

/// Extract just the addresses from `Device <ADDR> <NAME>` lines.
pub(crate) fn parse_device_addresses(list: &str) -> Vec<String> {
    list.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("Device ")?;
            let addr = rest.split_whitespace().next()?;
            (!addr.is_empty()).then(|| addr.to_string())
        })
        .collect()
}

// ── Mutations (run off the event loop) ───────────────────────────────────────

/// `bluetoothctl power on|off` argv.
pub(crate) fn power_args(on: bool) -> Vec<String> {
    vec![
        "power".to_string(),
        if on { "on" } else { "off" }.to_string(),
    ]
}

/// `bluetoothctl --timeout <secs> scan on` argv. A plain one-shot `scan on`
/// does NOT persist discovery — bluetoothctl ties the scan to its D-Bus
/// connection, which ends when the process exits. `--timeout` keeps the
/// process (and thus discovery) alive for the given window, then auto-stops.
pub(crate) fn scan_args(secs: u32) -> Vec<String> {
    vec![
        "--timeout".to_string(),
        secs.to_string(),
        "scan".to_string(),
        "on".to_string(),
    ]
}

/// How long a single discovery run lasts (seconds).
pub const SCAN_DURATION_SECS: u32 = 10;

/// `bluetoothctl <verb> <addr>` argv for pair/connect/disconnect/remove/trust.
pub(crate) fn device_args(verb: &str, addr: &str) -> Vec<String> {
    vec![verb.to_string(), addr.to_string()]
}

/// Set adapter power. Off-thread: the controller change is not instant.
pub fn set_power(on: bool) {
    run_bluetoothctl_background(power_args(on));
}

/// Run a timed discovery (`SCAN_DURATION_SECS`). Off-thread: the background
/// thread blocks for the whole scan window, which must never be the event loop.
pub fn start_scan() {
    run_bluetoothctl_background(scan_args(SCAN_DURATION_SECS));
}

/// Pair (then trust + connect) a device by address. Pairing is a remote
/// handshake that can block for seconds, so this runs off-thread and chains the
/// steps; failures are logged no-ops.
pub fn pair_device(addr: &str) {
    let addr = addr.to_string();
    std::thread::spawn(move || {
        for verb in ["pair", "trust", "connect"] {
            run_bluetoothctl_blocking(&device_args(verb, &addr));
        }
    });
}

/// Connect an already-paired device by address. Off-thread.
pub fn connect_device(addr: &str) {
    run_bluetoothctl_background(device_args("connect", addr));
}

fn run_bluetoothctl(args: &[&str]) -> Option<String> {
    let output = Command::new("bluetoothctl")
        .env("LC_ALL", "C")
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn run_bluetoothctl_background(args: Vec<String>) {
    std::thread::spawn(move || {
        run_bluetoothctl_blocking(&args);
    });
}

fn run_bluetoothctl_blocking(args: &[String]) {
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    match Command::new("bluetoothctl")
        .env("LC_ALL", "C")
        .args(&arg_refs)
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => tracing::warn!("bluetoothctl {:?} exited with {}", args, status),
        Err(err) => tracing::warn!("failed to run bluetoothctl {:?}: {}", args, err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_show_flag_reads_yes_no() {
        let show = "Controller 00:AA:01:00:00:00 (public)\n\tPowered: yes\n\tDiscovering: no\n";
        assert!(parse_show_flag(show, "Powered:"));
        assert!(!parse_show_flag(show, "Discovering:"));
        assert!(!parse_show_flag(show, "Missing:"));
    }

    #[test]
    fn parse_devices_flags_paired_and_connected() {
        let list = "Device AA:BB:CC:DD:EE:FF My Headphones\nDevice 11:22:33:44:55:66 Keyboard\n";
        let paired = vec!["AA:BB:CC:DD:EE:FF".to_string()];
        let connected = vec!["AA:BB:CC:DD:EE:FF".to_string()];
        let devices = parse_devices(list, &paired, &connected);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].address, "AA:BB:CC:DD:EE:FF");
        assert_eq!(devices[0].name, "My Headphones");
        assert!(devices[0].paired && devices[0].connected);
        assert_eq!(devices[1].name, "Keyboard");
        assert!(!devices[1].paired && !devices[1].connected);
    }

    #[test]
    fn parse_device_addresses_extracts_addrs() {
        let list =
            "Device AA:BB:CC:DD:EE:FF My Headphones\ngarbage line\nDevice 11:22:33:44:55:66 Kbd\n";
        assert_eq!(
            parse_device_addresses(list),
            vec![
                "AA:BB:CC:DD:EE:FF".to_string(),
                "11:22:33:44:55:66".to_string()
            ]
        );
    }

    #[test]
    fn argv_builders() {
        assert_eq!(power_args(true), vec!["power", "on"]);
        assert_eq!(power_args(false), vec!["power", "off"]);
        assert_eq!(scan_args(10), vec!["--timeout", "10", "scan", "on"]);
        assert_eq!(device_args("pair", "AA:BB"), vec!["pair", "AA:BB"]);
    }
}
