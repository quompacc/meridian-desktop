//! Narrow compositor IPC adapter for launcher activation.
//!
//! The document supplies only a desktop-entry identity. This module resolves
//! the already validated Rust snapshot and sends its stored argv to Meridian's
//! authenticated control socket; arbitrary process spawning is never exposed.

use std::{io::Write, os::unix::net::UnixStream};

use meridian_app_catalog::DesktopApp;
use meridian_ipc::ShellCommand;

pub(crate) fn launch(app: &DesktopApp) -> Result<(), String> {
    with_authenticated_stream(|stream| {
        write(
            stream,
            &ShellCommand::LaunchApp {
                program: app.program.clone(),
                args: app.args.clone(),
                terminal: app.terminal,
            },
        )
    })
}

pub(crate) fn toggle_launcher() -> Result<(), String> {
    with_authenticated_stream(|stream| write(stream, &ShellCommand::ToggleLauncher))
}

fn with_authenticated_stream(
    operation: impl FnOnce(&mut UnixStream) -> Result<(), String>,
) -> Result<(), String> {
    let token = std::env::var(meridian_ipc::IPC_TOKEN_ENV)
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "IPC authentication token is unavailable".to_string())?;
    let mut stream = UnixStream::connect(meridian_ipc::socket_path())
        .map_err(|error| format!("cannot connect to compositor IPC: {error}"))?;
    write(
        &mut stream,
        &ShellCommand::Authenticate {
            role: "shell".to_string(),
            token,
        },
    )?;
    operation(&mut stream)
}

fn write(stream: &mut UnixStream, command: &ShellCommand) -> Result<(), String> {
    let bytes = meridian_ipc::encode_command(command)
        .map_err(|error| format!("cannot encode IPC command: {error}"))?;
    stream
        .write_all(&bytes)
        .map_err(|error| format!("cannot write IPC command: {error}"))
}
