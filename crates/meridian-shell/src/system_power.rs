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
    fn inhibit(
        &self,
        what: &str,
        who: &str,
        why: &str,
        mode: &str,
    ) -> zbus::Result<zbus::zvariant::OwnedFd>;
    #[zbus(signal)]
    fn prepare_for_sleep(&self, active: bool) -> zbus::Result<()>;
}

#[cfg(any(target_os = "openbsd", target_os = "freebsd"))]
async fn request_platform(action: SystemPowerAction) -> zbus::Result<()> {
    let connection = zbus::Connection::system().await?;
    let manager = ConsoleKitManagerProxy::new(&connection).await?;
    let policykit_interactivity = true;
    match action {
        SystemPowerAction::PowerOff => manager.power_off(policykit_interactivity).await,
        SystemPowerAction::Reboot => manager.reboot(policykit_interactivity).await,
        SystemPowerAction::Suspend => request_consolekit_suspend(&manager).await,
    }
}

#[cfg(target_os = "openbsd")]
async fn request_consolekit_suspend(manager: &ConsoleKitManagerProxy<'_>) -> zbus::Result<()> {
    use std::future::poll_fn;
    use zbus::export::futures_core::Stream;

    let mut inhibitor = Some(
        manager
            .inhibit(
                "sleep",
                "Meridian",
                "Synchronize the native compositor sleep lifecycle",
                "delay",
            )
            .await?,
    );
    let signals = manager.receive_prepare_for_sleep().await?;
    let mut signals = Box::pin(signals);
    let mut suspend = Box::pin(manager.suspend(true));
    let mut ipc = crate::wayland::IpcClient::connect();
    let mut prepared = false;
    let mut resumed = false;

    loop {
        tokio::select! {
            result = &mut suspend => {
                if prepared && !resumed && !ipc.send(&meridian_ipc::ShellCommand::PowerResume) {
                    tracing::warn!("failed to send compositor resume transition");
                }
                return result;
            }
            signal = poll_fn(|cx| signals.as_mut().poll_next(cx)) => {
                let Some(signal) = signal else {
                    return Err(zbus::Error::Failure(
                        "ConsoleKit sleep signal stream ended".to_string(),
                    ));
                };
                if signal.args()?.active {
                    if !prepared {
                        prepare_compositor_for_sleep(&mut ipc).await?;
                        prepared = true;
                        drop(inhibitor.take());
                    }
                } else if !resumed {
                    resumed = ipc.send(&meridian_ipc::ShellCommand::PowerResume);
                    if !resumed {
                        tracing::warn!("failed to send compositor resume transition");
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "openbsd")]
async fn prepare_compositor_for_sleep(ipc: &mut crate::wayland::IpcClient) -> zbus::Result<()> {
    use std::time::{Duration, Instant};

    const PREPARE_TIMEOUT: Duration = Duration::from_secs(2);
    const PREPARE_POLL_INTERVAL: Duration = Duration::from_millis(10);

    if !ipc.send(&meridian_ipc::ShellCommand::PowerPrepareSleep) {
        return Err(zbus::Error::Failure(
            "failed to request compositor sleep preparation".to_string(),
        ));
    }

    let deadline = Instant::now() + PREPARE_TIMEOUT;
    loop {
        if ipc
            .poll()
            .into_iter()
            .any(|event| event == meridian_ipc::ShellEvent::PowerSleepPrepared)
        {
            break;
        }
        if Instant::now() >= deadline {
            let _ = ipc.send(&meridian_ipc::ShellCommand::PowerResume);
            return Err(zbus::Error::Failure(
                "compositor sleep preparation timed out".to_string(),
            ));
        }
        tokio::time::sleep(PREPARE_POLL_INTERVAL).await;
    }

    tracing::info!("compositor sleep preparation acknowledged");
    Ok(())
}

#[cfg(target_os = "freebsd")]
async fn request_consolekit_suspend(manager: &ConsoleKitManagerProxy<'_>) -> zbus::Result<()> {
    manager.suspend(true).await
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
