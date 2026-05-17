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

    let state = general_fields[0].to_ascii_lowercase();
    if state != "connected" {
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
    use super::{parse_state, parse_wifi_signal_from_scan, NetworkController};
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
