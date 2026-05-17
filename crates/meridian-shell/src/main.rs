use std::time::Duration;

use smithay_client_toolkit::reexports::calloop::{
    timer::{TimeoutAction, Timer},
    EventLoop,
};
use tracing_subscriber::EnvFilter;
use tracing::info;

mod buffer;
mod draw;
mod icons;
mod launcher;
mod panel;
mod ui;
mod wayland;
mod workspaces;

use panel::PinnedApp;

pub use draw::{Painter, TextRenderer};
pub use wayland::{ClickAction, ClickZone, IpcClient, Rect};
use wayland::{CommitReason, RepaintReason};

pub const PANEL_HEIGHT: u32 = 42;
pub const LAUNCHER_WIDTH: u32 = 720;
pub const LAUNCHER_HEIGHT: u32 = 520;
pub const CALENDAR_POPUP_WIDTH: u32 = 280;
pub const CALENDAR_POPUP_HEIGHT: u32 = 220;
pub const WORKSPACE_POPUP_WIDTH: u32 = 280;
pub const WORKSPACE_POPUP_HEIGHT: u32 = 200;
pub const SHELL_POPUP_BOTTOM_MARGIN: i32 = 2;

pub(crate) fn default_pinned_apps() -> Vec<PinnedApp> {
    vec![
        PinnedApp {
            label: "Term".to_string(),
            program: "kitty".to_string(),
            args: vec![],
            terminal: false,
            icon_name: Some("utilities-terminal".to_string()),
        },
        PinnedApp {
            label: "Web".to_string(),
            program: "firefox".to_string(),
            args: vec![],
            terminal: false,
            icon_name: Some("firefox".to_string()),
        },
        PinnedApp {
            label: "Files".to_string(),
            program: "dolphin".to_string(),
            args: vec![],
            terminal: false,
            icon_name: Some("org.kde.dolphin".to_string()),
        },
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"))
        .add_directive("usvg=error".parse().expect("static directive parses"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let mut event_loop = EventLoop::try_new()?;
    let (mut shell, qh) = wayland::initialize(&mut event_loop)?;

    insert_tick_timer(&mut event_loop, qh)?;
    insert_commit_stats_timer(&mut event_loop)?;

    while !shell.exit {
        event_loop.dispatch(Duration::from_millis(500), &mut shell)?;
    }

    Ok(())
}

fn insert_tick_timer(
    event_loop: &mut EventLoop<'_, wayland::MeridianShell>,
    qh: wayland_client::QueueHandle<wayland::MeridianShell>,
) -> Result<(), Box<dyn std::error::Error>> {
    event_loop
        .handle()
        .insert_source(Timer::immediate(), move |_, _, shell| {
            static TICK_LOGS: std::sync::atomic::AtomicUsize =
                std::sync::atomic::AtomicUsize::new(0);
            let tick_log = TICK_LOGS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if tick_log < 3 {
                info!(
                    "Event loop tick {} (panel_configured={}, launcher_configured={})",
                    tick_log + 1,
                    shell.panel_configured,
                    shell.launcher_configured
                );
            }
            let ipc_changed = shell.poll_ipc();
            if ipc_changed {
                shell.draw_panel(&qh, RepaintReason::Ipc);
                if shell.launcher_state.open {
                    shell.draw_launcher(&qh, RepaintReason::Ipc);
                } else if shell.launcher_dirty {
                    shell.unmap_launcher(CommitReason::EventLoopTick);
                    shell.launcher_dirty = false;
                }
                if shell.workspace_popup_open {
                    shell.draw_workspace_popup(&qh, RepaintReason::Ipc);
                }
            }
            shell.tick(&qh);
            TimeoutAction::ToDuration(Duration::from_millis(250))
        })?;

    Ok(())
}

fn insert_commit_stats_timer(
    event_loop: &mut EventLoop<'_, wayland::MeridianShell>,
) -> Result<(), Box<dyn std::error::Error>> {
    event_loop
        .handle()
        .insert_source(Timer::immediate(), move |_, _, shell| {
            shell.tick_commit_stats();
            TimeoutAction::ToDuration(Duration::from_secs(1))
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::default_pinned_apps;

    #[test]
    fn default_pinned_apps_contains_expected_entries() {
        let pinned = default_pinned_apps();
        assert_eq!(pinned.len(), 3);
        assert_eq!(pinned[0].label, "Term");
        assert_eq!(pinned[0].program, "kitty");
        assert_eq!(pinned[0].icon_name.as_deref(), Some("utilities-terminal"));
        assert_eq!(pinned[1].label, "Web");
        assert_eq!(pinned[1].program, "firefox");
        assert_eq!(pinned[1].icon_name.as_deref(), Some("firefox"));
        assert_eq!(pinned[2].label, "Files");
        assert_eq!(pinned[2].program, "dolphin");
        assert_eq!(pinned[2].icon_name.as_deref(), Some("org.kde.dolphin"));
    }
}
