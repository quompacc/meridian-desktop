//! Privileged system power requests through the platform session service.
//!
//! A request creates one short-lived worker thread because zbus is async while
//! the shell event loop is calloop-based. There is no timer or idle polling.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SystemPowerAction {
    PowerOff,
    Reboot,
    Suspend,
}

pub(crate) fn request(action: SystemPowerAction) -> bool {
    std::thread::Builder::new()
        .name("meridian-system-power".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    tracing::warn!(%error, ?action, "system power runtime creation failed");
                    return;
                }
            };

            if let Err(error) = runtime.block_on(request_platform(action)) {
                tracing::warn!(%error, ?action, "system power request failed");
            }
        })
        .is_ok()
}

#[cfg(any(target_os = "openbsd", target_os = "freebsd"))]
#[zbus::proxy(
    interface = "org.freedesktop.ConsoleKit.Manager",
    default_service = "org.freedesktop.ConsoleKit",
    default_path = "/org/freedesktop/ConsoleKit/Manager"
)]
trait ConsoleKitManager {
    fn power_off(&self, policykit_interactivity: bool) -> zbus::Result<()>;
    fn reboot(&self, policykit_interactivity: bool) -> zbus::Result<()>;
    fn suspend(&self, policykit_interactivity: bool) -> zbus::Result<()>;
}

#[cfg(any(target_os = "openbsd", target_os = "freebsd"))]
async fn request_platform(action: SystemPowerAction) -> zbus::Result<()> {
    let connection = zbus::Connection::system().await?;
    let manager = ConsoleKitManagerProxy::new(&connection).await?;
    let policykit_interactivity = true;
    match action {
        SystemPowerAction::PowerOff => manager.power_off(policykit_interactivity).await,
        SystemPowerAction::Reboot => manager.reboot(policykit_interactivity).await,
        SystemPowerAction::Suspend => manager.suspend(policykit_interactivity).await,
    }
}

#[cfg(target_os = "linux")]
#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait LoginManager {
    fn power_off(&self, interactive: bool) -> zbus::Result<()>;
    fn reboot(&self, interactive: bool) -> zbus::Result<()>;
    fn suspend(&self, interactive: bool) -> zbus::Result<()>;
}

#[cfg(target_os = "linux")]
async fn request_platform(action: SystemPowerAction) -> zbus::Result<()> {
    let connection = zbus::Connection::system().await?;
    let manager = LoginManagerProxy::new(&connection).await?;
    let interactive = true;
    match action {
        SystemPowerAction::PowerOff => manager.power_off(interactive).await,
        SystemPowerAction::Reboot => manager.reboot(interactive).await,
        SystemPowerAction::Suspend => manager.suspend(interactive).await,
    }
}

#[cfg(not(any(target_os = "openbsd", target_os = "freebsd", target_os = "linux")))]
async fn request_platform(_action: SystemPowerAction) -> zbus::Result<()> {
    Err(zbus::Error::Unsupported)
}
