use std::time::Duration;

use smithay_client_toolkit::reexports::calloop::{
    timer::{TimeoutAction, Timer},
    EventLoop,
};
use tracing::info;

mod buffer;
mod draw;
mod launcher;
mod panel;
mod wayland;

pub use draw::{Painter, TextRenderer};
pub use wayland::{ClickAction, ClickZone, IpcClient, Rect};
use wayland::{CommitReason, RepaintReason};

pub const PANEL_HEIGHT: u32 = 36;
pub const LAUNCHER_WIDTH: u32 = 520;
pub const LAUNCHER_HEIGHT: u32 = 420;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

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
