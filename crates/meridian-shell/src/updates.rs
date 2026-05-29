//! Read-only package update status for the Settings "Updates" page.
//!
//! Uses `apt list --upgradable` (read-only, no lock). Degrades gracefully if
//! apt is unavailable. The parser is unit-tested against fixture output.

use std::process::Command;

const MAX_LISTED: usize = 12;

pub fn updates_rows() -> Vec<(String, String)> {
    match Command::new("apt").args(["list", "--upgradable"]).output() {
        Ok(out) if out.status.success() => {
            rows_from(&parse_upgradable(&String::from_utf8_lossy(&out.stdout)))
        }
        _ => vec![("Status".to_string(), "apt nicht verfügbar".to_string())],
    }
}

fn parse_upgradable(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|line| line.contains("upgradable from:"))
        .filter_map(|line| line.split('/').next())
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect()
}

fn rows_from(packages: &[String]) -> Vec<(String, String)> {
    if packages.is_empty() {
        return vec![("Status".to_string(), "System ist aktuell".to_string())];
    }
    let mut rows = vec![("Verfügbare Updates".to_string(), packages.len().to_string())];
    for pkg in packages.iter().take(MAX_LISTED) {
        rows.push((pkg.clone(), "aktualisierbar".to_string()));
    }
    if packages.len() > MAX_LISTED {
        rows.push((
            "…".to_string(),
            format!("+{} weitere", packages.len() - MAX_LISTED),
        ));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::{parse_upgradable, rows_from};

    #[test]
    fn parse_extracts_package_names() {
        let out = "Listing...\n\
            firefox/stable 120.0 amd64 [upgradable from: 119.0]\n\
            vim/stable 9.1 amd64 [upgradable from: 9.0]\n";
        assert_eq!(
            parse_upgradable(out),
            vec!["firefox".to_string(), "vim".to_string()]
        );
    }

    #[test]
    fn parse_ignores_header_and_blanks() {
        assert!(parse_upgradable("Listing...\n\n").is_empty());
    }

    #[test]
    fn rows_up_to_date_when_empty() {
        assert_eq!(
            rows_from(&[]),
            vec![("Status".to_string(), "System ist aktuell".to_string())]
        );
    }

    #[test]
    fn rows_count_and_overflow() {
        let packages: Vec<String> = (0..15).map(|i| format!("pkg{i}")).collect();
        let rows = rows_from(&packages);
        assert_eq!(
            rows[0],
            ("Verfügbare Updates".to_string(), "15".to_string())
        );
        assert!(rows.iter().any(|(l, _)| l == "…"));
    }
}
