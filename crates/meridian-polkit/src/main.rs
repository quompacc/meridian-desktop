// meridian-polkit-agent — polkit authentication agent for the Meridian
// desktop. Long-lived daemon, registers with polkitd at startup,
// presents a layer-shell popup whenever a privileged action needs
// auth, runs PAM in a worker thread, and reports the result back.

mod auth;
mod dbus;
mod ui;
mod wayland;

use std::time::Duration;

use ab_glyph::FontRef;
use meridian_config::{MeridianConfig, ThemeManager};
use smithay_client_toolkit::reexports::calloop::{
    channel::{channel as cchannel, Event as ChannelEvent},
    EventLoop,
};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::wayland::{AppState, PamResult};

static FONT_DATA: &[u8] = include_bytes!("../../meridian-ui/assets/fonts/AdwaitaSans-Regular.ttf");

fn install_panic_logger() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::capture();
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<unknown panic>".to_string());
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown location>".to_string());
        let pid = std::process::id();
        let line = format!(
            "[{}] meridian-polkit-agent panic pid={}\n  at: {}\n  msg: {}\n  trace: {:?}\n\n",
            chrono_now(),
            pid,
            loc,
            payload,
            backtrace
        );
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/meridian-polkit-panic.log")
        {
            use std::io::Write;
            let _ = f.write_all(line.as_bytes());
        }
        default_hook(info);
    }));
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}", secs)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();
    install_panic_logger();

    let session_id = std::env::var("XDG_SESSION_ID").unwrap_or_else(|_| {
        warn!("XDG_SESSION_ID not set; falling back to \"c1\"");
        "c1".to_string()
    });
    let locale = std::env::var("LANG").unwrap_or_else(|_| "en_US.UTF-8".to_string());

    info!(
        session_id = %session_id,
        locale = %locale,
        "meridian-polkit-agent starting"
    );

    let font = FontRef::try_from_slice(FONT_DATA).expect("font load");

    // Pull the active theme from ~/.config/meridian/config.toml. If the
    // user switches themes at runtime, the agent keeps the old palette
    // until restart — fine for v1, the dbus thread can later observe
    // config changes and trigger a re-render.
    //
    // ThemeManager::new() loads the literal "default" theme by name;
    // the user's configured choice lives in MeridianConfig.general.theme.
    // Apply it explicitly, same dance as meridian-shell/init.rs.
    let meridian_config = MeridianConfig::load();
    let mut theme_manager = ThemeManager::new();
    if !meridian_config.general.theme.is_empty()
        && meridian_config.general.theme != theme_manager.current().name
    {
        if let Err(err) = theme_manager.set_theme(&meridian_config.general.theme) {
            warn!(
                "failed to load configured theme {:?}: {} (using built-in default)",
                meridian_config.general.theme, err
            );
        }
    }
    let theme = theme_manager.current().config.clone();
    info!(theme = %theme_manager.current().name, "theme loaded");

    // D-Bus thread sends BeginAuth/Cancel events to us.
    let dbus_rx = dbus::spawn(session_id, locale)?;

    // PAM worker threads send their results back via this channel.
    let (pam_tx, pam_rx) = cchannel::<PamResult>();

    // Wayland setup. AppState holds the wayland globals (filled in by the
    // registry callback) plus all UI state.
    let (conn, event_queue) = wayland::connect()?;
    let qh = event_queue.handle();
    let mut state = AppState::new(font, theme, pam_tx);

    let mut event_loop: EventLoop<'static, AppState> = EventLoop::try_new()?;
    let loop_handle = event_loop.handle();

    WaylandSource::new(conn, event_queue)
        .insert(loop_handle.clone())
        .map_err(|e| format!("calloop wayland source: {e}"))?;

    let qh_dbus = qh.clone();
    loop_handle
        .insert_source(dbus_rx, move |ev, _, state| {
            if let ChannelEvent::Msg(msg) = ev {
                match msg {
                    dbus::DbusEvent::BeginAuth(req) => state.on_auth_request(req, &qh_dbus),
                    dbus::DbusEvent::Cancel { cookie } => state.on_cancel_from_polkit(cookie),
                }
            }
        })
        .map_err(|e| format!("calloop dbus source: {e:?}"))?;

    let qh_pam = qh.clone();
    loop_handle
        .insert_source(pam_rx, move |ev, _, state| {
            if let ChannelEvent::Msg(result) = ev {
                state.on_pam_result(result);
                state.draw(&qh_pam);
            }
        })
        .map_err(|e| format!("calloop pam source: {e:?}"))?;

    info!("meridian-polkit-agent event loop ready");
    while state.running {
        event_loop.dispatch(Duration::from_secs(60), &mut state)?;
    }
    Ok(())
}
