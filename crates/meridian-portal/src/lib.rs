mod file_chooser;

use tracing::info;
use zbus::connection::Builder;

pub const DBUS_NAME: &str = "org.freedesktop.impl.portal.desktop.meridian";
pub const OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let _conn = Builder::session()?
        .name(DBUS_NAME)?
        .serve_at(OBJECT_PATH, file_chooser::FileChooserImpl)?
        .build()
        .await?;

    info!("portal service ready: name={DBUS_NAME} path={OBJECT_PATH}");

    // Block forever — the connection keeps the service alive.
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}
