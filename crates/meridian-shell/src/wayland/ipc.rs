use std::{
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use meridian_ipc::{ShellCommand, ShellEvent};
use tracing::{debug, warn};

const IPC_MAX_BUFFER_BYTES: usize = 64 * 1024;

pub struct IpcClient {
    stream: Option<UnixStream>,
    buffer: Vec<u8>,
    last_attempt: Instant,
}

impl IpcClient {
    pub(crate) fn connect() -> Self {
        let mut client = Self {
            stream: None,
            buffer: Vec::new(),
            last_attempt: Instant::now() - Duration::from_secs(5),
        };
        client.reconnect();
        client
    }

    pub(crate) fn should_reconnect(&self) -> bool {
        self.stream.is_none() && self.last_attempt.elapsed() >= Duration::from_secs(2)
    }

    pub(crate) fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    pub(crate) fn reconnect(&mut self) {
        self.last_attempt = Instant::now();
        match UnixStream::connect(meridian_ipc::socket_path()) {
            Ok(stream) => {
                if let Err(err) = stream.set_nonblocking(true) {
                    warn!("failed to set meridian IPC nonblocking: {}", err);
                }
                self.stream = Some(stream);
            }
            Err(err) => {
                debug!("meridian IPC unavailable: {}", err);
            }
        }
    }

    pub(crate) fn poll(&mut self) -> Vec<ShellEvent> {
        let mut out = Vec::new();
        let Some(stream) = self.stream.as_mut() else {
            return out;
        };

        let mut tmp = [0_u8; 4096];
        let mut oversized_incomplete_line = false;
        loop {
            match stream.read(&mut tmp) {
                Ok(0) => {
                    self.disconnect();
                    break;
                }
                Ok(n) => {
                    self.buffer.extend_from_slice(&tmp[..n]);
                    if self.buffer.len() > IPC_MAX_BUFFER_BYTES {
                        warn!(
                            "meridian IPC read buffer exceeded limit ({} bytes), reconnecting",
                            IPC_MAX_BUFFER_BYTES
                        );
                        self.buffer.clear();
                        self.disconnect();
                        oversized_incomplete_line = true;
                        break;
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                Err(err) => {
                    warn!("meridian IPC read failed: {}", err);
                    self.disconnect();
                    break;
                }
            }
        }

        if oversized_incomplete_line {
            return out;
        }

        while let Some(pos) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let line = self.buffer.drain(..=pos).collect::<Vec<_>>();
            let line = String::from_utf8_lossy(&line);
            if let Some(event) = parse_event_line(line.trim()) {
                out.push(event);
            }
        }

        out
    }

    pub fn send(&mut self, command: &ShellCommand) -> bool {
        if self.stream.is_none() {
            self.reconnect();
        }

        let Some(stream) = self.stream.as_mut() else {
            return false;
        };

        let Ok(bytes) = meridian_ipc::encode_command(command) else {
            return false;
        };

        match stream.write_all(&bytes) {
            Ok(()) => true,
            Err(err) => {
                warn!("meridian IPC write failed: {}", err);
                self.disconnect();
                false
            }
        }
    }

    fn disconnect(&mut self) {
        if let Some(stream) = self.stream.take() {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

fn parse_event_line(line: &str) -> Option<ShellEvent> {
    if line.is_empty() {
        return None;
    }
    if let Ok(event) = meridian_ipc::decode_event(line) {
        return Some(event);
    }

    let mut parts = line.splitn(3, ' ');
    match parts.next()? {
        "workspace-changed" => parts
            .next()
            .and_then(|workspace| workspace.parse().ok())
            .map(|workspace| ShellEvent::WorkspaceChanged { workspace }),
        "window-opened" => {
            let id = parts.next()?.to_string();
            let title = parts.next().unwrap_or("").to_string();
            Some(ShellEvent::WindowOpened { id, title })
        }
        "window-closed" => Some(ShellEvent::WindowClosed {
            id: parts.next()?.to_string(),
        }),
        "window-focused" => Some(ShellEvent::WindowFocused {
            id: parts.next()?.to_string(),
        }),
        "config-reloaded" => parts
            .next()
            .and_then(|value| value.parse().ok())
            .map(|success| ShellEvent::ConfigReloaded { success }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::Write,
        os::unix::net::UnixStream,
        time::{Duration, Instant},
    };

    use super::{IpcClient, IPC_MAX_BUFFER_BYTES};

    #[test]
    fn oversized_incomplete_line_disconnects_and_clears_buffer() {
        let (mut writer, reader) = UnixStream::pair().expect("stream pair");
        reader.set_nonblocking(true).expect("set nonblocking");

        let mut client = IpcClient {
            stream: Some(reader),
            buffer: Vec::new(),
            last_attempt: Instant::now() - Duration::from_secs(5),
        };

        writer
            .write_all(&vec![b'x'; IPC_MAX_BUFFER_BYTES + 1])
            .expect("write oversized payload");

        let events = client.poll();
        assert!(events.is_empty());
        assert!(!client.is_connected());
        assert!(client.buffer.is_empty());
    }
}
