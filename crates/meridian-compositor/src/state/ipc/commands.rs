use std::{env, process::Command};

use meridian_config::MeridianConfig;
use meridian_ipc::{ScreenshotBridgeError, ScreenshotBridgeResult, ShellCommand, ShellEvent};
use smithay::utils::SERIAL_COUNTER;
use smithay::wayland::seat::WaylandFocus;

use super::conversions::ipc_workspace_to_index;
use crate::{
    cursor::CursorImage,
    state::{window_list_entry, MeridianState, OutputLayout},
};

impl MeridianState {
    pub fn poll_ipc(&mut self) {
        let poll = self.ipc.poll();

        if poll.accepted_clients > 0 {
            tracing::info!("accepted {} shell IPC client(s)", poll.accepted_clients);
            self.broadcast_workspace();
            self.broadcast_window_snapshot();
        }

        for command in poll.commands {
            tracing::info!(command = command.name(), "received shell IPC command");
            self.handle_shell_command(command);
        }

        for bridge in poll.screenshot_requests {
            tracing::info!(
                "compositor screenshot bridge request received: request_id={} output={:?} include_cursor={}",
                bridge.request.request_id,
                bridge.request.output,
                bridge.request.include_cursor
            );
            let request_id = bridge.request.request_id.clone();
            let client_id = bridge.client_id;
            match super::screenshot::handle_screenshot_bridge_request(bridge.request, client_id) {
                super::screenshot::ScreenshotBridgeOutcome::Queue(request) => {
                    // Allowed: the render loop captures and responds once the
                    // PNG is written. Hold the client_id so the response can be
                    // routed back to the right requester.
                    tracing::info!(
                        "screenshot bridge allowed, queued: request_id={}",
                        request_id
                    );
                    self.pending_screenshot_requests
                        .push(crate::state::PendingScreenshotRequest { client_id, request });
                }
                super::screenshot::ScreenshotBridgeOutcome::AwaitConsent(request) => {
                    // Hold the request and ask the authenticated shell to show a
                    // consent modal. The answer returns as a shell command.
                    let app_id = request.metadata.requester.clone().unwrap_or_default();
                    tracing::info!(
                        "screenshot bridge needs consent: request_id={} app_id={:?}",
                        request_id,
                        app_id
                    );
                    self.pending_screenshot_consent
                        .push(crate::state::PendingScreenshotRequest { client_id, request });
                    let recipients = self
                        .ipc
                        .broadcast(&ShellEvent::ScreenshotConsentRequest { request_id, app_id });
                    if recipients == 0 {
                        self.reject_pending_screenshot_consent_without_shell();
                    }
                }
                super::screenshot::ScreenshotBridgeOutcome::AwaitRegionPick(request) => {
                    // Hold the request and ask the authenticated shell to show
                    // the region picker. The answer returns as a shell command.
                    let app_id = request.metadata.requester.clone().unwrap_or_default();
                    tracing::info!(
                        "screenshot bridge needs region pick: request_id={} app_id={:?}",
                        request_id,
                        app_id
                    );
                    self.pending_screenshot_region
                        .push(crate::state::PendingScreenshotRequest { client_id, request });
                    let recipients = self
                        .ipc
                        .broadcast(&ShellEvent::ScreenshotRegionRequest { request_id, app_id });
                    if recipients == 0 {
                        self.reject_pending_screenshot_region_without_shell();
                    }
                }
                super::screenshot::ScreenshotBridgeOutcome::Respond(result) => {
                    tracing::info!(
                        "screenshot bridge rejected: request_id={} result={:?}",
                        request_id,
                        result
                    );
                    self.ipc
                        .send_screenshot_bridge_response(client_id, request_id, result);
                }
            }
        }
    }

    fn handle_shell_command(&mut self, command: ShellCommand) {
        match command {
            ShellCommand::Authenticate { .. } => {
                tracing::debug!("ignoring IPC authentication command after server-side handling");
            }
            ShellCommand::SwitchWorkspace { workspace } => {
                let idx = ipc_workspace_to_index(workspace);
                self.switch_workspace(idx);
            }
            ShellCommand::ToggleLauncher => {
                self.ipc.broadcast(&ShellEvent::ToggleLauncher);
            }
            ShellCommand::ToggleQuickSettings => {
                self.ipc.broadcast(&ShellEvent::ToggleQuickSettings);
            }
            ShellCommand::OpenSystemSettings => {
                self.ipc.broadcast(&ShellEvent::OpenSystemSettings);
            }
            ShellCommand::AppearanceRefresh => {
                self.ipc.broadcast(&ShellEvent::AppearanceRefresh);
            }
            ShellCommand::AppearanceThemeSet { theme } => {
                self.ipc
                    .broadcast(&ShellEvent::AppearanceThemeSet { theme });
            }
            ShellCommand::AppearanceWallpaperSet { path } => {
                if super::appearance::valid_wallpaper_path(&path) {
                    self.ipc
                        .broadcast(&ShellEvent::AppearanceWallpaperSet { path });
                } else {
                    tracing::warn!("rejected invalid appearance wallpaper path");
                }
            }
            ShellCommand::AppearanceWallpaperModeSet { mode } => {
                self.ipc
                    .broadcast(&ShellEvent::AppearanceWallpaperModeSet { mode });
            }
            ShellCommand::QuickSettingsNetworkRefresh => {
                self.ipc.broadcast(&ShellEvent::QuickSettingsNetworkRefresh);
            }
            ShellCommand::QuickSettingsNetworkConnect { ssid, password } => {
                if super::network::valid_connect_request(&ssid, password.as_deref()) {
                    self.ipc
                        .broadcast(&ShellEvent::QuickSettingsNetworkConnect { ssid, password });
                } else {
                    tracing::warn!("rejected invalid Quick Settings network request");
                }
            }
            ShellCommand::QuickSettingsNetworkDisconnect => {
                self.ipc
                    .broadcast(&ShellEvent::QuickSettingsNetworkDisconnect);
            }
            ShellCommand::AudioVolumeSet { percent } => {
                if percent <= 100 {
                    self.ipc.broadcast(&ShellEvent::AudioVolumeSet { percent });
                } else {
                    tracing::warn!(percent, "rejected out-of-range Quick Settings volume");
                }
            }
            ShellCommand::AudioMuteToggle => {
                self.ipc
                    .broadcast(&ShellEvent::QuickSettingsAudioMuteToggle);
            }
            ShellCommand::PowerProfileSet { profile } => {
                self.ipc.broadcast(&ShellEvent::PowerProfileSet { profile });
            }
            ShellCommand::FocusWindow { id } => {
                self.focus_window_by_id(&id);
            }
            ShellCommand::LaunchApp {
                program,
                args,
                terminal,
            } => {
                let Some(spec) = super::launch::prepare_launch(&program, &args, terminal) else {
                    tracing::warn!(
                        "cannot launch app {:?} with args {:?}: invalid command or no terminal emulator found",
                        program,
                        args
                    );
                    return;
                };

                tracing::info!(
                    "launching app from shell: program={:?} args={:?}",
                    spec.program,
                    spec.args
                );
                let xdg_runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
                    // SAFETY: `geteuid` has no preconditions and reads the current process uid.
                    format!("/run/user/{}", unsafe { libc::geteuid() })
                });

                let mut launch = Command::new(&spec.program);
                launch
                    .args(&spec.args)
                    .env(
                        "WAYLAND_DISPLAY",
                        self.socket_name.to_string_lossy().as_ref(),
                    )
                    .env("XDG_RUNTIME_DIR", xdg_runtime_dir)
                    .env("XDG_SESSION_TYPE", "wayland")
                    .env("XDG_CURRENT_DESKTOP", "Meridian")
                    .env("XDG_SESSION_DESKTOP", "meridian")
                    .env("DESKTOP_SESSION", "meridian");
                super::launch::apply_launch_environment(&mut launch, &spec.program);
                if super::launch::is_firefox_program(&spec.program)
                    && std::env::var_os("MOZ_ENABLE_WAYLAND").is_none()
                {
                    launch.env("MOZ_ENABLE_WAYLAND", "1");
                }

                super::launch::spawn_and_reap(launch, &spec.program, &spec.args);
            }
            ShellCommand::ReloadConfig => {
                self.reload_config();
            }
            ShellCommand::Quit => {
                self.loop_signal.stop();
            }
            ShellCommand::CaptureWindowThumbnail {
                id,
                max_width,
                max_height,
            } => {
                use crate::state::ThumbnailRequest;
                self.pending_thumbnail_requests.push(ThumbnailRequest {
                    window_id: id,
                    max_width: if max_width == 0 { 200 } else { max_width },
                    max_height: if max_height == 0 { 112 } else { max_height },
                });
                // Mark all outputs dirty so the render loop picks up the request
                // on the next frame (same pattern as screencopy frame handler).
                if let Some(ref mut drm) = self.drm_backend {
                    for out in drm.outputs.iter_mut() {
                        out.needs_repaint = true;
                    }
                }
            }
            ShellCommand::ScreenshotConsentResponse {
                request_id,
                allowed,
            } => {
                self.resolve_screenshot_consent(&request_id, allowed);
            }
            ShellCommand::ScreenshotRegionResponse { request_id, region } => {
                self.resolve_screenshot_region(&request_id, region);
            }
        }
    }

    fn reject_pending_screenshot_consent_without_shell(&mut self) {
        let Some(pending) = self.pending_screenshot_consent.pop() else {
            return;
        };
        tracing::warn!(
            request_id = %pending.request.request_id,
            "screenshot consent request rejected: no authenticated shell IPC client"
        );
        self.ipc.send_screenshot_bridge_response(
            pending.client_id,
            pending.request.request_id,
            ScreenshotBridgeResult::Error {
                error: ScreenshotBridgeError::CompositorUnavailable(
                    "shell IPC client unavailable".to_string(),
                ),
            },
        );
    }

    fn reject_pending_screenshot_region_without_shell(&mut self) {
        let Some(pending) = self.pending_screenshot_region.pop() else {
            return;
        };
        tracing::warn!(
            request_id = %pending.request.request_id,
            "screenshot region request rejected: no authenticated shell IPC client"
        );
        self.ipc.send_screenshot_bridge_response(
            pending.client_id,
            pending.request.request_id,
            ScreenshotBridgeResult::Error {
                error: ScreenshotBridgeError::CompositorUnavailable(
                    "shell IPC client unavailable".to_string(),
                ),
            },
        );
    }

    /// Apply the user's region pick to a held screenshot request: `Some(region)`
    /// stamps the region onto the request and moves it to the capture queue;
    /// `None` means the user cancelled (Esc) and we reply permission-denied.
    /// Unknown ids are logged but otherwise ignored.
    fn resolve_screenshot_region(
        &mut self,
        request_id: &str,
        region: Option<meridian_ipc::ScreenshotRegion>,
    ) {
        let Some(pos) = self
            .pending_screenshot_region
            .iter()
            .position(|p| p.request.request_id == request_id)
        else {
            tracing::warn!(
                "screenshot region response for unknown request_id={}",
                request_id
            );
            return;
        };
        let mut pending = self.pending_screenshot_region.remove(pos);
        match region {
            Some(region) => {
                tracing::info!(
                    "screenshot region picked: request_id={} region={:?}",
                    request_id,
                    region
                );
                pending.request.region = Some(region);
                self.pending_screenshot_requests.push(pending);
                if let Some(ref mut drm) = self.drm_backend {
                    for out in drm.outputs.iter_mut() {
                        out.needs_repaint = true;
                    }
                }
            }
            None => {
                tracing::info!(
                    "screenshot region pick cancelled: request_id={}",
                    request_id
                );
                self.ipc.send_screenshot_bridge_response(
                    pending.client_id,
                    pending.request.request_id,
                    super::screenshot::permission_denied_result(),
                );
            }
        }
    }

    /// Apply the user's consent answer to a held screenshot request: on allow,
    /// move it to the capture queue and nudge the render loop; on deny (or an
    /// unknown id) respond with permission-denied. Unknown ids are ignored.
    fn resolve_screenshot_consent(&mut self, request_id: &str, allowed: bool) {
        let Some(pos) = self
            .pending_screenshot_consent
            .iter()
            .position(|p| p.request.request_id == request_id)
        else {
            tracing::warn!(
                "screenshot consent response for unknown request_id={}",
                request_id
            );
            return;
        };
        let pending = self.pending_screenshot_consent.remove(pos);
        if allowed {
            tracing::info!("screenshot consent granted: request_id={}", request_id);
            self.pending_screenshot_requests.push(pending);
            if let Some(ref mut drm) = self.drm_backend {
                for out in drm.outputs.iter_mut() {
                    out.needs_repaint = true;
                }
            }
        } else {
            tracing::info!("screenshot consent denied: request_id={}", request_id);
            self.ipc.send_screenshot_bridge_response(
                pending.client_id,
                pending.request.request_id,
                super::screenshot::permission_denied_result(),
            );
        }
    }

    pub fn reload_config(&mut self) {
        tracing::info!("config reload requested");
        let mut config = MeridianConfig::default();
        if let Err(err) = config.reload() {
            tracing::warn!("config reload failed; keeping previous config: {}", err);
            self.ipc
                .broadcast(&ShellEvent::ConfigReloaded { success: false });
            return;
        }

        let changes = super::super::setup::apply_config_overrides(&mut self.theme_manager, &config);
        let previous_outputs = std::mem::take(&mut self.output_config_entries);
        self.output_config_entries = config.outputs.clone();
        self.output_layout = OutputLayout::from_config_entries(&self.output_config_entries);
        self.reapply_output_layout(&previous_outputs);
        self.keybind_config = config.keybinds;
        self.idle_timeout = config
            .general
            .idle_timeout_secs
            .map(std::time::Duration::from_secs);

        if changes.theme_changed {
            tracing::info!(
                "theme override changed: {}",
                self.theme_manager.current().name
            );
            let _ = meridian_boot_common::write_appearance(super::super::setup::theme_appearance(
                self.theme_manager.current(),
            ));
        }
        if changes.wallpaper_changed {
            tracing::info!("wallpaper override changed");
        }
        if changes.theme_changed || changes.wallpaper_changed {
            self.wallpaper_manager
                .apply_theme(self.theme_manager.current());
            self.workspaces.active_space_mut().refresh();
        }

        if changes.cursor_changed {
            tracing::info!("cursor override changed");
            self.reload_cursor_runtime();
        }

        // Re-render every output so theme/decoration changes (incl. live
        // glass tuning) show up immediately instead of waiting for damage.
        if let Some(ref mut drm) = self.drm_backend {
            for out in drm.outputs.iter_mut() {
                out.needs_repaint = true;
            }
        }

        tracing::info!("config reload succeeded");
        self.ipc
            .broadcast(&ShellEvent::ConfigReloaded { success: true });
    }

    fn reload_cursor_runtime(&mut self) {
        let cursor_config = &self.theme_manager.current().config.cursor;
        if let Some(drm) = &mut self.drm_backend {
            if !cursor_config.theme.is_empty() {
                env::set_var("XCURSOR_THEME", &cursor_config.theme);
            }
            env::set_var("XCURSOR_SIZE", cursor_config.size.to_string());

            let cursor_theme =
                env::var("XCURSOR_THEME").unwrap_or_else(|_| cursor_config.theme.clone());
            let cursor_size = env::var("XCURSOR_SIZE")
                .ok()
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(cursor_config.size);
            tracing::debug!(
                "cursor theme loaded: theme={} size={}",
                cursor_theme,
                cursor_size
            );
            let cursor_image = CursorImage::load_theme(&cursor_theme, cursor_size);
            tracing::debug!(
                "cursor hotspot: {},{}",
                cursor_image.xhot,
                cursor_image.yhot
            );
            drm.cursor_buffer = cursor_image.to_memory_buffer();
            drm.cursor_image = cursor_image;
            drm.cursor_icon = crate::backend::drm::DrmCursorIcon::Default;
            drm.compositor_cursor_cache.clear();
            drm.named_cursor_cache.clear();
        } else {
            tracing::debug!("cursor runtime reload skipped: drm backend not active");
        }
    }

    pub fn focus_window_by_id(&mut self, id: &str) {
        let idx = self.current_workspace_index();
        let mapped_window = self
            .workspaces
            .space_at(idx)
            .elements()
            .find(|window| {
                window_list_entry(window)
                    .map(|(window_id, _)| window_id == id)
                    .unwrap_or(false)
            })
            .cloned();
        if let Some(window) = mapped_window {
            self.workspaces
                .space_at_mut(idx)
                .raise_element(&window, true);

            if let Some(surface) = window.wl_surface().map(|surface| surface.into_owned()) {
                let serial = SERIAL_COUNTER.next_serial();
                self.set_keyboard_focus_with_decorations(Some(surface.clone()), serial);
                self.update_focused_output_from_surface(
                    &surface,
                    "keyboard-focus-ipc-focus-window",
                );
                self.broadcast_toplevel_focused(&surface);
            }

            self.workspaces.space_at(idx).elements().for_each(|window| {
                if let Some(toplevel) = window.toplevel() {
                    toplevel.send_pending_configure();
                }
            });
            return;
        }

        if let Some(minimized) = self.minimized_windows.remove(id) {
            if minimized.workspace != idx {
                tracing::warn!(
                    "focus-window requested minimized id on another workspace: id={} minimized_workspace={} active_workspace={}",
                    id,
                    minimized.workspace + 1,
                    idx + 1
                );
                self.minimized_windows.insert(id.to_string(), minimized);
                return;
            }

            if let Some(x11) = minimized.window.x11_surface() {
                if let Err(err) = x11.set_hidden(false) {
                    tracing::warn!(
                        "focus-window restore minimized x11 window: set_hidden(false) failed: {}",
                        err
                    );
                }
            }

            self.workspaces.space_at_mut(idx).map_element(
                minimized.window.clone(),
                minimized.restore_loc,
                true,
            );
            self.workspaces
                .space_at_mut(idx)
                .raise_element(&minimized.window, true);

            if let Some(surface) = minimized
                .window
                .wl_surface()
                .map(|surface| surface.into_owned())
            {
                let serial = SERIAL_COUNTER.next_serial();
                self.set_keyboard_focus_with_decorations(Some(surface.clone()), serial);
                self.update_focused_output_from_surface(
                    &surface,
                    "keyboard-focus-ipc-restore-minimized-window",
                );
                self.broadcast_toplevel_focused(&surface);
            }

            self.workspaces.space_at(idx).elements().for_each(|window| {
                if let Some(toplevel) = window.toplevel() {
                    toplevel.send_pending_configure();
                }
            });
            self.mark_all_outputs_dirty("ipc-restore-minimized-window");
            self.broadcast_window_snapshot();
            return;
        }

        tracing::warn!("focus-window requested unknown id: {}", id);
    }

    pub fn spawn_lock_screen(&self) {
        let display = self.socket_name.to_string_lossy().to_string();
        let xdg_runtime = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::geteuid() }));
        match std::process::Command::new("meridian-lock")
            .env("WAYLAND_DISPLAY", &display)
            .env("XDG_RUNTIME_DIR", &xdg_runtime)
            .spawn()
        {
            Ok(child) => {
                tracing::info!("spawned meridian-lock");
                reap_lock_screen_child(child);
            }
            Err(e) => tracing::warn!("failed to spawn meridian-lock: {}", e),
        }
    }
}

/// Wait on the lock-screen child so it cannot zombie on the compositor and
/// its exit status stays observable. The session lock is a security feature:
/// a crashing `meridian-lock` must not disappear silently (P1-4,
/// AUDIT_2026-08-19). Exit 0 is the normal unlock path.
fn reap_lock_screen_child(mut child: std::process::Child) {
    if let Err(err) = std::thread::Builder::new()
        .name("meridian-lock-reaper".to_string())
        .spawn(move || match child.wait() {
            Ok(status) if status.success() => {
                tracing::info!("meridian-lock exited successfully (session unlocked)");
            }
            Ok(status) => {
                tracing::warn!(
                    "meridian-lock exited with {status} — the lock screen may have crashed; session security state is uncertain"
                );
            }
            Err(err) => {
                tracing::warn!("failed to reap meridian-lock: {err}");
            }
        })
    {
        tracing::warn!("failed to spawn meridian-lock reaper thread: {}", err);
    }
}
