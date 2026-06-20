//! FreeBSD network backend.
//!
//! NetworkManager/`nmcli` is Linux-only, so on FreeBSD the panel and settings
//! network state is derived from `ifconfig(8)`, and Wi-Fi scanning from
//! `ifconfig <wlan> list scan`. The wired path needs no management to be
//! displayed; managed Wi-Fi association is a best-effort `wpa_cli` call and is
//! only meaningful where a `wlan` device is configured.

use std::process::Command;

use super::{ConnectionKind, NetworkState};

/// A network connection profile shown on the Settings network page. Same shape
/// as the nmcli backend's so the settings UI stays backend-agnostic. On FreeBSD
/// a "profile" is a configured interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionProfile {
    pub name: String,
    pub type_label: String,
    pub active: bool,
}

/// A Wi-Fi network from a scan, as shown on the Settings network page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal: u8,
    pub secured: bool,
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
        self.last_state = run_ifconfig(&["-a"])
            .as_deref()
            .map(parse_ifconfig_state)
            .unwrap_or(NetworkState::Offline);
        &self.last_state
    }

    pub fn state(&self) -> &NetworkState {
        &self.last_state
    }
}

/// Per-interface summary distilled from one `ifconfig` block.
#[derive(Debug, Clone, PartialEq, Eq)]
struct IfaceSummary {
    name: String,
    is_loopback: bool,
    kind: ConnectionKind,
    has_inet: bool,
    active: bool,
    ssid: Option<String>,
}

impl IfaceSummary {
    fn is_wifi(&self) -> bool {
        matches!(self.kind, ConnectionKind::Wifi { .. })
    }
}

/// Classify an interface by its FreeBSD device name. Wireless devices are
/// `wlanN`; tunnels/VPNs use well-known prefixes; virtual aggregations map to
/// `Other`; everything else is treated as wired Ethernet.
fn classify_kind(name: &str) -> ConnectionKind {
    const VPN_PREFIXES: [&str; 7] = ["tun", "tap", "wg", "ppp", "gif", "ovpn", "ipsec"];
    const OTHER_PREFIXES: [&str; 5] = ["bridge", "vlan", "lagg", "epair", "vmnet"];
    if name.starts_with("wlan") {
        ConnectionKind::Wifi { signal: None }
    } else if VPN_PREFIXES.iter().any(|p| name.starts_with(p)) {
        ConnectionKind::Vpn
    } else if OTHER_PREFIXES.iter().any(|p| name.starts_with(p)) {
        ConnectionKind::Other
    } else {
        ConnectionKind::Ethernet
    }
}

/// Priority for picking the active connection when several are up, lowest first:
/// wired beats Wi-Fi beats VPN beats anything else (mirrors the nmcli backend).
fn kind_priority(kind: &ConnectionKind) -> u8 {
    match kind {
        ConnectionKind::Ethernet => 0,
        ConnectionKind::Wifi { .. } => 1,
        ConnectionKind::Vpn => 2,
        ConnectionKind::Other => 3,
    }
}

/// Derive the tray/settings state from `ifconfig -a`. An interface counts as
/// connected when it is non-loopback, link-active, and carries an IPv4 address.
pub(crate) fn parse_ifconfig_state(output: &str) -> NetworkState {
    let ifaces = parse_ifconfig_interfaces(output);
    let mut connected: Vec<&IfaceSummary> = ifaces
        .iter()
        .filter(|i| !i.is_loopback && i.active && i.has_inet)
        .collect();

    if connected.is_empty() {
        // Usable interfaces present but none up → Disconnected; nothing but
        // loopback → Offline.
        if ifaces.iter().any(|i| !i.is_loopback) {
            return NetworkState::Disconnected;
        }
        return NetworkState::Offline;
    }

    connected.sort_by_key(|i| kind_priority(&i.kind));
    let chosen = connected[0];
    let connection_name = if chosen.is_wifi() {
        chosen
            .ssid
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| chosen.name.clone())
    } else {
        chosen.name.clone()
    };
    NetworkState::Connected {
        kind: chosen.kind.clone(),
        connection_name,
    }
}

/// Split `ifconfig -a` output into one [`IfaceSummary`] per interface block.
/// A block starts at column 0 with `name: flags=...`; indented lines carry the
/// `inet`, `status:`, `media:`, and `ssid` details.
fn parse_ifconfig_interfaces(output: &str) -> Vec<IfaceSummary> {
    let mut out = Vec::new();
    let mut cur: Option<IfaceSummary> = None;

    for line in output.lines() {
        let starts_block = !line.starts_with([' ', '\t']) && line.contains(':');
        if starts_block {
            if let Some(done) = cur.take() {
                out.push(done);
            }
            if let Some((name, _)) = line.split_once(':') {
                let name = name.trim().to_string();
                let is_loopback = name.starts_with("lo");
                let kind = classify_kind(&name);
                cur = Some(IfaceSummary {
                    name,
                    is_loopback,
                    kind,
                    has_inet: false,
                    active: false,
                    ssid: None,
                });
            }
            continue;
        }

        let Some(iface) = cur.as_mut() else {
            continue;
        };
        let t = line.trim();
        if t.starts_with("inet ") {
            iface.has_inet = true;
        } else if let Some(rest) = t.strip_prefix("status:") {
            let st = rest.trim();
            iface.active = st.starts_with("active") || st.starts_with("associated");
        } else if t.starts_with("media:") {
            if t.contains("802.11") {
                iface.kind = ConnectionKind::Wifi { signal: None };
            }
        } else if let Some(ssid) = parse_ssid_line(t) {
            iface.kind = ConnectionKind::Wifi { signal: None };
            iface.ssid = Some(ssid);
        }
    }
    if let Some(done) = cur.take() {
        out.push(done);
    }
    out
}

/// Extract the SSID from an `ifconfig` `ssid ...` line. FreeBSD quotes SSIDs
/// containing spaces (`ssid "Foo Bar" channel ...`) and leaves unquoted ones
/// running up to ` channel`. Returns `None` for an empty/unassociated SSID.
fn parse_ssid_line(line: &str) -> Option<String> {
    let rest = line.strip_prefix("ssid ")?.trim_start();
    let ssid = if let Some(after_quote) = rest.strip_prefix('"') {
        match after_quote.find('"') {
            Some(end) => after_quote[..end].to_string(),
            None => after_quote.to_string(),
        }
    } else {
        rest.split(" channel")
            .next()
            .unwrap_or(rest)
            .trim()
            .to_string()
    };
    if ssid.is_empty() {
        None
    } else {
        Some(ssid)
    }
}

/// List configured interfaces as connection profiles (loopback excluded).
pub fn list_saved_connections() -> Vec<ConnectionProfile> {
    run_ifconfig(&["-a"])
        .as_deref()
        .map(parse_saved_connections)
        .unwrap_or_default()
}

pub(crate) fn parse_saved_connections(output: &str) -> Vec<ConnectionProfile> {
    parse_ifconfig_interfaces(output)
        .into_iter()
        .filter(|i| !i.is_loopback)
        .map(|i| ConnectionProfile {
            type_label: match i.kind {
                ConnectionKind::Ethernet => "Ethernet",
                ConnectionKind::Wifi { .. } => "WLAN",
                ConnectionKind::Vpn => "VPN",
                ConnectionKind::Other => "Andere",
            }
            .to_string(),
            name: i
                .ssid
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| i.name.clone()),
            active: i.active && i.has_inet,
        })
        .collect()
}

/// Scan for Wi-Fi networks on the first `wlan` interface. Best-effort: empty if
/// there is no wlan device or the scan fails.
pub fn scan_wifi_networks() -> Vec<WifiNetwork> {
    let Some(wlan) = first_wlan_interface() else {
        return Vec::new();
    };
    run_ifconfig(&[&wlan, "list", "scan"])
        .as_deref()
        .map(parse_scan)
        .unwrap_or_default()
}

/// Parse `ifconfig <wlan> list scan`. The SSID column is variable-width, so we
/// anchor on the BSSID (a MAC address): everything before it is the SSID, and
/// the whitespace-separated fields after it are CHAN RATE S:N INT CAPS...
pub(crate) fn parse_scan(output: &str) -> Vec<WifiNetwork> {
    let mut best: std::collections::BTreeMap<String, WifiNetwork> =
        std::collections::BTreeMap::new();
    for line in output.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let Some(mac_at) = find_bssid(line) else {
            continue;
        };
        let ssid = line[..mac_at].trim().to_string();
        if ssid.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line[mac_at..].split_whitespace().collect();
        // 0:BSSID 1:CHAN 2:RATE 3:S:N 4:INT 5..:CAPS
        let signal = fields
            .get(3)
            .and_then(|sn| sn.split(':').next())
            .and_then(|s| s.parse::<i32>().ok())
            .map(rssi_to_percent)
            .unwrap_or(0);
        let caps = fields.get(5..).map(|c| c.join(" ")).unwrap_or_default();
        let secured = caps.contains("RSN")
            || caps.contains("WPA")
            || caps.contains("WEP")
            || caps.contains("PRIVACY");
        let candidate = WifiNetwork {
            ssid: ssid.clone(),
            signal,
            secured,
            in_use: false,
        };
        best.entry(ssid)
            .and_modify(|existing| {
                if candidate.signal > existing.signal {
                    *existing = candidate.clone();
                }
            })
            .or_insert(candidate);
    }
    let mut nets: Vec<WifiNetwork> = best.into_values().collect();
    nets.sort_by(|a, b| b.signal.cmp(&a.signal).then(a.ssid.cmp(&b.ssid)));
    nets
}

/// Find the byte offset of the first BSSID (MAC `xx:xx:xx:xx:xx:xx`) token.
fn find_bssid(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let is_hex = |b: u8| b.is_ascii_hexdigit();
    let mut i = 0;
    while i + 17 <= bytes.len() {
        let w = &bytes[i..i + 17];
        let mac = w
            .iter()
            .enumerate()
            .all(|(k, &b)| if k % 3 == 2 { b == b':' } else { is_hex(b) });
        // Require a word boundary before the MAC so we don't match inside a name.
        if mac && (i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Map a dBm RSSI to a rough 0..=100 strength, clamping to the usual
/// [-100, -50] dBm working range.
fn rssi_to_percent(rssi: i32) -> u8 {
    let pct = 2 * (rssi + 100);
    pct.clamp(0, 100) as u8
}

fn first_wlan_interface() -> Option<String> {
    run_ifconfig(&["-l"])?
        .split_whitespace()
        .find(|name| name.starts_with("wlan"))
        .map(str::to_string)
}

/// Activate a saved connection. On FreeBSD wired links are managed by the base
/// system (rc.conf/dhclient), not by this shell; surfacing them is enough.
/// Logged so the action is observable.
pub fn activate_connection(name: &str) {
    tracing::info!(
        "network(freebsd): activate '{}' is a no-op; wired links are managed by rc.conf/dhclient",
        name
    );
}

/// Best-effort Wi-Fi association through wpa_cli. Only meaningful with a wlan
/// device whose wpa_supplicant control socket is reachable; logged on failure.
pub fn connect_wifi(ssid: &str, password: Option<&str>) {
    let ssid = ssid.to_string();
    let password = password.map(str::to_string);
    std::thread::spawn(move || {
        let Some(wlan) = first_wlan_interface() else {
            tracing::warn!("network(freebsd): no wlan device; cannot connect to {ssid}");
            return;
        };
        if !wpa_cli_connect(&wlan, &ssid, password.as_deref()) {
            tracing::warn!(
                "network(freebsd): wpa_cli association to {ssid} on {wlan} failed; \
                 configure wpa_supplicant manually"
            );
        }
    });
}

/// Drive wpa_cli to add, configure, and enable a network. Returns false on the
/// first failing step. Passwords go through `set_network ... psk` arguments to
/// wpa_cli, which talks to the local control socket only.
fn wpa_cli_connect(iface: &str, ssid: &str, password: Option<&str>) -> bool {
    let wpa = |args: &[&str]| -> bool {
        Command::new("wpa_cli")
            .args(["-i", iface])
            .args(args)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };
    let Some(id) = Command::new("wpa_cli")
        .args(["-i", iface, "add_network"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.lines().last().map(str::trim).map(str::to_string))
    else {
        return false;
    };
    let ssid_arg = format!("\"{ssid}\"");
    if !wpa(&["set_network", &id, "ssid", &ssid_arg]) {
        return false;
    }
    match password {
        Some(pw) => {
            let psk = format!("\"{pw}\"");
            if !wpa(&["set_network", &id, "psk", &psk]) {
                return false;
            }
        }
        None => {
            let _ = wpa(&["set_network", &id, "key_mgmt", "NONE"]);
        }
    }
    wpa(&["enable_network", &id]) && wpa(&["save_config"])
}

fn run_ifconfig(args: &[&str]) -> Option<String> {
    let output = Command::new("ifconfig")
        .env("LC_ALL", "C")
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIRED: &str = "\
re0: flags=1008843<UP,BROADCAST,RUNNING,SIMPLEX,MULTICAST> metric 0 mtu 1500
\toptions=8120b8<VLAN_MTU,VLAN_HWTAGGING,JUMBO_MTU,VLAN_HWCSUM>
\tether a0:1d:48:00:11:22
\tinet 192.168.1.238 netmask 0xffffff00 broadcast 192.168.1.255
\tmedia: Ethernet autoselect (1000baseT <full-duplex>)
\tstatus: active
\tnd6 options=29<PERFORMNUD,IFDISABLED,AUTO_LINKLOCAL>
lo0: flags=1008049<UP,LOOPBACK,RUNNING,MULTICAST> metric 0 mtu 16384
\tinet 127.0.0.1 netmask 0xff000000
\tgroups: lo
";

    const WIFI: &str = "\
wlan0: flags=8843<UP,BROADCAST,RUNNING,SIMPLEX,MULTICAST> metric 0 mtu 1500
\tinet 192.168.1.50 netmask 0xffffff00 broadcast 192.168.1.255
\tssid HomeNet channel 36 (5180 MHz 11a ht/40+) bssid 00:11:22:33:44:55
\tmedia: IEEE 802.11 OFDM/54Mbps mode 11a
\tstatus: associated
lo0: flags=1008049<UP,LOOPBACK,RUNNING,MULTICAST> metric 0 mtu 16384
\tinet 127.0.0.1 netmask 0xff000000
";

    #[test]
    fn parses_wired_connected() {
        assert_eq!(
            parse_ifconfig_state(WIRED),
            NetworkState::Connected {
                kind: ConnectionKind::Ethernet,
                connection_name: "re0".to_string(),
            }
        );
    }

    #[test]
    fn parses_wifi_with_ssid() {
        assert_eq!(
            parse_ifconfig_state(WIFI),
            NetworkState::Connected {
                kind: ConnectionKind::Wifi { signal: None },
                connection_name: "HomeNet".to_string(),
            }
        );
    }

    #[test]
    fn wired_wins_over_wifi() {
        let both = format!("{WIRED}{WIFI}");
        assert!(matches!(
            parse_ifconfig_state(&both),
            NetworkState::Connected {
                kind: ConnectionKind::Ethernet,
                ..
            }
        ));
    }

    #[test]
    fn down_interface_is_disconnected() {
        let down = "\
re0: flags=8802<BROADCAST,SIMPLEX,MULTICAST> metric 0 mtu 1500
\tmedia: Ethernet autoselect
\tstatus: no carrier
lo0: flags=1008049<UP,LOOPBACK,RUNNING,MULTICAST> metric 0 mtu 16384
\tinet 127.0.0.1 netmask 0xff000000
";
        assert_eq!(parse_ifconfig_state(down), NetworkState::Disconnected);
    }

    #[test]
    fn loopback_only_is_offline() {
        let lo = "\
lo0: flags=1008049<UP,LOOPBACK,RUNNING,MULTICAST> metric 0 mtu 16384
\tinet 127.0.0.1 netmask 0xff000000
";
        assert_eq!(parse_ifconfig_state(lo), NetworkState::Offline);
    }

    #[test]
    fn quoted_ssid_with_spaces() {
        assert_eq!(
            parse_ssid_line("ssid \"Cafe Guest\" channel 6"),
            Some("Cafe Guest".to_string())
        );
        assert_eq!(
            parse_ssid_line("ssid Home channel 11"),
            Some("Home".to_string())
        );
        assert_eq!(parse_ssid_line("ssid \"\""), None);
    }

    #[test]
    fn saved_connections_lists_non_loopback() {
        let profiles = parse_saved_connections(WIRED);
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "re0");
        assert_eq!(profiles[0].type_label, "Ethernet");
        assert!(profiles[0].active);
    }

    #[test]
    fn scan_parses_ssid_signal_and_security() {
        let scan = "\
SSID/MESH ID                     BSSID              CHAN RATE   S:N     INT CAPS
HomeNet                          00:11:22:33:44:55    6   54M  -55:-95  100 EPS  RSN HTCAP
OpenCafe                         66:77:88:99:aa:bb   11   54M  -70:-95  100 ES   HTCAP
HomeNet                          00:11:22:33:44:66    6   54M  -80:-95  100 EPS  RSN HTCAP
";
        let nets = parse_scan(scan);
        let names: Vec<&str> = nets.iter().map(|n| n.ssid.as_str()).collect();
        assert_eq!(names, vec!["HomeNet", "OpenCafe"]); // deduped, sorted by signal
        let home = nets.iter().find(|n| n.ssid == "HomeNet").unwrap();
        assert!(home.secured);
        assert_eq!(home.signal, rssi_to_percent(-55)); // strongest BSS kept
        let open = nets.iter().find(|n| n.ssid == "OpenCafe").unwrap();
        assert!(!open.secured);
    }

    #[test]
    fn rssi_scales_to_percent() {
        assert_eq!(rssi_to_percent(-50), 100);
        assert_eq!(rssi_to_percent(-75), 50);
        assert_eq!(rssi_to_percent(-100), 0);
        assert_eq!(rssi_to_percent(-120), 0);
    }

    #[test]
    fn classify_kind_maps_prefixes() {
        assert_eq!(classify_kind("re0"), ConnectionKind::Ethernet);
        assert_eq!(
            classify_kind("wlan0"),
            ConnectionKind::Wifi { signal: None }
        );
        assert_eq!(classify_kind("tun0"), ConnectionKind::Vpn);
        assert_eq!(classify_kind("wg0"), ConnectionKind::Vpn);
        assert_eq!(classify_kind("bridge0"), ConnectionKind::Other);
        assert_eq!(classify_kind("vlan10"), ConnectionKind::Other);
    }

    #[test]
    fn vpn_chosen_when_only_tunnel_up() {
        let tun = "\
tun0: flags=8051<UP,POINTOPOINT,RUNNING,MULTICAST> metric 0 mtu 1500
\tinet 10.8.0.2 --> 10.8.0.1 netmask 0xffffffff
\tstatus: active
";
        assert_eq!(
            parse_ifconfig_state(tun),
            NetworkState::Connected {
                kind: ConnectionKind::Vpn,
                connection_name: "tun0".to_string(),
            }
        );
    }

    #[test]
    fn find_bssid_locates_mac() {
        let line = "HomeNet                          00:11:22:33:44:55    6   54M";
        let at = find_bssid(line).unwrap();
        assert!(line[at..].starts_with("00:11:22:33:44:55"));
    }
}
