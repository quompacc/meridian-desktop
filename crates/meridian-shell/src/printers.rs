use std::collections::{HashMap, HashSet};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrinterSnapshot {
    pub(crate) service: PrinterServiceState,
    pub(crate) default_printer: Option<String>,
    pub(crate) printers: Vec<PrinterInfo>,
    pub(crate) job_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrinterServiceState {
    Running,
    Stopped,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrinterInfo {
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) accepting: Option<bool>,
    pub(crate) status: String,
    pub(crate) is_default: bool,
    pub(crate) job_count: usize,
}

impl PrinterSnapshot {
    pub(crate) fn poll() -> Self {
        let scheduler = run_lpstat(&["-r"]);
        let service = match scheduler {
            Some(CommandOutput {
                success: true,
                stdout,
                ..
            }) if stdout.to_ascii_lowercase().contains("scheduler is running") => {
                PrinterServiceState::Running
            }
            Some(_) => PrinterServiceState::Stopped,
            None => PrinterServiceState::Unavailable,
        };

        if service != PrinterServiceState::Running {
            return Self {
                service,
                default_printer: None,
                printers: Vec::new(),
                job_count: 0,
            };
        }

        let printer_output = run_lpstat(&["-p"])
            .filter(|out| out.success)
            .map(|out| out.stdout)
            .unwrap_or_default();
        let default_output = run_lpstat(&["-d"])
            .filter(|out| out.success)
            .map(|out| out.stdout)
            .unwrap_or_default();
        let accepting_output = run_lpstat(&["-a"])
            .filter(|out| out.success)
            .map(|out| out.stdout)
            .unwrap_or_default();
        let jobs_output = run_lpstat(&["-o"])
            .filter(|out| out.success)
            .map(|out| out.stdout)
            .unwrap_or_default();

        parse_snapshot(
            service,
            &printer_output,
            &default_output,
            &accepting_output,
            &jobs_output,
        )
    }
}

pub(crate) fn parse_snapshot(
    service: PrinterServiceState,
    printers: &str,
    default_printer: &str,
    accepting: &str,
    jobs: &str,
) -> PrinterSnapshot {
    let default_name = parse_default_printer(default_printer);
    let accepting_by_name = parse_accepting(accepting);
    let jobs_by_name = parse_jobs(jobs);

    let mut rows: Vec<PrinterInfo> = printers
        .lines()
        .filter_map(|line| parse_printer_line(line, &default_name, &accepting_by_name, &jobs_by_name))
        .collect();
    rows.sort_by(|a, b| {
        b.is_default
            .cmp(&a.is_default)
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
    });

    let job_count = jobs_by_name.values().sum();
    PrinterSnapshot {
        service,
        default_printer: default_name,
        printers: rows,
        job_count,
    }
}

fn parse_printer_line(
    line: &str,
    default_name: &Option<String>,
    accepting_by_name: &HashMap<String, bool>,
    jobs_by_name: &HashMap<String, usize>,
) -> Option<PrinterInfo> {
    let line = line.trim();
    let rest = line.strip_prefix("printer ")?;
    let mut parts = rest.splitn(2, ' ');
    let name = parts.next()?.trim();
    if name.is_empty() {
        return None;
    }
    let status_tail = parts.next().unwrap_or("").trim();
    let enabled = !status_tail.to_ascii_lowercase().starts_with("disabled");
    let status = status_tail
        .split_once(". ")
        .map(|(first, _)| first)
        .unwrap_or(status_tail)
        .trim()
        .trim_end_matches('.')
        .to_string();
    let jobs = *jobs_by_name.get(name).unwrap_or(&0);
    Some(PrinterInfo {
        name: name.to_string(),
        enabled,
        accepting: accepting_by_name.get(name).copied(),
        status,
        is_default: default_name.as_deref() == Some(name),
        job_count: jobs,
    })
}

fn parse_default_printer(output: &str) -> Option<String> {
    for line in output.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix("system default destination: ") {
            let name = name.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn parse_accepting(output: &str) -> HashMap<String, bool> {
    let mut map = HashMap::new();
    for line in output.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let mut fields = line.split_whitespace();
        let Some(name) = fields.next() else { continue };
        let Some(state) = fields.next() else { continue };
        map.insert(name.to_string(), state == "accepting");
    }
    map
}

fn parse_jobs(output: &str) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    let mut seen_jobs = HashSet::new();
    for line in output.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Some(job_id) = line.split_whitespace().next() else {
            continue;
        };
        if !seen_jobs.insert(job_id.to_string()) {
            continue;
        }
        let Some((name, _id)) = job_id.rsplit_once('-') else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        *map.entry(name.to_string()).or_insert(0) += 1;
    }
    map
}

#[derive(Debug)]
struct CommandOutput {
    success: bool,
    stdout: String,
}

fn run_lpstat(args: &[&str]) -> Option<CommandOutput> {
    let output = Command::new("lpstat").env("LC_ALL", "C").args(args).output().ok()?;
    Some(CommandOutput {
        success: output.status.success(),
        stdout: String::from_utf8(output.stdout).ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_snapshot, PrinterServiceState};

    #[test]
    fn parse_snapshot_extracts_default_status_and_jobs() {
        let snapshot = parse_snapshot(
            PrinterServiceState::Running,
            "printer Office is idle. enabled since Mon 01 Jan 2024 10:00:00 AM\nprinter Label disabled since Mon 01 Jan 2024 10:01:00 AM - paused\n",
            "system default destination: Office\n",
            "Office accepting requests since Mon 01 Jan 2024 10:00:00 AM\nLabel not accepting requests since Mon 01 Jan 2024 10:00:00 AM\n",
            "Office-12 fakeuser 1024 Mon 01 Jan 2024 10:03:00 AM\nLabel-5 fakeuser 512 Mon 01 Jan 2024 10:04:00 AM\nOffice-13 fakeuser 1024 Mon 01 Jan 2024 10:05:00 AM\n",
        );

        assert_eq!(snapshot.default_printer.as_deref(), Some("Office"));
        assert_eq!(snapshot.job_count, 3);
        assert_eq!(snapshot.printers.len(), 2);
        assert_eq!(snapshot.printers[0].name, "Office");
        assert!(snapshot.printers[0].is_default);
        assert!(snapshot.printers[0].enabled);
        assert_eq!(snapshot.printers[0].accepting, Some(true));
        assert_eq!(snapshot.printers[0].job_count, 2);
        assert_eq!(snapshot.printers[1].accepting, Some(false));
        assert!(!snapshot.printers[1].enabled);
    }

    #[test]
    fn parse_snapshot_handles_no_default_and_empty_printer_list() {
        let snapshot = parse_snapshot(
            PrinterServiceState::Running,
            "",
            "no system default destination\n",
            "",
            "",
        );

        assert!(snapshot.default_printer.is_none());
        assert!(snapshot.printers.is_empty());
        assert_eq!(snapshot.job_count, 0);
    }
}
