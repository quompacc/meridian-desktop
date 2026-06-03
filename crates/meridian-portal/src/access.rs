use std::collections::HashMap;

use tracing::info;
use zbus::zvariant::{ObjectPath, OwnedValue};

type Asv = HashMap<String, OwnedValue>;

/// Auto-allow `org.freedesktop.impl.portal.Access` implementation.
///
/// xdg-desktop-portal's Screenshot / ScreenCast / Camera (etc.) front-ends call
/// `AccessDialog` on an Access impl to render their own GTK-style consent
/// dialog before invoking the real backend. Without any Access impl on the
/// session bus xdp silently skips those portals at startup.
///
/// Meridian renders its own consent UI further down the stack (compositor
/// policy + shell modal triggered from `ScreenshotRequestOrigin::PortalDbus`),
/// so the right thing here is to make xdp's Access call a no-op: respond
/// `(0, {})` ("user allowed"), and let our own modal be the source of truth.
pub struct AccessImpl;

#[zbus::interface(name = "org.freedesktop.impl.portal.Access")]
impl AccessImpl {
    #[zbus(property)]
    fn version(&self) -> u32 {
        1
    }

    #[allow(clippy::too_many_arguments)]
    async fn access_dialog(
        &self,
        _handle: ObjectPath<'_>,
        app_id: &str,
        _parent_window: &str,
        title: &str,
        _subtitle: &str,
        _body: &str,
        _options: Asv,
    ) -> (u32, Asv) {
        info!(
            "AccessDialog auto-allow: title={title:?} app_id={app_id:?} (Meridian renders its own consent further down)"
        );
        (0, Asv::new())
    }
}
