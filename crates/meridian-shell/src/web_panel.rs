//! Lifecycle owner for the optional WebKit panel process.

use std::{
    io::{self, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
};

use meridian_ipc::{
    AppearanceSnapshot, AppearanceTheme, AppearanceWallpaperMode, QuickSettingsAudio,
    QuickSettingsBattery, QuickSettingsNetwork, QuickSettingsSnapshot, QuickSettingsWifiNetwork,
};

pub(crate) struct WebPanelProcess {
    child: Child,
    control: ChildStdin,
}

pub(crate) struct WebLauncherProcess {
    child: Child,
    control: ChildStdin,
    visible: bool,
}

pub(crate) struct WebQuickSettingsProcess {
    child: Child,
    control: ChildStdin,
    visible: bool,
}

impl WebPanelProcess {
    pub(crate) fn spawn(light_theme: bool) -> io::Result<Self> {
        let executable = runtime_executable()?;
        let theme = if light_theme { "light" } else { "dark" };
        let mut child = Command::new(&executable)
            .arg("--surface=panel")
            .arg(format!("--theme={theme}"))
            .arg("--persistent-panel")
            .stdin(Stdio::piped())
            .spawn()?;
        let control = child.stdin.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "panel stdin was not piped")
        })?;
        tracing::info!(pid = child.id(), path = ?executable, "managed WebKit panel started");
        Ok(Self { child, control })
    }

    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    pub(crate) fn set_theme(&mut self, light_theme: bool) -> io::Result<()> {
        let theme = if light_theme { "light" } else { "dark" };
        writeln!(self.control, "theme {theme}")?;
        self.control.flush()
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
        let mut child = Command::new(&executable)
            .arg("--surface=launcher")
            .arg(format!("--theme={theme}"))
            .arg("--persistent-launcher")
            .stdin(Stdio::piped())
            .spawn()?;
        let control = child.stdin.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "launcher stdin was not piped")
        })?;
        tracing::info!(pid = child.id(), path = ?executable, "managed WebKit launcher started");
        Ok(Self {
            child,
            control,
            visible: false,
        })
    }

    fn is_running(&mut self) -> io::Result<bool> {
        self.child.try_wait().map(|status| status.is_none())
    }

    fn set_visible(&mut self, visible: bool) -> io::Result<()> {
        self.control
            .write_all(if visible { b"show\n" } else { b"hide\n" })?;
        self.control.flush()?;
        self.visible = visible;
        Ok(())
    }

    fn show_settings(&mut self, snapshot: &AppearanceSnapshot) -> io::Result<()> {
        let snapshot = canonical_appearance_json(snapshot)?;
        writeln!(self.control, "show-settings {snapshot}")?;
        self.control.flush()?;
        self.visible = true;
        Ok(())
    }

    fn update_appearance(&mut self, snapshot: &AppearanceSnapshot) -> io::Result<()> {
        let snapshot = canonical_appearance_json(snapshot)?;
        writeln!(self.control, "appearance {snapshot}")?;
        self.control.flush()
    }

    fn set_theme(&mut self, theme: AppearanceTheme) -> io::Result<()> {
        writeln!(self.control, "theme {}", theme.config_name())?;
        self.control.flush()
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

impl WebQuickSettingsProcess {
    fn spawn(light_theme: bool) -> io::Result<Self> {
        let executable = runtime_executable()?;
        let theme = if light_theme { "light" } else { "dark" };
        let mut child = Command::new(&executable)
            .arg("--surface=quick-settings")
            .arg(format!("--theme={theme}"))
            .arg("--persistent-quick-settings")
            .stdin(Stdio::piped())
            .spawn()?;
        let control = child.stdin.take().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::BrokenPipe,
                "quick-settings stdin was not piped",
            )
        })?;
        tracing::info!(pid = child.id(), path = ?executable, "managed WebKit Quick Settings started");
        Ok(Self {
            child,
            control,
            visible: false,
        })
    }

    fn is_running(&mut self) -> io::Result<bool> {
        self.child.try_wait().map(|status| status.is_none())
    }

    fn set_visible(&mut self, visible: bool) -> io::Result<()> {
        self.control
            .write_all(if visible { b"show\n" } else { b"hide\n" })?;
        self.control.flush()?;
        self.visible = visible;
        Ok(())
    }

    fn show_with_snapshot(&mut self, snapshot: &QuickSettingsSnapshot) -> io::Result<()> {
        let snapshot = serde_json::to_string(snapshot)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        writeln!(self.control, "show {snapshot}")?;
        self.control.flush()?;
        self.visible = true;
        Ok(())
    }

    fn update_snapshot(&mut self, snapshot: &QuickSettingsSnapshot) -> io::Result<()> {
        let snapshot = serde_json::to_string(snapshot)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        writeln!(self.control, "update {snapshot}")?;
        self.control.flush()
    }

    fn set_theme(&mut self, theme: AppearanceTheme) -> io::Result<()> {
        writeln!(self.control, "theme {}", theme.config_name())?;
        self.control.flush()
    }
}

impl Drop for WebQuickSettingsProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

impl crate::wayland::MeridianShell {
    pub(crate) fn prewarm_web_launcher(&mut self) {
        if !self.web_panel_enabled || self.web_launcher.is_some() {
            return;
        }
        match WebLauncherProcess::spawn(self.theme.appearance_is_light()) {
            Ok(process) => {
                self.web_launcher = Some(process);
                tracing::info!("managed WebKit launcher prewarming while hidden");
            }
            Err(error) => tracing::error!("failed to prewarm managed WebKit launcher: {error}"),
        }
    }

    pub(crate) fn toggle_web_launcher(&mut self) {
        if let Some(process) = self.web_launcher.as_mut() {
            if process.is_running().unwrap_or(false) {
                let visible = !process.visible;
                match process.set_visible(visible) {
                    Ok(()) => tracing::info!(visible, "managed WebKit launcher visibility changed"),
                    Err(error) => {
                        tracing::error!("failed to control managed WebKit launcher: {error}");
                        self.web_launcher = None;
                    }
                }
                return;
            }
        }
        self.web_launcher = None;
        match WebLauncherProcess::spawn(self.theme.appearance_is_light()) {
            Ok(mut process) => match process.set_visible(true) {
                Ok(()) => self.web_launcher = Some(process),
                Err(error) => tracing::error!("failed to show managed WebKit launcher: {error}"),
            },
            Err(error) => tracing::error!("failed to start managed WebKit launcher: {error}"),
        }
    }

    pub(crate) fn prewarm_web_quick_settings(&mut self) {
        if !self.web_panel_enabled || self.web_quick_settings.is_some() {
            return;
        }
        match WebQuickSettingsProcess::spawn(self.theme.appearance_is_light()) {
            Ok(process) => {
                self.web_quick_settings = Some(process);
                tracing::info!("managed WebKit Quick Settings prewarming while hidden");
            }
            Err(error) => {
                tracing::error!("failed to prewarm managed WebKit Quick Settings: {error}")
            }
        }
    }

    pub(crate) fn open_web_system_settings(&mut self) {
        self.hide_web_quick_settings();
        let snapshot = self.appearance_snapshot();
        let running = self
            .web_launcher
            .as_mut()
            .is_some_and(|process| process.is_running().unwrap_or(false));
        if !running {
            self.web_launcher = None;
        }
        if let Some(process) = self.web_launcher.as_mut() {
            if let Err(error) = process.show_settings(&snapshot) {
                tracing::error!("failed to show WebKit System Settings: {error}");
                self.web_launcher = None;
            }
            return;
        }
        match WebLauncherProcess::spawn(self.theme.appearance_is_light()) {
            Ok(mut process) => match process.show_settings(&snapshot) {
                Ok(()) => self.web_launcher = Some(process),
                Err(error) => tracing::error!("failed to show WebKit System Settings: {error}"),
            },
            Err(error) => tracing::error!("failed to start WebKit System Settings: {error}"),
        }
    }

    pub(crate) fn toggle_web_quick_settings(&mut self) {
        let running = self
            .web_quick_settings
            .as_mut()
            .is_some_and(|process| process.is_running().unwrap_or(false));
        if !running {
            self.web_quick_settings = None;
        }
        if self
            .web_quick_settings
            .as_ref()
            .is_some_and(|process| process.visible)
        {
            if let Some(process) = self.web_quick_settings.as_mut() {
                if let Err(error) = process.set_visible(false) {
                    tracing::error!("failed to control managed WebKit Quick Settings: {error}");
                    self.web_quick_settings = None;
                } else {
                    tracing::info!(
                        visible = false,
                        "managed WebKit Quick Settings visibility changed"
                    );
                }
            }
            return;
        }

        self.refresh_quick_settings_network();
        let snapshot = self.quick_settings_snapshot();
        if let Some(process) = self.web_quick_settings.as_mut() {
            match process.show_with_snapshot(&snapshot) {
                Ok(()) => tracing::info!(
                    visible = true,
                    "managed WebKit Quick Settings visibility changed"
                ),
                Err(error) => {
                    tracing::error!("failed to control managed WebKit Quick Settings: {error}");
                    self.web_quick_settings = None;
                }
            }
            return;
        }
        match WebQuickSettingsProcess::spawn(self.theme.appearance_is_light()) {
            Ok(mut process) => match process.show_with_snapshot(&snapshot) {
                Ok(()) => self.web_quick_settings = Some(process),
                Err(error) => {
                    tracing::error!("failed to show managed WebKit Quick Settings: {error}")
                }
            },
            Err(error) => tracing::error!("failed to start managed WebKit Quick Settings: {error}"),
        }
    }

    pub(crate) fn hide_web_quick_settings(&mut self) {
        let Some(process) = self.web_quick_settings.as_mut() else {
            return;
        };
        if !process.visible {
            return;
        }
        if let Err(error) = process.set_visible(false) {
            tracing::error!("failed to hide managed WebKit Quick Settings: {error}");
            self.web_quick_settings = None;
        }
    }

    pub(crate) fn refresh_web_quick_settings(&mut self) {
        let snapshot = self.quick_settings_snapshot();
        let Some(process) = self.web_quick_settings.as_mut() else {
            return;
        };
        if !process.visible || !process.is_running().unwrap_or(false) {
            return;
        }
        if let Err(error) = process.update_snapshot(&snapshot) {
            tracing::error!("failed to refresh managed WebKit Quick Settings: {error}");
            self.web_quick_settings = None;
        }
    }

    pub(crate) fn refresh_web_appearance(&mut self) {
        let snapshot = self.appearance_snapshot();
        let Some(process) = self.web_launcher.as_mut() else {
            return;
        };
        if !process.is_running().unwrap_or(false) {
            return;
        }
        if let Err(error) = process.update_appearance(&snapshot) {
            tracing::error!("failed to refresh WebKit appearance settings: {error}");
            self.web_launcher = None;
        }
    }

    pub(crate) fn sync_web_theme(&mut self) {
        let theme = if self.theme.appearance_is_light() {
            AppearanceTheme::Light
        } else {
            AppearanceTheme::Dark
        };
        if let Some(process) = self.web_launcher.as_mut() {
            if let Err(error) = process.set_theme(theme) {
                tracing::error!("failed to update WebKit launcher theme: {error}");
                self.web_launcher = None;
            }
        }
        if let Some(process) = self.web_quick_settings.as_mut() {
            if let Err(error) = process.set_theme(theme) {
                tracing::error!("failed to update WebKit Quick Settings theme: {error}");
                self.web_quick_settings = None;
            }
        }
        if self.web_panel_enabled {
            self.web_panel_theme_refresh = Some(theme == AppearanceTheme::Light);
        }
    }

    pub(crate) fn refresh_quick_settings_network(&mut self) {
        self.network_controller.poll();
        self.network_profiles = crate::network::list_saved_connections();
        self.wifi_networks = crate::network::scan_wifi_networks();
    }

    fn quick_settings_snapshot(&self) -> QuickSettingsSnapshot {
        use crate::{
            battery::ChargeState,
            network::{ConnectionKind, NetworkState},
        };

        let wifi_networks = self
            .wifi_networks
            .iter()
            .take(64)
            .map(|network| QuickSettingsWifiNetwork {
                ssid: network.ssid.clone(),
                signal_percent: network.signal.min(100),
                secured: network.secured,
                known: self
                    .network_profiles
                    .iter()
                    .any(|profile| profile.name == network.ssid),
                in_use: network.in_use,
            })
            .collect();
        let network = match self.network_controller.state() {
            NetworkState::Offline => QuickSettingsNetwork {
                available: false,
                connected: false,
                kind: None,
                name: None,
                signal_percent: None,
                wifi_networks,
            },
            NetworkState::Disconnected => QuickSettingsNetwork {
                available: true,
                connected: false,
                kind: None,
                name: None,
                signal_percent: None,
                wifi_networks,
            },
            NetworkState::Connected {
                kind,
                connection_name,
            } => {
                let (kind, signal_percent) = match kind {
                    ConnectionKind::Ethernet => ("Ethernet", None),
                    ConnectionKind::Wifi { signal } => ("WLAN", *signal),
                    ConnectionKind::Vpn => ("VPN", None),
                    ConnectionKind::Other => ("Netzwerk", None),
                };
                QuickSettingsNetwork {
                    available: true,
                    connected: true,
                    kind: Some(kind.to_string()),
                    name: Some(connection_name.clone()),
                    signal_percent,
                    wifi_networks,
                }
            }
        };
        let audio = self.audio_snapshot.default_output.as_ref();
        QuickSettingsSnapshot {
            network,
            audio: QuickSettingsAudio {
                available: audio.is_some(),
                output_name: audio.map(|device| device.name.clone()),
                volume_percent: audio.and_then(|device| device.volume_percent),
                muted: audio.is_some_and(|device| device.muted),
            },
            battery: QuickSettingsBattery {
                present: self.battery_snapshot.present,
                capacity: self.battery_snapshot.capacity,
                charging: matches!(self.battery_snapshot.state, ChargeState::Charging),
                on_ac: self.battery_snapshot.on_ac,
            },
            power_profile: self
                .power_profile
                .map(|profile| profile.label().to_string()),
        }
    }

    fn appearance_snapshot(&self) -> AppearanceSnapshot {
        let theme = if self.theme.appearance_is_light() {
            AppearanceTheme::Light
        } else {
            AppearanceTheme::Dark
        };
        let wallpaper_path = self.wallpaper_path.as_deref().or_else(|| {
            self.theme
                .wallpaper
                .as_ref()
                .map(|wallpaper| wallpaper.path.as_str())
        });
        let wallpaper_name = wallpaper_path.and_then(|path| {
            std::path::Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        });
        let wallpaper_mode = match self.wallpaper_mode {
            meridian_config::WallpaperMode::Fill => AppearanceWallpaperMode::Fill,
            meridian_config::WallpaperMode::Fit => AppearanceWallpaperMode::Fit,
            meridian_config::WallpaperMode::Center => AppearanceWallpaperMode::Center,
            meridian_config::WallpaperMode::Tile => AppearanceWallpaperMode::Tile,
        };
        AppearanceSnapshot {
            theme,
            wallpaper_name,
            wallpaper_mode,
        }
    }
}

fn canonical_appearance_json(value: &AppearanceSnapshot) -> io::Result<String> {
    serde_json::to_string(value).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
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
