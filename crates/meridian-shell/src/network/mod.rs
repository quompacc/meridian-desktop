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

    pub fn summary(&self) -> String {
        match self {
            Self::Connected {
                kind,
                connection_name,
            } => {
                let kind_label = match kind {
                    ConnectionKind::Ethernet => "Ethernet",
                    ConnectionKind::Wifi { .. } => "WiFi",
                    ConnectionKind::Vpn => "VPN",
                    ConnectionKind::Other => "Network",
                };
                format!("{kind_label}: {connection_name}")
            }
            Self::Disconnected => "Disconnected".to_string(),
            Self::Offline => "Offline".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConnectionKind, NetworkState};

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
