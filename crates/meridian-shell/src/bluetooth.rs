//! Read-only Bluetooth adapter status for the Settings "Bluetooth" page.
//!
//! Lists adapters from `/sys/class/bluetooth` (no external tooling). Pairing
//! and device details are a follow-up once Bluetooth hardware is available;
//! on a machine without an adapter the page says so honestly.

use std::fs;

pub fn bluetooth_rows() -> Vec<(String, String)> {
    rows_from(&list_adapters())
}

fn list_adapters() -> Vec<String> {
    let mut adapters: Vec<String> = fs::read_dir("/sys/class/bluetooth")
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|name| name.starts_with("hci"))
                .collect()
        })
        .unwrap_or_default();
    adapters.sort();
    adapters
}

fn rows_from(adapters: &[String]) -> Vec<(String, String)> {
    if adapters.is_empty() {
        vec![(
            "Status".to_string(),
            "Kein Bluetooth-Adapter gefunden".to_string(),
        )]
    } else {
        let mut rows = vec![("Adapter".to_string(), adapters.len().to_string())];
        for adapter in adapters {
            rows.push((adapter.clone(), "verfügbar".to_string()));
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::rows_from;

    #[test]
    fn no_adapter_message_when_empty() {
        assert_eq!(
            rows_from(&[]),
            vec![(
                "Status".to_string(),
                "Kein Bluetooth-Adapter gefunden".to_string()
            )]
        );
    }

    #[test]
    fn lists_adapters_with_count() {
        let rows = rows_from(&["hci0".to_string()]);
        assert_eq!(rows[0], ("Adapter".to_string(), "1".to_string()));
        assert!(rows.iter().any(|(l, v)| l == "hci0" && v == "verfügbar"));
    }
}
