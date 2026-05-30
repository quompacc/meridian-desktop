use std::process::Command;

use super::{ConnectionKind, NetworkState};

const GENERAL_ARGS: [&str; 5] = ["-t", "-f", "STATE,CONNECTIVITY", "general", "status"];
const DEVICE_ARGS: [&str; 5] = [
    "-t",
    "-f",
    "TYPE,STATE,CONNECTION,DEVICE",
    "device",
    "status",
];
const WIFI_ARGS: [&str; 6] = ["-t", "-f", "IN-USE,SIGNAL,SSID", "dev", "wifi", "list"];
const CONNECTION_LIST_ARGS: [&str; 5] = ["-t", "-f", "NAME,TYPE,DEVICE", "connection", "show"];
const WIFI_SCAN_ARGS: [&str; 6] = [
    "-t",
    "-f",
    "IN-USE,SIGNAL,SECURITY,SSID",
    "dev",
    "wifi",
    "list",
];

/// A saved NetworkManager connection profile, as shown on the Settings
/// network page. `active` mirrors whether the profile currently has a device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionProfile {
    pub name: String,
    /// Human-facing type label ("Ethernet", "WLAN", "VPN", …).
    pub type_label: String,
    pub active: bool,
}

/// A Wi-Fi network from a scan, as shown on the Settings network page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiNetwork {
    pub ssid: String,
    /// Signal strength 0..=100.
    pub signal: u8,
    /// True when the network advertises any security (WPA/WEP/802.1X).
    pub secured: bool,
    /// True when this is the currently-connected network (nmcli IN-USE "*").
    pub in_use: bool,
}

pub struct NetworkController {
    last_state: NetworkState,
}

impl NetworkController {
    pub fn new() -> Self {
        Self {
            last_state: NetworkState::Offline,
        }
    }

    pub fn poll(&mut self) -> &NetworkState {
        // TODO: consider switching to `nmcli monitor` event stream when we need lower latency.
        let general_output = run_nmcli(&GENERAL_ARGS);
        let device_output = run_nmcli(&DEVICE_ARGS);

        let mut next = match (general_output, device_output) {
            (Some(general), Some(device)) => parse_state(&general, &device),
            _ => NetworkState::Offline,
        };

        if matches!(
            next,
            NetworkState::Connected {
                kind: ConnectionKind::Wifi { .. },
                ..
            }
        ) {
            let signal = run_nmcli(&WIFI_ARGS)
                .as_deref()
                .and_then(parse_wifi_signal_from_scan);
            if let NetworkState::Connected {
                kind: ConnectionKind::Wifi { signal: current },
                ..
            } = &mut next
            {
                *current = signal;
            }
        }

        self.last_state = next;
        &self.last_state
    }

    pub fn state(&self) -> &NetworkState {
        &self.last_state
    }
}

pub(crate) fn parse_state(general: &str, device: &str) -> NetworkState {
    let Some(general_line) = general.lines().map(str::trim).find(|line| !line.is_empty()) else {
        return NetworkState::Offline;
    };
    let general_fields = parse_terse_fields(general_line);
    if general_fields.len() < 2 {
        return NetworkState::Offline;
    }

    // nmcli emits multi-word states like "connected (local only)" or
    // "connected (site only)" when there's a working LAN connection but
    // limited or no internet. The device loop below already does the
    // same lenient check on per-device STATE; accept any "connected*"
    // here too so a local-only LAN still shows in the tray instead of
    // being misreported as Disconnected.
    let state = general_fields[0].to_ascii_lowercase();
    if !state.starts_with("connected") {
        return NetworkState::Disconnected;
    }

    let mut wifi_connection: Option<String> = None;
    let mut vpn_connection: Option<String> = None;
    let mut other_connection: Option<String> = None;

    for raw_line in device.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let fields = parse_terse_fields(line);
        if fields.len() < 4 {
            continue;
        }

        let kind = fields[0].to_ascii_lowercase();
        let dev_state = fields[1].to_ascii_lowercase();
        if !dev_state.starts_with("connected") || kind == "loopback" {
            continue;
        }

        let connection_name = fields[2].clone();
        match kind.as_str() {
            "ethernet" => {
                return NetworkState::Connected {
                    kind: ConnectionKind::Ethernet,
                    connection_name,
                };
            }
            "wifi" | "wireless" => {
                if wifi_connection.is_none() {
                    wifi_connection = Some(connection_name);
                }
            }
            "vpn" => {
                if vpn_connection.is_none() {
                    vpn_connection = Some(connection_name);
                }
            }
            _ => {
                if other_connection.is_none() {
                    other_connection = Some(connection_name);
                }
            }
        }
    }

    if let Some(connection_name) = wifi_connection {
        return NetworkState::Connected {
            kind: ConnectionKind::Wifi { signal: None },
            connection_name,
        };
    }
    if let Some(connection_name) = vpn_connection {
        return NetworkState::Connected {
            kind: ConnectionKind::Vpn,
            connection_name,
        };
    }
    if let Some(connection_name) = other_connection {
        return NetworkState::Connected {
            kind: ConnectionKind::Other,
            connection_name,
        };
    }

    NetworkState::Disconnected
}

/// List saved connection profiles (skipping loopback). Best-effort: returns an
/// empty list if nmcli is unavailable. Read-only, safe on the event loop.
pub fn list_saved_connections() -> Vec<ConnectionProfile> {
    run_nmcli(&CONNECTION_LIST_ARGS)
        .as_deref()
        .map(parse_saved_connections)
        .unwrap_or_default()
}

/// Parse `nmcli -t -f NAME,TYPE,DEVICE connection show`. A profile counts as
/// active when its DEVICE field is non-empty (nmcli leaves it blank for
/// inactive profiles). Loopback is filtered out.
pub(crate) fn parse_saved_connections(output: &str) -> Vec<ConnectionProfile> {
    let mut profiles = Vec::new();
    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let fields = parse_terse_fields(line);
        if fields.len() < 3 {
            continue;
        }
        let name = fields[0].clone();
        let kind = fields[1].to_ascii_lowercase();
        let device = fields[2].trim();
        if kind == "loopback" || name.is_empty() {
            continue;
        }
        let type_label = match kind.as_str() {
            k if k.contains("ethernet") => "Ethernet",
            k if k.contains("wireless") || k == "wifi" => "WLAN",
            k if k.contains("vpn") || k.contains("wireguard") => "VPN",
            k if k.contains("bridge") => "Bridge",
            k if k.contains("tun") => "Tunnel",
            _ => "Andere",
        };
        profiles.push(ConnectionProfile {
            name,
            type_label: type_label.to_string(),
            active: !device.is_empty(),
        });
    }
    profiles
}

/// Build the argv to activate a saved profile by name:
/// `nmcli connection up id <name>`. Pure so it can be unit-tested without
/// spawning; `id` disambiguates a name from a UUID/path.
pub(crate) fn activate_connection_args(name: &str) -> Vec<String> {
    vec![
        "connection".to_string(),
        "up".to_string(),
        "id".to_string(),
        name.to_string(),
    ]
}

/// Activate a saved profile by name. Runs nmcli on a background thread because
/// bringing a link up can block for seconds (DHCP, auth) and must never stall
/// the single-threaded shell event loop. Best-effort: logs on failure.
pub fn activate_connection(name: &str) {
    run_nmcli_background(activate_connection_args(name));
}

/// Scan for Wi-Fi networks (read-only). Best-effort: empty list if nmcli is
/// unavailable or no radio. nmcli's `dev wifi list` returns the cached scan
/// quickly, so this is safe on the event loop; an explicit rescan (slow) is a
/// separate background action.
pub fn scan_wifi_networks() -> Vec<WifiNetwork> {
    run_nmcli(&WIFI_SCAN_ARGS)
        .as_deref()
        .map(parse_wifi_networks)
        .unwrap_or_default()
}

/// Parse `nmcli -t -f IN-USE,SIGNAL,SECURITY,SSID dev wifi list`. Skips rows
/// with an empty SSID (hidden networks). De-duplicates by SSID keeping the
/// strongest signal, and sorts strongest-first.
pub(crate) fn parse_wifi_networks(output: &str) -> Vec<WifiNetwork> {
    let mut best: std::collections::BTreeMap<String, WifiNetwork> =
        std::collections::BTreeMap::new();
    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let fields = parse_terse_fields(line);
        if fields.len() < 4 {
            continue;
        }
        let in_use = fields[0].trim() == "*";
        let signal = fields[1].trim().parse::<u8>().unwrap_or(0).min(100);
        // SECURITY is blank for open networks; "--" can also appear.
        let sec = fields[2].trim();
        let secured = !sec.is_empty() && sec != "--";
        // SSID is the last field; it may itself contain escaped colons which
        // parse_terse_fields already unescaped, but a raw SSID with a colon
        // would have been split — rejoin any trailing fields to be safe.
        let ssid = fields[3..].join(":");
        let ssid = ssid.trim().to_string();
        if ssid.is_empty() {
            continue;
        }
        let candidate = WifiNetwork {
            ssid: ssid.clone(),
            signal,
            secured,
            in_use,
        };
        // Prefer the in-use BSS for an SSID, then the strongest signal, so a
        // connected network keeps its in_use flag even if another BSS of the
        // same name is stronger.
        best.entry(ssid)
            .and_modify(|existing| {
                let better =
                    (candidate.in_use, candidate.signal) > (existing.in_use, existing.signal);
                if better {
                    *existing = candidate.clone();
                }
            })
            .or_insert(candidate);
    }
    let mut networks: Vec<WifiNetwork> = best.into_values().collect();
    networks.sort_by(|a, b| b.signal.cmp(&a.signal).then(a.ssid.cmp(&b.ssid)));
    networks
}

/// Build the argv to connect to a Wi-Fi network. With `password`, appends
/// `password <pw>`; without, relies on stored credentials or an open network.
/// Pure for unit testing.
pub(crate) fn connect_wifi_args(ssid: &str, password: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "device".to_string(),
        "wifi".to_string(),
        "connect".to_string(),
        ssid.to_string(),
    ];
    if let Some(pw) = password {
        args.push("password".to_string());
        args.push(pw.to_string());
    }
    args
}

/// Connect to a Wi-Fi network on a background thread (the connect blocks for
/// seconds on association/DHCP/auth and must never stall the event loop).
/// Best-effort: logs on failure. NOTE: the password is passed as a process
/// argument, briefly visible in the process table — acceptable for a local
/// single-user desktop, matching how `nmcli` is normally invoked.
pub fn connect_wifi(ssid: &str, password: Option<&str>) {
    run_nmcli_background(connect_wifi_args(ssid, password));
}

/// Run nmcli with owned args on a detached thread, logging non-zero/spawn
/// failures. Shared by the activate/connect actions.
fn run_nmcli_background(args: Vec<String>) {
    std::thread::spawn(move || {
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        match Command::new("nmcli")
            .env("LC_ALL", "C")
            .args(&arg_refs)
            .status()
        {
            Ok(status) if status.success() => {}
            Ok(status) => tracing::warn!("nmcli {:?} exited with {}", args, status),
            Err(err) => tracing::warn!("failed to run nmcli {:?}: {}", args, err),
        }
    });
}

fn run_nmcli(args: &[&str]) -> Option<String> {
    let output = Command::new("nmcli")
        .env("LC_ALL", "C")
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn parse_terse_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut escaped = false;

    for ch in line.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            ':' => {
                fields.push(current);
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    fields.push(current);
    fields
}

fn parse_wifi_signal_from_scan(output: &str) -> Option<u8> {
    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let fields = parse_terse_fields(line);
        if fields.len() < 3 {
            continue;
        }
        if fields[0].trim() != "*" {
            continue;
        }
        let parsed = fields[1].trim().parse::<u8>().ok()?;
        return Some(parsed.min(100));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        activate_connection_args, connect_wifi_args, parse_saved_connections, parse_state,
        parse_wifi_networks, parse_wifi_signal_from_scan, NetworkController,
    };
    use crate::network::{ConnectionKind, NetworkState};

    #[test]
    fn parse_state_returns_offline_for_unparsable_general() {
        let state = parse_state("garbage", "ethernet:connected:Wired connection 1:enp1s0");
        assert_eq!(state, NetworkState::Offline);
    }

    #[test]
    fn parse_state_returns_disconnected_when_general_is_not_connected() {
        let state = parse_state(
            "disconnected:none",
            "ethernet:connected:Wired connection 1:enp1s0",
        );
        assert_eq!(state, NetworkState::Disconnected);
    }

    #[test]
    fn parse_state_returns_ethernet_when_wired_connected() {
        let state = parse_state(
            "connected:full",
            "ethernet:connected:Wired connection 1:enp1s0\nloopback:connected (externally):lo:lo",
        );
        assert_eq!(
            state,
            NetworkState::Connected {
                kind: ConnectionKind::Ethernet,
                connection_name: "Wired connection 1".to_string(),
            }
        );
    }

    #[test]
    fn parse_state_returns_wifi_when_wireless_connected() {
        let state = parse_state("connected:full", "wifi:connected:HomeAP:wlan0");
        assert_eq!(
            state,
            NetworkState::Connected {
                kind: ConnectionKind::Wifi { signal: None },
                connection_name: "HomeAP".to_string(),
            }
        );
    }

    #[test]
    fn parse_state_ignores_loopback() {
        let state = parse_state(
            "connected:full",
            "loopback:connected (externally):lo:lo\nloopback:connected:lo:lo",
        );
        assert_eq!(state, NetworkState::Disconnected);
    }

    #[test]
    fn parse_state_strips_quoted_colons_in_connection_names() {
        let state = parse_state("connected:full", r"ethernet:connected:Foo\:Bar:enp1s0");
        assert_eq!(
            state,
            NetworkState::Connected {
                kind: ConnectionKind::Ethernet,
                connection_name: "Foo:Bar".to_string(),
            }
        );
    }

    #[test]
    fn parse_wifi_signal_finds_active_network() {
        let signal = parse_wifi_signal_from_scan("*:75:MyHomeWifi\n:60:NeighborWifi");
        assert_eq!(signal, Some(75));
    }

    #[test]
    fn parse_saved_connections_marks_active_and_skips_loopback() {
        let out =
            "Wired:802-3-ethernet:enp1s0\nHomeWifi:802-11-wireless:\nlo:loopback:lo\nWork VPN:vpn:";
        let profiles = parse_saved_connections(out);
        assert_eq!(profiles.len(), 3); // loopback dropped
        assert_eq!(profiles[0].name, "Wired");
        assert_eq!(profiles[0].type_label, "Ethernet");
        assert!(profiles[0].active); // has a device
        assert_eq!(profiles[1].name, "HomeWifi");
        assert_eq!(profiles[1].type_label, "WLAN");
        assert!(!profiles[1].active); // blank device
        assert_eq!(profiles[2].type_label, "VPN");
        assert!(!profiles[2].active);
    }

    #[test]
    fn parse_saved_connections_handles_quoted_colons_in_name() {
        let profiles = parse_saved_connections(r"Foo\:Bar:802-3-ethernet:enp1s0");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "Foo:Bar");
    }

    #[test]
    fn activate_connection_args_uses_id_form() {
        assert_eq!(
            activate_connection_args("Home Wifi"),
            vec!["connection", "up", "id", "Home Wifi"]
        );
    }

    #[test]
    fn parse_wifi_networks_dedups_sorts_and_flags_security() {
        let out = "\
*:62:WPA2:HomeNet\n\
:80:WPA2:HomeNet\n\
:45::OpenCafe\n\
:30:WPA1 WPA2:Office\n\
:55:--:AlsoOpen\n\
:90:WPA2:\n";
        let nets = parse_wifi_networks(out);
        // Hidden (empty SSID) dropped; HomeNet deduped, in-use BSS kept.
        // Sorted strongest-first: HomeNet 62, AlsoOpen 55, OpenCafe 45, Office 30.
        let names: Vec<&str> = nets.iter().map(|n| n.ssid.as_str()).collect();
        assert_eq!(names, vec!["HomeNet", "AlsoOpen", "OpenCafe", "Office"]);
        let home = nets.iter().find(|n| n.ssid == "HomeNet").unwrap();
        assert!(home.in_use); // in-use 62 kept over stronger non-in-use 80
        assert_eq!(home.signal, 62);
        assert!(home.secured);
        let open = nets.iter().find(|n| n.ssid == "OpenCafe").unwrap();
        assert!(!open.secured);
        let also = nets.iter().find(|n| n.ssid == "AlsoOpen").unwrap();
        assert!(!also.secured); // "--" treated as open
    }

    #[test]
    fn connect_wifi_args_with_and_without_password() {
        assert_eq!(
            connect_wifi_args("Cafe", None),
            vec!["device", "wifi", "connect", "Cafe"]
        );
        assert_eq!(
            connect_wifi_args("Home", Some("s3cret")),
            vec!["device", "wifi", "connect", "Home", "password", "s3cret"]
        );
    }

    #[test]
    fn integration_real_nmcli_can_be_polled() {
        if std::process::Command::new("nmcli")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("skipping; nmcli not in PATH");
            return;
        }

        let mut controller = NetworkController::new();
        let state = controller.poll().clone();
        assert!(matches!(
            state,
            NetworkState::Connected { .. } | NetworkState::Disconnected | NetworkState::Offline
        ));
    }
}
