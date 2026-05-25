use std::time::Duration;

use smithay_client_toolkit::reexports::calloop::{
    generic::Generic,
    timer::{TimeoutAction, Timer},
    EventLoop, Interest, Mode, PostAction,
};
use tracing::info;
use tracing_subscriber::EnvFilter;

mod app_view;
mod audio;
mod audio_popup;
mod buffer;
mod context_menu;
mod draw;
mod icons;
mod launcher;
mod network;
mod network_popup;
mod notification_popup;
mod notifications;
mod panel;
mod panel_view;
mod power_footer;
mod printers;
mod settings_view;
mod status_notifier;
mod status_notifier_popup;
mod thumbnail_popup;
mod ui;
mod ui_preview;
mod wayland;
mod widget_action;
mod widget_traversal;
mod workspaces;

use panel::PinnedApp;

pub use draw::{Painter, TextRenderer};
pub use wayland::{ClickAction, ClickZone, IpcClient, Rect};
use wayland::{CommitReason, RepaintReason};

const SHELL_IDLE_TICK: Duration = Duration::from_secs(1);
const NETWORK_IDLE_POLL: Duration = Duration::from_secs(15);
const NETWORK_ACTIVE_POLL: Duration = Duration::from_secs(2);

pub const PANEL_HEIGHT: u32 = 42;
pub const LAUNCHER_WIDTH: u32 = 880;
pub const LAUNCHER_HEIGHT: u32 = 620;
pub const CALENDAR_POPUP_WIDTH: u32 = 280;
pub const CALENDAR_POPUP_HEIGHT: u32 = 220;
pub const WORKSPACE_POPUP_WIDTH: u32 = 280;
pub const WORKSPACE_POPUP_HEIGHT: u32 = 200;
pub const NETWORK_POPUP_WIDTH: u32 = 280;
pub const NETWORK_POPUP_HEIGHT: u32 = 150;
pub const AUDIO_POPUP_WIDTH: u32 = 300;
pub const AUDIO_POPUP_HEIGHT: u32 = 172;
pub const AUDIO_POPUP_RIGHT_MARGIN: i32 = 126;
pub const SNI_MENU_RIGHT_MARGIN: i32 = 8;
pub const SHELL_POPUP_BOTTOM_MARGIN: i32 = 2;
pub const NETWORK_POPUP_RIGHT_MARGIN: i32 = 220;
pub const NOTIFICATION_WIDTH: u32 = 360;
pub const NOTIFICATION_HEIGHT: u32 = 90;
pub const NOTIFICATION_TOP_MARGIN: i32 = 20;
pub const NOTIFICATION_RIGHT_MARGIN: i32 = 12;
pub const THUMBNAIL_POPUP_HEIGHT: u32 = 136; // 2*PAD + THUMB_H
pub const THUMBNAIL_POPUP_MAX_WIDTH: u32 = 800;
pub const THUMBNAIL_THUMB_W: u32 = 200;
pub const THUMBNAIL_THUMB_H: u32 = 112;
pub const THUMBNAIL_CARD_GAP: u32 = 8;
pub const THUMBNAIL_CARD_PAD: u32 = 12;
pub const THUMBNAIL_HOVER_DELAY_MS: u128 = 400;
pub const THUMBNAIL_OPEN_TIMEOUT_MS: u128 = 1200;
pub const POWER_ARM_TIMEOUT_MS: u128 = 4000;

/// Duration of the login->desktop panel entrance (slide up + fade in).
pub const PANEL_INTRO_SECS: f32 = 0.7;
pub const THUMBNAIL_MAX_WINDOWS: usize = 3;
pub(crate) fn default_pinned_apps() -> Vec<PinnedApp> {
    vec![
        PinnedApp {
            label: "Term".to_string(),
            program: "konsole".to_string(),
            args: vec![],
            terminal: false,
            icon_name: Some("utilities-terminal".to_string()),
        },
        PinnedApp {
            label: "Web".to_string(),
            program: "chromium".to_string(),
            args: vec![],
            terminal: false,
            icon_name: Some("chromium".to_string()),
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

    insert_ipc_event_source(&mut event_loop, qh.clone(), &shell)?;
    insert_tick_timer(&mut event_loop, qh.clone())?;
    insert_network_poll_timer(&mut event_loop, qh.clone())?;
    insert_notifications_source(&mut event_loop, qh.clone())?;
    shell.status_notifier_tx = insert_status_notifier_source(&mut event_loop, qh.clone())?;
    insert_notification_expiry_timer(&mut event_loop, qh)?;

    while !shell.exit {
        event_loop.dispatch(Duration::from_millis(500), &mut shell)?;
    }

    Ok(())
}

fn redraw_after_ipc(
    shell: &mut wayland::MeridianShell,
    qh: &wayland_client::QueueHandle<wayland::MeridianShell>,
) {
    shell.draw_panel(qh, RepaintReason::Ipc);
    if shell.launcher_state.open {
        shell.draw_launcher(qh, RepaintReason::Ipc);
    } else if shell.launcher_dirty {
        shell.unmap_launcher(CommitReason::EventLoopTick);
        shell.launcher_dirty = false;
    }
    if shell.workspace_popup_open {
        shell.draw_workspace_popup(qh, RepaintReason::Ipc);
    }
    if shell.desktop_menu_open {
        shell.draw_desktop_menu(qh, RepaintReason::Ipc);
    }
    if shell.thumbnail_dirty && shell.thumbnail_popup_open {
        shell.draw_thumbnail_popup(qh, RepaintReason::Ipc);
    }
}

fn insert_ipc_event_source(
    event_loop: &mut EventLoop<'_, wayland::MeridianShell>,
    qh: wayland_client::QueueHandle<wayland::MeridianShell>,
    shell: &wayland::MeridianShell,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(stream) = shell.ipc.event_stream_clone() else {
        tracing::warn!(
            "meridian IPC event source unavailable at shell startup; falling back to tick polling"
        );
        return Ok(());
    };

    event_loop.handle().insert_source(
        Generic::new(stream, Interest::READ, Mode::Level),
        move |_, _, shell| {
            let changed = shell.poll_ipc();
            if changed {
                redraw_after_ipc(shell, &qh);
            }
            Ok(if shell.ipc.is_connected() {
                PostAction::Continue
            } else {
                PostAction::Remove
            })
        },
    )?;
    Ok(())
}

fn insert_status_notifier_source(
    event_loop: &mut EventLoop<'_, wayland::MeridianShell>,
    qh: wayland_client::QueueHandle<wayland::MeridianShell>,
) -> Result<
    Option<smithay_client_toolkit::reexports::calloop::channel::Sender<status_notifier::DbusEvent>>,
    Box<dyn std::error::Error>,
> {
    let source = match status_notifier::spawn() {
        Ok(source) => source,
        Err(e) => {
            tracing::warn!(error = %e, "status-notifier: failed to spawn dbus thread");
            return Ok(None);
        }
    };
    let tx = source.tx.clone();
    event_loop
        .handle()
        .insert_source(source.rx, move |event, _, shell| {
            use smithay_client_toolkit::reexports::calloop::channel::Event as ChEvent;
            match event {
                ChEvent::Msg(status_notifier::DbusEvent::ItemsChanged(items)) => {
                    tracing::info!(count = items.len(), "status-notifier: items changed");
                    shell.status_notifier_items = items;
                    shell.status_notifier_menu = None;
                    shell.panel_dirty = true;
                    shell.draw_panel(&qh, wayland::RepaintReason::Ipc);
                }
                ChEvent::Msg(status_notifier::DbusEvent::MenuLayout(menu_state)) => {
                    tracing::info!(
                        service = %menu_state.service,
                        menu_path = %menu_state.menu_path,
                        x = menu_state.point.x,
                        y = menu_state.point.y,
                        visible_items = menu_state.menu.visible_item_count(),
                        actionable_items = menu_state.menu.actionable_item_count(),
                        "status-notifier: dbusmenu layout ready for shell"
                    );
                    shell.open_status_notifier_menu(&qh, menu_state);
                }
                ChEvent::Closed => {
                    tracing::warn!("status-notifier: dbus channel closed");
                }
            }
        })?;
    Ok(Some(tx))
}

/// Periodic timer that prunes expired notifications from the queue and
/// re-renders the popup. Default expire is 5s per the freedesktop spec;
/// callers can pass `expire_timeout = 0` to opt out of auto-expiry.
/// Polling at 250ms is a deliberate-simple compromise — a future
/// optimisation could schedule the next wake at the soonest expiry.
fn insert_notification_expiry_timer(
    event_loop: &mut EventLoop<'_, wayland::MeridianShell>,
    qh: wayland_client::QueueHandle<wayland::MeridianShell>,
) -> Result<(), Box<dyn std::error::Error>> {
    event_loop.handle().insert_source(
        Timer::from_duration(SHELL_IDLE_TICK),
        move |_, _, shell| {
            let now = std::time::Instant::now();
            let before = shell.notifications.len();
            shell.notifications.retain(|n| !n.is_expired(now));
            if shell.notifications.len() != before {
                if shell.notifications.is_empty() {
                    shell.unmap_notification_popup(wayland::CommitReason::UnknownOther);
                } else {
                    shell.draw_notification_popup(&qh, wayland::RepaintReason::Clock);
                }
            }
            if let Some(path) = shell.poll_wallpaper_picker() {
                let mode = shell.wallpaper_mode;
                shell.apply_wallpaper(&qh, path, mode);
            }
            TimeoutAction::ToDuration(shell.notification_timer_interval())
        },
    )?;
    Ok(())
}

/// Spawn the freedesktop notification daemon dbus thread and register
/// its calloop channel with the shell event loop. Best-effort: if the
/// daemon fails to start we log and continue — the panel + launcher
/// still work without notifications.
fn insert_notifications_source(
    event_loop: &mut EventLoop<'_, wayland::MeridianShell>,
    qh: wayland_client::QueueHandle<wayland::MeridianShell>,
) -> Result<(), Box<dyn std::error::Error>> {
    let rx = match notifications::spawn() {
        Ok(rx) => rx,
        Err(e) => {
            tracing::warn!(error = %e, "notifications: failed to spawn dbus thread");
            return Ok(());
        }
    };
    event_loop
        .handle()
        .insert_source(rx, move |event, _, shell| {
            use smithay_client_toolkit::reexports::calloop::channel::Event as ChEvent;
            match event {
                ChEvent::Msg(notifications::DbusEvent::Notify(n)) => {
                    tracing::info!(
                        id = n.id,
                        app = %n.app,
                        title = %n.title,
                        body = %n.body,
                        urgency = ?n.urgency,
                        "notifications: incoming"
                    );
                    shell.notifications.push_back(n);
                    shell.notification_dirty = true;
                    shell.draw_notification_popup(&qh, wayland::RepaintReason::Ipc);
                }
                ChEvent::Msg(notifications::DbusEvent::Close(id)) => {
                    tracing::info!(id, "notifications: close request");
                    shell.notifications.retain(|n| n.id != id);
                    if shell.notifications.is_empty() {
                        shell.unmap_notification_popup(wayland::CommitReason::UnknownOther);
                    } else {
                        shell.draw_notification_popup(&qh, wayland::RepaintReason::Ipc);
                    }
                }
                ChEvent::Closed => {
                    tracing::warn!("notifications: dbus channel closed");
                }
            }
        })?;
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
                redraw_after_ipc(shell, &qh);
            }
            shell.tick(&qh);
            TimeoutAction::ToDuration(shell.tick_timer_interval())
        })?;

    Ok(())
}

fn insert_network_poll_timer(
    event_loop: &mut EventLoop<'_, wayland::MeridianShell>,
    qh: wayland_client::QueueHandle<wayland::MeridianShell>,
) -> Result<(), Box<dyn std::error::Error>> {
    event_loop
        .handle()
        .insert_source(Timer::immediate(), move |_, _, shell| {
            let previous = shell.network_controller.state().clone();
            let current = shell.network_controller.poll().clone();
            if current != previous {
                shell.draw_panel(&qh, RepaintReason::Ipc);
                if shell.network_popup_open {
                    shell.draw_network_popup(&qh, RepaintReason::Ipc);
                }
            }
            TimeoutAction::ToDuration(if shell.network_popup_open {
                NETWORK_ACTIVE_POLL
            } else {
                NETWORK_IDLE_POLL
            })
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
        assert_eq!(pinned[0].program, "konsole");
        assert_eq!(pinned[0].icon_name.as_deref(), Some("utilities-terminal"));
        assert_eq!(pinned[1].label, "Web");
        assert_eq!(pinned[1].program, "chromium");
        assert_eq!(pinned[1].icon_name.as_deref(), Some("chromium"));
        assert_eq!(pinned[2].label, "Files");
        assert_eq!(pinned[2].program, "dolphin");
        assert_eq!(pinned[2].icon_name.as_deref(), Some("org.kde.dolphin"));
    }
}
