use std::{env, path::Path, process::Command};

use meridian_config::MeridianConfig;
use meridian_ipc::{ShellCommand, ShellEvent};
use smithay::utils::SERIAL_COUNTER;
use smithay::wayland::seat::WaylandFocus;

use super::conversions::ipc_workspace_to_index;
use crate::{
    cursor::CursorImage,
    state::{window_list_entry, MeridianState},
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
            tracing::info!("received shell IPC command: {:?}", command);
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
            let result = super::screenshot::handle_screenshot_bridge_request(
                bridge.request,
                bridge.client_id,
            );
            tracing::info!(
                "screenshot bridge denied/unsupported: request_id={} result={:?}",
                request_id,
                result
            );
            self.ipc
                .send_screenshot_bridge_response(bridge.client_id, request_id, result);
        }
    }

    fn handle_shell_command(&mut self, command: ShellCommand) {
        match command {
            ShellCommand::SwitchWorkspace { workspace } => {
                let idx = ipc_workspace_to_index(workspace);
                self.switch_workspace(idx);
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
                if is_firefox_program(&spec.program)
                    && std::env::var_os("MOZ_ENABLE_WAYLAND").is_none()
                {
                    launch.env("MOZ_ENABLE_WAYLAND", "1");
                }

                if let Err(err) = launch.spawn() {
                    if err.kind() == std::io::ErrorKind::NotFound {
                        tracing::warn!(
                            "failed to launch app: program not found: {:?}",
                            spec.program
                        );
                    } else {
                        tracing::warn!(
                            "failed to launch app program {:?} args {:?}: {}",
                            spec.program,
                            spec.args,
                            err
                        );
                    }
                }
            }
            ShellCommand::ReloadConfig => {
                self.reload_config();
            }
            ShellCommand::Quit => {
                self.loop_signal.stop();
            }
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
        self.keybind_config = config.keybinds;

        if changes.theme_changed {
            tracing::info!(
                "theme override changed: {}",
                self.theme_manager.current().name
            );
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
}

fn is_firefox_program(program: &str) -> bool {
    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("firefox") || name.eq_ignore_ascii_case("firefox-esr")
        })
}
