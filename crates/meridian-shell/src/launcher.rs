use std::{env, path::Path, process::Command};

pub use meridian_app_catalog::DesktopApp;
use meridian_ipc::ShellCommand;
use tracing::{info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherCategory {
    Favorites,
    All,
    Internet,
    Office,
    Development,
    Graphics,
    System,
    Utilities,
}

impl LauncherCategory {
    pub const ALL: [Self; 8] = [
        Self::Favorites,
        Self::All,
        Self::Internet,
        Self::Office,
        Self::Development,
        Self::Graphics,
        Self::System,
        Self::Utilities,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Favorites => "Favoriten",
            Self::All => "Alle Anwendungen",
            Self::Internet => "Internet",
            Self::Office => "Büro",
            Self::Development => "Entwicklung",
            Self::Graphics => "Grafik",
            Self::System => "System",
            Self::Utilities => "Dienstprogramme",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LauncherState {
    pub open: bool,
    pub apps: Vec<DesktopApp>,
    pub category: LauncherCategory,
}

impl LauncherState {
    pub fn new_with_apps(apps: Vec<DesktopApp>) -> Self {
        Self {
            open: false,
            apps,
            category: LauncherCategory::Favorites,
        }
    }

    /// Flip the launcher open/closed. Does NOT rescan the desktop entries:
    /// scanning every applications dir (hundreds of fs reads + TryExec stats) is
    /// done off the event-loop thread via `MeridianShell::request_launcher_apps_refresh`
    /// so opening the launcher never blocks the UI (LAUNCH-2). The grid renders
    /// immediately from the cached `apps`; a fresh list swaps in within a tick.
    pub fn toggle(&mut self) -> bool {
        self.open = !self.open;
        self.open
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn reshuffle(&mut self) {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64 ^ d.as_secs().wrapping_mul(0x9e37_79b9))
            .unwrap_or(0xdead_beef);
        let mut rng = seed;
        let n = self.apps.len();
        for i in (1..n).rev() {
            rng = rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let j = (rng >> 33) as usize % (i + 1);
            self.apps.swap(i, j);
        }
    }

    pub(crate) fn launch_desktop_app(app: DesktopApp, ipc: &mut crate::IpcClient) {
        if app.program.trim().is_empty() {
            warn!("ignoring launch request for {}: empty argv", app.name);
            return;
        }

        info!(
            "requesting launch: {} (program={} args={:?})",
            app.name, app.program, app.args
        );
        let command = ShellCommand::LaunchApp {
            program: app.program.clone(),
            args: app.args.clone(),
            terminal: app.terminal,
        };
        if !ipc.send(&command) {
            warn!(
                "IPC unavailable, launching locally: program={} args={:?}",
                app.program, app.args
            );
            let mut local = if app.terminal {
                let Some(terminal_program) = terminal_program() else {
                    warn!(
                        "cannot launch terminal app {:?}: no terminal emulator found",
                        app.name
                    );
                    return;
                };
                let mut cmd = Command::new(terminal_program);
                cmd.arg("-e").arg(&app.program);
                cmd
            } else {
                Command::new(&app.program)
            };

            if let Ok(wayland_display) = std::env::var("WAYLAND_DISPLAY") {
                local.env("WAYLAND_DISPLAY", wayland_display);
            }
            if let Ok(xdg_runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
                local.env("XDG_RUNTIME_DIR", xdg_runtime_dir);
            }
            local
                .env("XDG_SESSION_TYPE", "wayland")
                .env("XDG_CURRENT_DESKTOP", "Meridian")
                .env("XDG_SESSION_DESKTOP", "meridian")
                .env("DESKTOP_SESSION", "meridian");
            if is_firefox_program(&app.program) && env::var_os("MOZ_ENABLE_WAYLAND").is_none() {
                local.env("MOZ_ENABLE_WAYLAND", "1");
            }

            match local.args(&app.args).spawn() {
                Ok(child) => info!("local launch pid: {}", child.id()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    warn!("failed to launch {}: program not found", app.name)
                }
                Err(err) => warn!("failed to launch {}: {}", app.name, err),
            }
        }
    }
}

fn is_executable_available(binary_or_path: &str) -> bool {
    if binary_or_path.is_empty() {
        return false;
    }
    let candidate = Path::new(binary_or_path);
    if candidate.is_absolute() {
        return is_executable_file(candidate);
    }

    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path)
        .map(|entry| entry.join(binary_or_path))
        .any(|candidate| is_executable_file(&candidate))
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn terminal_program() -> Option<String> {
    env::var("TERMINAL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            [
                "foot",
                "alacritty",
                "kitty",
                "wezterm",
                "ghostty",
                "kgx",
                "konsole",
                "xterm",
            ]
            .into_iter()
            .find(|candidate| is_executable_available(candidate))
            .map(str::to_string)
        })
}

fn is_firefox_program(program: &str) -> bool {
    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("firefox") || name.eq_ignore_ascii_case("firefox-esr")
        })
}

#[cfg(test)]
#[path = "launcher_tests.rs"]
mod tests;
