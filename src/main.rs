use std::{
    path::PathBuf,
    process::{Child, Command},
    time::Duration,
};

use meridian_compositor::{
    backend::{drm::init_drm, winit::init_winit},
    protocols::xwayland::start_xwayland,
    state::MeridianState,
};
use smithay::reexports::{
    calloop::{
        timer::{TimeoutAction, Timer},
        EventLoop,
    },
    wayland_server::Display,
};
use tracing::{info, warn};

struct ShellWatchdog {
    child: Option<Child>,
    last_start: std::time::Instant,
    shutting_down: bool,
    wayland_display: String,
    shell_binary: PathBuf,
}

impl ShellWatchdog {
    fn new(wayland_display: String) -> Self {
        let shell_binary = find_shell_binary();
        info!("meridian-shell binary: {:?}", shell_binary);
        Self {
            child: None,
            last_start: std::time::Instant::now() - Duration::from_secs(5),
            shutting_down: false,
            wayland_display,
            shell_binary,
        }
    }

    fn start(&mut self) {
        if self.shutting_down {
            return;
        }
        info!(
            "starting meridian-shell: {:?} (WAYLAND_DISPLAY={})",
            self.shell_binary, self.wayland_display
        );
        match Command::new(&self.shell_binary)
            .env("WAYLAND_DISPLAY", &self.wayland_display)
            .env(
                "XDG_RUNTIME_DIR",
                std::env::var("XDG_RUNTIME_DIR")
                    .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::geteuid() })),
            )
            .spawn()
        {
            Ok(child) => {
                info!("meridian-shell started (pid {})", child.id());
                self.child = Some(child);
                self.last_start = std::time::Instant::now();
            }
            Err(err) => {
                warn!(
                    "failed to start meridian-shell {:?}: {}",
                    self.shell_binary, err
                );
            }
        }
    }

    fn watch(&mut self) {
        if self.shutting_down {
            return;
        }

        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    info!("meridian-shell exited: {}", status);
                    self.child = None;
                }
                Ok(None) => return,
                Err(err) => {
                    warn!("meridian-shell wait error: {}", err);
                    self.child = None;
                }
            }
        }

        if self.child.is_none() && self.last_start.elapsed() >= Duration::from_secs(2) {
            self.start();
        }
    }

    fn stop(&mut self) {
        self.shutting_down = true;
        if let Some(mut child) = self.child.take() {
            info!("stopping meridian-shell (pid {})", child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for ShellWatchdog {
    fn drop(&mut self) {
        self.stop();
    }
}

fn find_shell_binary() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("meridian-shell");
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from("meridian-shell")
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let mut event_loop: EventLoop<'static, MeridianState> = EventLoop::try_new()?;
    let display: Display<MeridianState> = Display::new()?;
    let mut state = MeridianState::new(&mut event_loop, display)?;

    let in_session = std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok();

    if in_session {
        info!("Detected parent display – using winit backend");
        init_winit(&mut event_loop, &mut state)?;
    } else {
        info!("No parent display – using DRM/KMS backend");
        init_drm(&mut event_loop, &mut state)?;
    }

    info!("Meridian running on socket: {:?}", state.socket_name);
    let socket_name = state.socket_name.to_string_lossy().to_string();
    unsafe { std::env::set_var("WAYLAND_DISPLAY", &state.socket_name) };

    start_xwayland(&mut state);

    let shell_disabled =
        env_flag_enabled("MERIDIAN_DRM_DISABLE_SHELL") || env_flag_enabled("MERIDIAN_NO_SHELL");

    if shell_disabled {
        info!("shell auto-start disabled by env (MERIDIAN_DRM_DISABLE_SHELL or MERIDIAN_NO_SHELL)");
    }

    event_loop.handle().insert_source(
        Timer::from_duration(Duration::from_millis(100)),
        |_, _, state| {
            state.poll_ipc();
            TimeoutAction::ToDuration(Duration::from_millis(100))
        },
    )?;

    if !shell_disabled {
        let mut watchdog = ShellWatchdog::new(socket_name);
        watchdog.start();
        event_loop.handle().insert_source(
            Timer::from_duration(Duration::from_secs(2)),
            move |_, _, _| {
                watchdog.watch();
                TimeoutAction::ToDuration(Duration::from_secs(2))
            },
        )?;
    }

    event_loop.run(None, &mut state, move |_| {})?;
    Ok(())
}
