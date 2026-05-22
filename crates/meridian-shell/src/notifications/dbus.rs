// dbus side of the notification daemon: bind the well-known service name
// `org.freedesktop.Notifications` on the session bus and implement the
// freedesktop spec methods. Incoming Notify/CloseNotification requests
// are forwarded as `DbusEvent`s into a calloop channel so the shell's
// calloop-based main loop can react.
//
// The dbus side runs on its own OS thread with a current-thread tokio
// runtime — that's the smallest async footprint that zbus v5 will
// accept. The main loop itself stays purely calloop-driven.
//
// v1 scope: Notify, CloseNotification, GetCapabilities,
// GetServerInformation. Signals (NotificationClosed, ActionInvoked) and
// inline actions are intentionally NOT wired yet — see ROADMAP A1.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use smithay_client_toolkit::reexports::calloop::channel as cchannel;
use tracing::{error, info, warn};
use zbus::{connection::Builder, fdo, interface, zvariant::OwnedValue};

use super::state::{expires_in_from_timeout, Notification, Urgency};

const SERVICE_NAME: &str = "org.freedesktop.Notifications";
const OBJECT_PATH: &str = "/org/freedesktop/Notifications";

/// Messages crossing from the dbus thread into the calloop main loop.
pub enum DbusEvent {
    /// A `Notify` call has produced a fully-formed [`Notification`]; the
    /// main loop should queue + render it.
    Notify(Notification),
    /// `CloseNotification(id)` — main loop should find and dismiss the
    /// popup if it is still on screen.
    Close(u32),
}

struct NotificationsService {
    /// Monotonic ID counter. 0 is reserved by the spec as "not a valid
    /// id"; we start at 1. If a caller passes a non-zero replaces_id we
    /// reuse it instead of allocating a new one.
    next_id: Arc<AtomicU32>,
    tx: cchannel::Sender<DbusEvent>,
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationsService {
    /// Spec method: <https://specifications.freedesktop.org/notification-spec/notification-spec-latest.html#command-notify>
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        _app_icon: String,
        summary: String,
        body: String,
        _actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> fdo::Result<u32> {
        let id = if replaces_id == 0 {
            self.next_id.fetch_add(1, Ordering::Relaxed)
        } else {
            replaces_id
        };
        let urgency = hints
            .get("urgency")
            .and_then(|v| u8::try_from(v).ok())
            .map(Urgency::from_byte)
            .unwrap_or_default();

        let notif = Notification {
            id,
            app: app_name,
            title: summary,
            body,
            urgency,
            created_at: Instant::now(),
            expires_in: expires_in_from_timeout(expire_timeout),
        };

        if let Err(e) = self.tx.send(DbusEvent::Notify(notif)) {
            warn!(error = ?e, "notifications: main loop channel closed; dropping");
            // Still return Ok with the id — the spec says the id is just
            // an opaque handle; the caller can't tell whether we'll
            // actually render it.
        }
        Ok(id)
    }

    /// Spec method: <https://specifications.freedesktop.org/notification-spec/notification-spec-latest.html#command-close-notification>
    async fn close_notification(&self, id: u32) -> fdo::Result<()> {
        if let Err(e) = self.tx.send(DbusEvent::Close(id)) {
            warn!(error = ?e, id, "notifications: main loop channel closed during Close");
        }
        Ok(())
    }

    /// Spec method: <https://specifications.freedesktop.org/notification-spec/notification-spec-latest.html#command-get-capabilities>
    /// v1 advertises only `body` (multi-line body supported). Add
    /// `actions` / `body-markup` / `icon-static` here as those features
    /// land in the renderer.
    async fn get_capabilities(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["body".to_string()])
    }

    /// Spec method: <https://specifications.freedesktop.org/notification-spec/notification-spec-latest.html#command-get-server-information>
    async fn get_server_information(&self) -> fdo::Result<(String, String, String, String)> {
        Ok((
            "meridian-shell".to_string(),
            "Meridian Desktop".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
            // Spec version we target — 1.2 is the current stable.
            "1.2".to_string(),
        ))
    }
}

/// Spawn the notification daemon on its own OS thread with a
/// current-thread tokio runtime. Returns the calloop `Channel` end the
/// main loop should register; the matching `Sender` is moved into the
/// dbus thread.
pub fn spawn() -> std::io::Result<cchannel::Channel<DbusEvent>> {
    let (tx, rx) = cchannel::channel::<DbusEvent>();
    std::thread::Builder::new()
        .name("notifications-dbus".to_string())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    error!(error = %e, "notifications: tokio runtime build failed; daemon disabled");
                    return;
                }
            };
            rt.block_on(async move {
                if let Err(e) = run(tx).await {
                    error!(
                        error = %e,
                        "notifications: dbus serve failed; daemon disabled until shell restarts"
                    );
                }
            });
        })?;
    Ok(rx)
}

async fn run(tx: cchannel::Sender<DbusEvent>) -> zbus::Result<()> {
    let service = NotificationsService {
        next_id: Arc::new(AtomicU32::new(1)),
        tx,
    };
    let _conn = Builder::session()?
        .name(SERVICE_NAME)?
        .serve_at(OBJECT_PATH, service)?
        .build()
        .await?;
    info!(
        service = SERVICE_NAME,
        path = OBJECT_PATH,
        "notifications: dbus daemon ready"
    );
    // Hold the connection alive forever; the thread exits only if the
    // process does.
    std::future::pending::<()>().await;
    Ok(())
}
