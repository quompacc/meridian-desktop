//! Lifecycle owner for the optional WebKit panel process.

use std::{
    io,
    path::PathBuf,
    process::{Child, Command, ExitStatus},
};

pub(crate) struct WebPanelProcess {
    child: Child,
}

pub(crate) struct WebLauncherProcess {
    child: Child,
}

impl WebPanelProcess {
    pub(crate) fn spawn(light_theme: bool) -> io::Result<Self> {
        let executable = runtime_executable()?;
        let theme = if light_theme { "light" } else { "dark" };
        let child = Command::new(&executable)
            .arg("--surface=panel")
            .arg(format!("--theme={theme}"))
            .spawn()?;
        tracing::info!(pid = child.id(), path = ?executable, "managed WebKit panel started");
        Ok(Self { child })
    }

    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }
}

impl Drop for WebPanelProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

impl WebLauncherProcess {
    fn spawn(light_theme: bool) -> io::Result<Self> {
        let executable = runtime_executable()?;
        let theme = if light_theme { "light" } else { "dark" };
        let child = Command::new(&executable)
            .arg("--surface=launcher")
            .arg(format!("--theme={theme}"))
            .spawn()?;
        tracing::info!(pid = child.id(), path = ?executable, "managed WebKit launcher started");
        Ok(Self { child })
    }

    fn is_running(&mut self) -> io::Result<bool> {
        self.child.try_wait().map(|status| status.is_none())
    }
}

impl Drop for WebLauncherProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

impl crate::wayland::MeridianShell {
    pub(crate) fn toggle_web_launcher(&mut self) {
        let open = self
            .web_launcher
            .as_mut()
            .and_then(|process| process.is_running().ok())
            .unwrap_or(false);
        if open {
            self.web_launcher = None;
            tracing::info!("managed WebKit launcher closed");
            return;
        }
        self.web_launcher = None;
        match WebLauncherProcess::spawn(self.theme.appearance_is_light()) {
            Ok(process) => self.web_launcher = Some(process),
            Err(error) => tracing::error!("failed to start managed WebKit launcher: {error}"),
        }
    }
}

fn runtime_executable() -> io::Result<PathBuf> {
    let current = std::env::current_exe()?;
    let sibling = current.with_file_name("meridian-ui-runtime");
    if sibling.is_file() {
        return Ok(sibling);
    }
    Ok(PathBuf::from("meridian-ui-runtime"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_is_resolved_as_a_sibling_or_path_lookup() {
        let path = runtime_executable().expect("current executable path");
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("meridian-ui-runtime")
        );
    }
}
