use std::{
    fs,
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::net::{UnixListener, UnixStream},
};

use meridian_ipc::{
    ScreenshotBridgeMessage, ScreenshotBridgeRequest, ScreenshotBridgeResult, ShellCommand,
    ShellEvent,
};

const IPC_MAX_BUFFER_BYTES: usize = 64 * 1024;

pub struct IpcServer {
    listener: Option<UnixListener>,
    clients: Vec<IpcClient>,
    next_client_id: u64,
}

pub struct IpcPoll {
    pub accepted_clients: usize,
    pub commands: Vec<ShellCommand>,
    pub screenshot_requests: Vec<ScreenshotBridgeRequestEnvelope>,
}

pub struct ScreenshotBridgeRequestEnvelope {
    pub client_id: u64,
    pub request: ScreenshotBridgeRequest,
}

struct IpcClient {
    id: u64,
    stream: UnixStream,
    buffer: Vec<u8>,
    alive: bool,
}

impl IpcServer {
    pub fn new() -> Self {
        let path = meridian_ipc::socket_path();
        if path.exists() {
            if let Err(err) = fs::remove_file(&path) {
                tracing::warn!("failed to remove stale IPC socket {:?}: {}", path, err);
            }
        }

        let listener = match UnixListener::bind(&path) {
            Ok(listener) => {
                if let Err(err) = listener.set_nonblocking(true) {
                    tracing::warn!("failed to set IPC socket nonblocking: {}", err);
                }
                tracing::info!("Meridian IPC listening on {:?}", path);
                Some(listener)
            }
            Err(err) => {
                tracing::warn!("failed to bind IPC socket {:?}: {}", path, err);
                None
            }
        };

        Self {
            listener,
            clients: Vec::new(),
            next_client_id: 1,
        }
    }

    pub fn poll(&mut self) -> IpcPoll {
        let mut accepted_clients = 0;
        let mut commands = Vec::new();
        let mut screenshot_requests = Vec::new();

        if let Some(listener) = &self.listener {
            loop {
                match listener.accept() {
                    Ok((stream, _addr)) => {
                        if let Err(err) = stream.set_nonblocking(true) {
                            tracing::warn!("failed to set IPC client nonblocking: {}", err);
                        }
                        let client_id = self.next_client_id;
                        self.next_client_id = self.next_client_id.saturating_add(1);
                        self.clients.push(IpcClient {
                            id: client_id,
                            stream,
                            buffer: Vec::new(),
                            alive: true,
                        });
                        accepted_clients += 1;
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                    Err(err) => {
                        tracing::warn!("failed to accept IPC client: {}", err);
                        break;
                    }
                }
            }
        }

        let mut tmp = [0_u8; 4096];
        for client in &mut self.clients {
            let mut oversized_incomplete_line = false;
            loop {
                match client.stream.read(&mut tmp) {
                    Ok(0) => {
                        client.alive = false;
                        break;
                    }
                    Ok(n) => {
                        client.buffer.extend_from_slice(&tmp[..n]);
                        if client.buffer.len() > IPC_MAX_BUFFER_BYTES {
                            tracing::warn!(
                                "IPC client {} exceeded read buffer limit ({} bytes), closing connection",
                                client.id,
                                IPC_MAX_BUFFER_BYTES
                            );
                            client.buffer.clear();
                            client.alive = false;
                            oversized_incomplete_line = true;
                            break;
                        }
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                    Err(err) => {
                        tracing::warn!("IPC client read failed: {}", err);
                        client.alive = false;
                        break;
                    }
                }
            }

            if oversized_incomplete_line {
                continue;
            }

            while let Some(pos) = client.buffer.iter().position(|byte| *byte == b'\n') {
                let line = client.buffer.drain(..=pos).collect::<Vec<_>>();
                let line = String::from_utf8_lossy(&line);
                let line = line.trim();
                match meridian_ipc::decode_command(line) {
                    Ok(command) => commands.push(command),
                    Err(_) => match meridian_ipc::decode_screenshot_bridge_message(line) {
                        Ok(ScreenshotBridgeMessage::ScreenshotRequest { request }) => {
                            screenshot_requests.push(ScreenshotBridgeRequestEnvelope {
                                client_id: client.id,
                                request,
                            });
                        }
                        Ok(ScreenshotBridgeMessage::ScreenshotResponse { .. }) => {
                            tracing::debug!(
                                "ignoring unexpected screenshot bridge response from client {}",
                                client.id
                            );
                        }
                        Err(err) => tracing::warn!("invalid IPC command {:?}: {}", line, err),
                    },
                }
            }
        }

        self.retain_alive();

        IpcPoll {
            accepted_clients,
            commands,
            screenshot_requests,
        }
    }

    pub fn broadcast(&mut self, event: &ShellEvent) {
        let Ok(bytes) = meridian_ipc::encode_event(event) else {
            return;
        };

        for client in &mut self.clients {
            if let Err(err) = client.stream.write_all(&bytes) {
                tracing::debug!("IPC client write failed: {}", err);
                client.alive = false;
            }
        }

        self.retain_alive();
    }

    pub fn send_screenshot_bridge_response(
        &mut self,
        client_id: u64,
        request_id: String,
        result: ScreenshotBridgeResult,
    ) {
        let message = ScreenshotBridgeMessage::ScreenshotResponse { request_id, result };
        let Ok(bytes) = meridian_ipc::encode_screenshot_bridge_message(&message) else {
            return;
        };

        let mut found = false;
        for client in &mut self.clients {
            if client.id != client_id {
                continue;
            }
            found = true;
            if let Err(err) = client.stream.write_all(&bytes) {
                tracing::debug!("IPC bridge client write failed: {}", err);
                client.alive = false;
            }
            break;
        }
        if !found {
            tracing::debug!(
                "IPC bridge response drop: client {} no longer connected",
                client_id
            );
        }
        self.retain_alive();
    }

    fn retain_alive(&mut self) {
        self.clients.retain_mut(|client| {
            if !client.alive {
                let _ = client.stream.shutdown(Shutdown::Both);
            }
            client.alive
        });
    }
}
