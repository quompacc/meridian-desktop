use super::{
    activate_connection_args, connect_wifi_invocation, parse_saved_connections, parse_state,
    parse_wifi_networks, parse_wifi_signal_from_scan, redact_nmcli_args, NetworkController,
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
fn connect_wifi_invocation_uses_stdin_for_passwords() {
    assert_eq!(
        connect_wifi_invocation("Cafe", None),
        super::NmcliInvocation {
            args: vec!["device", "wifi", "connect", "Cafe"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            stdin: None,
        }
    );
    assert_eq!(
        connect_wifi_invocation("Home", Some("s3cret")),
        super::NmcliInvocation {
            args: vec!["--ask", "device", "wifi", "connect", "Home"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            stdin: Some("s3cret\n".to_string()),
        }
    );
}

#[test]
fn redact_nmcli_args_hides_password_values() {
    let args = vec![
        "device".to_string(),
        "wifi".to_string(),
        "connect".to_string(),
        "Home".to_string(),
        "password".to_string(),
        "s3cret".to_string(),
    ];
    assert_eq!(
        redact_nmcli_args(&args),
        vec![
            "device",
            "wifi",
            "connect",
            "Home",
            "password",
            "<redacted>"
        ]
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
