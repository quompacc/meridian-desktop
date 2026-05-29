//! Read-only system information for the Settings "Uebersicht" page.
//!
//! Pure parsers (unit-tested) plus a best-effort `gather()` that reads `/proc`
//! and `/etc/os-release`. Any unreadable source degrades to a dash rather than
//! failing, so the page always renders.

use std::fs;

const UNKNOWN: &str = "—";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SystemInfo {
    pub os_name: String,
    pub hostname: String,
    pub kernel: String,
    pub uptime: String,
    pub cpu: String,
    pub memory: String,
}

impl SystemInfo {
    pub fn gather() -> Self {
        let read = |path: &str| fs::read_to_string(path).ok();

        let os_name = read("/etc/os-release")
            .and_then(|s| parse_os_pretty_name(&s))
            .unwrap_or_else(|| UNKNOWN.to_string());

        let hostname = read("/proc/sys/kernel/hostname")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| UNKNOWN.to_string());

        let ostype = read("/proc/sys/kernel/ostype")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Linux".to_string());
        let osrelease = read("/proc/sys/kernel/osrelease")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let kernel = if osrelease.is_empty() {
            UNKNOWN.to_string()
        } else {
            format!("{ostype} {osrelease}")
        };

        let uptime = read("/proc/uptime")
            .and_then(|s| parse_uptime_seconds(&s))
            .map(format_uptime)
            .unwrap_or_else(|| UNKNOWN.to_string());

        let cpu = read("/proc/cpuinfo")
            .map(|s| {
                let (model, count) = parse_cpuinfo(&s);
                format_cpu(model, count)
            })
            .unwrap_or_else(|| UNKNOWN.to_string());

        let memory = read("/proc/meminfo")
            .map(|s| {
                let (total, available) = parse_meminfo_kib(&s);
                format_memory(total, available)
            })
            .unwrap_or_else(|| UNKNOWN.to_string());

        Self {
            os_name,
            hostname,
            kernel,
            uptime,
            cpu,
            memory,
        }
    }

    /// Label/value pairs for the settings view, in display order.
    pub fn rows(&self) -> [(&'static str, &str); 6] {
        [
            ("System", self.os_name.as_str()),
            ("Hostname", self.hostname.as_str()),
            ("Kernel", self.kernel.as_str()),
            ("Laufzeit", self.uptime.as_str()),
            ("Prozessor", self.cpu.as_str()),
            ("Speicher", self.memory.as_str()),
        ]
    }
}

fn parse_os_pretty_name(s: &str) -> Option<String> {
    s.lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|rest| rest.trim().trim_matches('"').to_string())
        .filter(|value| !value.is_empty())
}

fn parse_uptime_seconds(s: &str) -> Option<u64> {
    s.split_whitespace()
        .next()?
        .parse::<f64>()
        .ok()
        .map(|secs| secs as u64)
}

fn format_uptime(secs: u64) -> String {
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let minutes = (secs % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

fn parse_cpuinfo(s: &str) -> (Option<String>, usize) {
    let mut model = None;
    let mut count = 0usize;
    for line in s.lines() {
        if line.starts_with("processor") {
            count += 1;
        }
        if model.is_none() {
            if let Some(rest) = line.strip_prefix("model name") {
                if let Some((_, value)) = rest.split_once(':') {
                    let value = value.trim();
                    if !value.is_empty() {
                        model = Some(value.to_string());
                    }
                }
            }
        }
    }
    (model, count)
}

fn format_cpu(model: Option<String>, count: usize) -> String {
    match (model, count) {
        (Some(m), n) if n > 0 => format!("{m} ({n}×)"),
        (Some(m), _) => m,
        (None, n) if n > 0 => format!("{n} CPUs"),
        _ => UNKNOWN.to_string(),
    }
}

fn parse_meminfo_kib(s: &str) -> (Option<u64>, Option<u64>) {
    let field = |key: &str| {
        s.lines().find_map(|line| {
            line.strip_prefix(key)?
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
        })
    };
    (field("MemTotal:"), field("MemAvailable:"))
}

fn format_memory(total: Option<u64>, available: Option<u64>) -> String {
    match (total, available) {
        (Some(t), Some(a)) => format!("{} / {} belegt", gib(t.saturating_sub(a)), gib(t)),
        (Some(t), None) => gib(t),
        _ => UNKNOWN.to_string(),
    }
}

fn gib(kib: u64) -> String {
    format!("{:.1} GiB", kib as f64 / 1_048_576.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_pretty_name_handles_quotes_and_absence() {
        let os = "NAME=Debian\nPRETTY_NAME=\"Debian GNU/Linux 13 (trixie)\"\nID=debian\n";
        assert_eq!(
            parse_os_pretty_name(os).as_deref(),
            Some("Debian GNU/Linux 13 (trixie)")
        );
        assert_eq!(parse_os_pretty_name("ID=debian\n"), None);
        assert_eq!(parse_os_pretty_name("PRETTY_NAME=\"\"\n"), None);
    }

    #[test]
    fn uptime_parses_first_field_and_formats() {
        assert_eq!(parse_uptime_seconds("12345.67 99999.00\n"), Some(12345));
        assert_eq!(parse_uptime_seconds("garbage"), None);
        assert_eq!(format_uptime(90), "1m");
        assert_eq!(format_uptime(3_660), "1h 1m");
        assert_eq!(format_uptime(90_061), "1d 1h 1m");
    }

    #[test]
    fn cpuinfo_counts_processors_and_takes_first_model() {
        let info = "processor\t: 0\nmodel name\t: AMD Ryzen 7\nprocessor\t: 1\nmodel name\t: AMD Ryzen 7\n";
        let (model, count) = parse_cpuinfo(info);
        assert_eq!(model.as_deref(), Some("AMD Ryzen 7"));
        assert_eq!(count, 2);
        assert_eq!(format_cpu(model, count), "AMD Ryzen 7 (2×)");
        assert_eq!(format_cpu(None, 4), "4 CPUs");
    }

    #[test]
    fn meminfo_parses_kib_and_formats_used_over_total() {
        let mem = "MemTotal:        8192000 kB\nMemFree:         1000000 kB\nMemAvailable:    4096000 kB\n";
        let (total, available) = parse_meminfo_kib(mem);
        assert_eq!(total, Some(8_192_000));
        assert_eq!(available, Some(4_096_000));
        // used = 8192000 - 4096000 = 4096000 kiB = 3.9 GiB; total ~7.8 GiB
        assert_eq!(format_memory(total, available), "3.9 GiB / 7.8 GiB belegt");
    }

    #[test]
    fn missing_meminfo_fields_degrade_gracefully() {
        assert_eq!(parse_meminfo_kib("Bogus: 1 kB\n"), (None, None));
        assert_eq!(format_memory(None, None), UNKNOWN);
    }
}
