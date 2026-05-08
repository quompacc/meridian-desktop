use std::{env, io, path::PathBuf};

use serde::{Deserialize, Serialize};

pub const SOCKET_NAME: &str = "meridian.sock";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ShellEvent {
    WorkspaceChanged { workspace: u8 },
    WindowOpened { id: String, title: String },
    WindowClosed { id: String },
    WindowFocused { id: String },
    ToggleLauncher,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ShellCommand {
    SwitchWorkspace {
        workspace: u8,
    },
    FocusWindow {
        id: String,
    },
    LaunchApp {
        command: String,
        #[serde(default)]
        terminal: bool,
    },
    ReloadConfig,
}

pub fn socket_path() -> PathBuf {
    if let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join(SOCKET_NAME);
    }

    let uid = unsafe { libc::geteuid() };
    PathBuf::from(format!("/run/user/{uid}")).join(SOCKET_NAME)
}

pub fn encode_command(command: &ShellCommand) -> io::Result<Vec<u8>> {
    encode_json_line(command)
}

pub fn encode_event(event: &ShellEvent) -> io::Result<Vec<u8>> {
    encode_json_line(event)
}

pub fn decode_command(line: &str) -> serde_json::Result<ShellCommand> {
    serde_json::from_str(line.trim())
}

pub fn decode_event(line: &str) -> serde_json::Result<ShellEvent> {
    serde_json::from_str(line.trim())
}

fn encode_json_line<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
    let mut bytes =
        serde_json::to_vec(value).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    bytes.push(b'\n');
    Ok(bytes)
}
