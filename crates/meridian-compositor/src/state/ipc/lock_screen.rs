use std::{io, process::ExitStatus};

use smithay::{
    reexports::calloop::channel::{self, Event},
    utils::SERIAL_COUNTER,
};

use crate::state::{LockClientFailureState, MeridianState};

#[cfg(target_os = "openbsd")]
const LOCK_SCREEN_PROGRAM: &str = "/usr/local/libexec/meridian-lock";
#[cfg(not(target_os = "openbsd"))]
const LOCK_SCREEN_PROGRAM: &str = "meridian-lock";

enum LockProcessExit {
    Status(ExitStatus),
    WaitFailed(io::Error),
}

impl MeridianState {
    pub fn spawn_lock_screen(&mut self) {
        let (exit_tx, exit_rx) = channel::channel();
        if let Err(err) = self
            .loop_handle
            .insert_source(exit_rx, |event, _, state| match event {
                Event::Msg(exit) => state.handle_lock_process_exit(exit),
                Event::Closed => {}
            })
        {
            tracing::warn!("failed to register meridian-lock supervisor: {err}");
            return;
        }

        let display = self.socket_name.to_string_lossy().to_string();
        let xdg_runtime = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::geteuid() }));
        match std::process::Command::new(LOCK_SCREEN_PROGRAM)
            .env("WAYLAND_DISPLAY", &display)
            .env("XDG_RUNTIME_DIR", &xdg_runtime)
            .spawn()
        {
            Ok(child) => {
                tracing::info!(program = LOCK_SCREEN_PROGRAM, "spawned meridian-lock");
                reap_lock_screen_child(child, exit_tx);
            }
            Err(err) => tracing::warn!("failed to spawn meridian-lock: {err}"),
        }
    }

    fn handle_lock_process_exit(&mut self, exit: LockProcessExit) {
        if matches!(&exit, LockProcessExit::Status(status) if status.success()) {
            tracing::info!("meridian-lock exited successfully after requesting session unlock");
            return;
        }

        let failure = self.lock_manager.handle_client_failure();
        match &exit {
            LockProcessExit::Status(status) => {
                tracing::error!(%status, ?failure, "meridian-lock exited unsuccessfully");
            }
            LockProcessExit::WaitFailed(err) => {
                tracing::error!(error = %err, ?failure, "failed to reap meridian-lock");
            }
        }

        match failure {
            LockClientFailureState::BeforeAcquisition => {
                tracing::warn!(
                    "meridian-lock failed before the compositor acquired a session lock"
                );
            }
            LockClientFailureState::PendingFailClosed
            | LockClientFailureState::LockedFailClosed => {
                let serial = SERIAL_COUNTER.next_serial();
                self.set_keyboard_focus_with_decorations(None, serial);
                self.mark_all_outputs_dirty("session-lock-client-failed");
                tracing::error!(
                    "session remains fail-closed after meridian-lock failure; console or SSH recovery is required"
                );
            }
        }
    }
}

/// Reap the child off the event-loop thread, then wake that thread so it can
/// enforce and report the compositor-owned fail-closed lock state.
fn reap_lock_screen_child(
    mut child: std::process::Child,
    exit_tx: channel::Sender<LockProcessExit>,
) {
    if let Err(err) = std::thread::Builder::new()
        .name("meridian-lock-reaper".to_string())
        .spawn(move || {
            let exit = match child.wait() {
                Ok(status) => LockProcessExit::Status(status),
                Err(err) => LockProcessExit::WaitFailed(err),
            };
            if exit_tx.send(exit).is_err() {
                tracing::error!("meridian-lock exit could not be delivered to the compositor");
            }
        })
    {
        tracing::warn!("failed to spawn meridian-lock reaper thread: {err}");
    }
}
