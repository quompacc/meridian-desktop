mod nmcli;

pub use self::nmcli::NetworkController;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NetworkState {
    Connected {
        kind: ConnectionKind,
        connection_name: String,
    },
    Disconnected,
    Offline,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConnectionKind {
    Ethernet,
    Wifi { signal: Option<u8> },
    Vpn,
    Other,
}

impl NetworkState {
    pub fn icon_name(&self) -> &'static str {
        match self {
            Self::Connected { kind, .. } => match kind {
                ConnectionKind::Ethernet => "network-wired-symbolic",
                ConnectionKind::Wifi { signal } => match signal {
                    Some(value) if *value >= 75 => "network-wireless-signal-excellent-symbolic",
                    Some(value) if *value >= 25 => "network-wireless-signal-good-symbolic",
                    Some(_) => "network-wireless-signal-none-symbolic",
                    None => "network-wireless-signal-good-symbolic",
                },
                ConnectionKind::Vpn => "network-vpn-symbolic",
                ConnectionKind::Other => "network-wired-symbolic",
            },
            Self::Disconnected => "network-wired-disconnected-symbolic",
            Self::Offline => "network-offline-symbolic",
        }
    }

    /// Label/value rows for the Settings network page (read-only summary).
    pub fn settings_rows(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Offline => vec![("Status", "Nicht verfügbar".to_string())],
            Self::Disconnected => vec![("Status", "Getrennt".to_string())],
            Self::Connected {
                kind,
                connection_name,
            } => {
                let mut rows = vec![
                    ("Status", "Verbunden".to_string()),
                    ("Verbindung", connection_name.clone()),
                ];
                let (type_label, signal) = match kind {
                    ConnectionKind::Ethernet => ("Ethernet", None),
                    ConnectionKind::Wifi { signal } => ("WLAN", *signal),
                    ConnectionKind::Vpn => ("VPN", None),
                    ConnectionKind::Other => ("Andere", None),
                };
                rows.push(("Typ", type_label.to_string()));
                if let Some(sig) = signal {
                    rows.push(("Signal", format!("{sig} %")));
                }
                rows
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConnectionKind, NetworkState};

    #[test]
    fn settings_rows_summarize_state() {
        assert_eq!(
            NetworkState::Offline.settings_rows(),
            vec![("Status", "Nicht verfügbar".to_string())]
        );
        let wifi = NetworkState::Connected {
            kind: ConnectionKind::Wifi { signal: Some(72) },
            connection_name: "Home".to_string(),
        };
        assert_eq!(
            wifi.settings_rows(),
            vec![
                ("Status", "Verbunden".to_string()),
                ("Verbindung", "Home".to_string()),
                ("Typ", "WLAN".to_string()),
                ("Signal", "72 %".to_string()),
            ]
        );
        let eth = NetworkState::Connected {
            kind: ConnectionKind::Ethernet,
            connection_name: "Wired".to_string(),
        };
        assert_eq!(eth.settings_rows().len(), 3);
    }

    #[test]
    fn icon_name_for_each_variant() {
        assert_eq!(
            NetworkState::Connected {
                kind: ConnectionKind::Ethernet,
                connection_name: "Wired".to_string(),
            }
            .icon_name(),
            "network-wired-symbolic"
        );
        assert_eq!(
            NetworkState::Connected {
                kind: ConnectionKind::Wifi { signal: Some(80) },
                connection_name: "WiFi".to_string(),
            }
            .icon_name(),
            "network-wireless-signal-excellent-symbolic"
        );
        assert_eq!(
            NetworkState::Connected {
                kind: ConnectionKind::Wifi { signal: Some(30) },
                connection_name: "WiFi".to_string(),
            }
            .icon_name(),
            "network-wireless-signal-good-symbolic"
        );
        assert_eq!(
            NetworkState::Connected {
                kind: ConnectionKind::Wifi { signal: Some(5) },
                connection_name: "WiFi".to_string(),
            }
            .icon_name(),
            "network-wireless-signal-none-symbolic"
        );
        assert_eq!(
            NetworkState::Connected {
                kind: ConnectionKind::Wifi { signal: None },
                connection_name: "WiFi".to_string(),
            }
            .icon_name(),
            "network-wireless-signal-good-symbolic"
        );
        assert_eq!(
            NetworkState::Connected {
                kind: ConnectionKind::Vpn,
                connection_name: "VPN".to_string(),
            }
            .icon_name(),
            "network-vpn-symbolic"
        );
        assert_eq!(
            NetworkState::Connected {
                kind: ConnectionKind::Other,
                connection_name: "Other".to_string(),
            }
            .icon_name(),
            "network-wired-symbolic"
        );
        assert_eq!(
            NetworkState::Disconnected.icon_name(),
            "network-wired-disconnected-symbolic"
        );
        assert_eq!(
            NetworkState::Offline.icon_name(),
            "network-offline-symbolic"
        );
    }
}
