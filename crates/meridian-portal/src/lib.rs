mod access;
#[cfg(not(target_os = "openbsd"))]
mod file_chooser;
#[cfg(target_os = "openbsd")]
mod openbsd_sandbox;
mod screenshot;
mod settings;

use tracing::info;
use zbus::connection::Builder;

pub const DBUS_NAME: &str = "org.freedesktop.impl.portal.desktop.meridian";
pub const OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let builder = Builder::session()?.name(DBUS_NAME)?;

    // A child file picker would inherit unveil(2). Giving it arbitrary file
    // access would also expose the user's complete home directory to this
    // long-lived D-Bus backend. OpenBSD therefore routes FileChooser to the
    // separately packaged GTK portal and keeps this process tightly unveiled.
    #[cfg(not(target_os = "openbsd"))]
    let builder = builder.serve_at(OBJECT_PATH, file_chooser::FileChooserImpl)?;

    let conn = builder
        .serve_at(OBJECT_PATH, screenshot::ScreenshotImpl)?
        .serve_at(OBJECT_PATH, access::AccessImpl)?
        .serve_at(OBJECT_PATH, settings::SettingsImpl)?
        .build()
        .await?;

    #[cfg(target_os = "openbsd")]
    openbsd_sandbox::install()?;

    info!("portal service ready: name={DBUS_NAME} path={OBJECT_PATH}");

    // Live theme switch: poll the active color-scheme and emit SettingChanged so
    // already-running apps (GTK/Qt/Firefox) update without a relaunch. Polling
    // (cheap config/theme file reads) avoids an inotify dependency; theme
    // switches are rare so ~2s latency is fine.
    {
        let conn = conn.clone();
        tokio::spawn(async move {
            let mut last = settings::SettingsImpl::color_scheme();
            let mut last_sources = settings::appearance_source_mtimes();
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                // Cheap mtime fingerprint first: only re-parse config/theme
                // when something actually changed on disk (P3-4).
                let sources = settings::appearance_source_mtimes();
                if sources == last_sources {
                    continue;
                }
                last_sources = sources;
                let now = settings::SettingsImpl::color_scheme();
                if now == last {
                    continue;
                }
                last = now;
                match zbus::object_server::SignalEmitter::new(&conn, OBJECT_PATH) {
                    Ok(emitter) => {
                        let value = zbus::zvariant::Value::U32(now);
                        if let Err(e) = settings::SettingsImpl::setting_changed(
                            &emitter,
                            settings::APPEARANCE_NS,
                            settings::COLOR_SCHEME_KEY,
                            value,
                        )
                        .await
                        {
                            tracing::warn!("SettingChanged emit failed: {e}");
                        } else {
                            info!("appearance color-scheme changed -> {now} (signalled)");
                        }
                    }
                    Err(e) => tracing::warn!("signal emitter: {e}"),
                }
            }
        });
    }

    // Block forever — the connection keeps the service alive.
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}
