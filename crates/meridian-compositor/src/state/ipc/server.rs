use std::{
    fs,
    io::{self, Read, Write},
    net::Shutdown,
    os::{
        fd::AsRawFd,
        unix::fs::{FileTypeExt, MetadataExt},
        unix::net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
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
    socket_path: Option<PathBuf>,
    socket_identity: Option<SocketIdentity>,
    auth_token: String,
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
    role: IpcClientRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IpcClientRole {
    Public,
    Shell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SocketIdentity {
    dev: u64,
    ino: u64,
}

impl IpcServer {
    pub fn new() -> Self {
        let path = meridian_ipc::socket_path();
        let auth_token = generate_ipc_auth_token();
        if path.exists() {
            if let Err(err) = fs::remove_file(&path) {
                tracing::warn!("failed to remove stale IPC socket {:?}: {}", path, err);
            }
        }

        let (listener, socket_path, socket_identity) = match UnixListener::bind(&path) {
            Ok(listener) => {
                if let Err(err) = listener.set_nonblocking(true) {
                    tracing::warn!("failed to set IPC socket nonblocking: {}", err);
                }
                let socket_identity = match socket_identity_for_path(&path) {
                    Ok(identity) => Some(identity),
                    Err(err) => {
                        tracing::warn!("failed to capture IPC socket identity {:?}: {}", path, err);
                        None
                    }
                };
                tracing::info!("Meridian IPC listening on {:?}", path);
                (Some(listener), Some(path), socket_identity)
            }
            Err(err) => {
                tracing::warn!("failed to bind IPC socket {:?}: {}", path, err);
                (None, None, None)
            }
        };

        Self {
            listener,
            clients: Vec::new(),
            next_client_id: 1,
            socket_path,
            socket_identity,
            auth_token,
        }
    }

    pub fn auth_token(&self) -> &str {
        &self.auth_token
    }

    pub fn poll(&mut self) -> IpcPoll {
        let mut accepted_clients = 0;
        let mut commands = Vec::new();
        let mut screenshot_requests = Vec::new();

        if let Some(listener) = &self.listener {
            loop {
                match listener.accept() {
                    Ok((stream, _addr)) => {
                        if !is_allowed_ipc_peer(&stream) {
                            let _ = stream.shutdown(Shutdown::Both);
                            continue;
                        }
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
                            role: IpcClientRole::Public,
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
                    Ok(ShellCommand::Authenticate { role, token }) => {
                        if role == "shell" && token == self.auth_token {
                            client.role = IpcClientRole::Shell;
                            tracing::info!(
                                client_id = client.id,
                                "IPC client authenticated as shell"
                            );
                        } else {
                            tracing::warn!(
                                client_id = client.id,
                                role = %role,
                                "rejecting IPC client authentication"
                            );
                            client.alive = false;
                            break;
                        }
                    }
                    Ok(command) => {
                        if client.role == IpcClientRole::Shell {
                            commands.push(command);
                        } else {
                            tracing::warn!(
                                client_id = client.id,
                                command = ?command,
                                "ignoring unauthenticated IPC control command"
                            );
                        }
                    }
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

    pub fn broadcast(&mut self, event: &ShellEvent) -> usize {
        let Ok(bytes) = meridian_ipc::encode_event(event) else {
            return 0;
        };

        let mut sent = 0;
        for client in &mut self.clients {
            if client.role != IpcClientRole::Shell {
                continue;
            }
            if let Err(err) = client.stream.write_all(&bytes) {
                tracing::debug!("IPC client write failed: {}", err);
                client.alive = false;
            } else {
                sent += 1;
            }
        }

        self.retain_alive();
        sent
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

impl Drop for IpcServer {
    fn drop(&mut self) {
        for client in &mut self.clients {
            let _ = client.stream.shutdown(Shutdown::Both);
        }
        self.clients.clear();

        // Ensure listener fd is dropped before unlinking the socket path.
        self.listener.take();

        let (Some(path), Some(expected)) = (self.socket_path.as_deref(), self.socket_identity)
        else {
            return;
        };
        match should_cleanup_socket_path(path, expected) {
            Ok(true) => {
                if let Err(err) = fs::remove_file(path) {
                    tracing::warn!(
                        "failed to remove IPC socket on shutdown {:?}: {}",
                        path,
                        err
                    );
                }
            }
            Ok(false) => {
                tracing::debug!(
                    "skipping IPC socket cleanup: path no longer points to this server socket: {:?}",
                    path
                );
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => {
                tracing::warn!(
                    "failed to validate IPC socket before cleanup {:?}: {}",
                    path,
                    err
                );
            }
        }
    }
}

fn is_allowed_ipc_peer(stream: &UnixStream) -> bool {
    let effective_uid = current_effective_uid();
    match peer_effective_uid(stream) {
        Ok(Some(peer_uid)) if is_same_uid(peer_uid, effective_uid) => true,
        Ok(Some(peer_uid)) => {
            tracing::warn!("rejecting IPC client from different uid: {}", peer_uid);
            false
        }
        Ok(None) => {
            tracing::warn!(
                "IPC peer credential check unsupported on this platform; allowing client"
            );
            true
        }
        Err(err) => {
            tracing::warn!(
                "rejecting IPC client: failed to read peer credentials: {}",
                err
            );
            false
        }
    }
}

fn current_effective_uid() -> u32 {
    // SAFETY: `geteuid` has no preconditions and returns the caller's effective uid.
    unsafe { libc::geteuid() as u32 }
}

fn is_same_uid(peer_uid: u32, effective_uid: u32) -> bool {
    peer_uid == effective_uid
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn peer_effective_uid(stream: &UnixStream) -> io::Result<Option<u32>> {
    // SAFETY: zero-initializing `ucred` is valid; kernel fully writes fields on successful `getsockopt`.
    let mut creds: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: pointers target valid writable storage; fd/socket level/option are correct for SO_PEERCRED.
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut creds as *mut libc::ucred).cast::<libc::c_void>(),
            &mut len,
        )
    };
    if rc == -1 {
        return Err(io::Error::last_os_error());
    }
    if len < std::mem::size_of::<libc::ucred>() as libc::socklen_t {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "short SO_PEERCRED payload",
        ));
    }
    Ok(Some(creds.uid))
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn peer_effective_uid(_stream: &UnixStream) -> io::Result<Option<u32>> {
    Ok(None)
}

fn socket_identity_for_path(path: &Path) -> io::Result<SocketIdentity> {
    let metadata = fs::metadata(path)?;
    Ok(SocketIdentity {
        dev: metadata.dev(),
        ino: metadata.ino(),
    })
}

fn should_cleanup_socket_path(path: &Path, expected: SocketIdentity) -> io::Result<bool> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_socket() {
        return Ok(false);
    }
    Ok(SocketIdentity {
        dev: metadata.dev(),
        ino: metadata.ino(),
    } == expected)
}

fn generate_ipc_auth_token() -> String {
    let mut bytes = [0_u8; 32];
    match fs::File::open("/dev/urandom").and_then(|mut file| file.read_exact(&mut bytes)) {
        Ok(()) => bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        Err(err) => {
            tracing::warn!(error = %err, "failed to read IPC auth token from /dev/urandom; using process-local fallback");
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0);
            format!("fallback-{}-{nanos}", std::process::id())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{Read, Write},
        os::unix::net::{UnixListener, UnixStream},
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    use meridian_ipc::{ShellCommand, ShellEvent};

    use super::{is_same_uid, should_cleanup_socket_path, socket_identity_for_path, IpcServer};

    #[test]
    fn same_uid_is_allowed() {
        assert!(is_same_uid(1000, 1000));
    }

    #[test]
    fn different_uid_is_rejected() {
        assert!(!is_same_uid(1000, 1001));
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn temp_runtime_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "meridian-ipc-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create runtime dir");
        dir
    }

    fn with_runtime_dir<R>(dir: &std::path::Path, f: impl FnOnce() -> R) -> R {
        let _guard = env_lock().lock().expect("env lock");
        let previous = std::env::var_os("XDG_RUNTIME_DIR");
        std::env::set_var("XDG_RUNTIME_DIR", dir);
        let result = f();
        match previous {
            Some(value) => std::env::set_var("XDG_RUNTIME_DIR", value),
            None => std::env::remove_var("XDG_RUNTIME_DIR"),
        }
        result
    }

    fn connect_client() -> UnixStream {
        UnixStream::connect(meridian_ipc::socket_path()).expect("connect ipc client")
    }

    fn write_command(stream: &mut UnixStream, command: &ShellCommand) {
        let bytes = meridian_ipc::encode_command(command).expect("encode command");
        stream.write_all(&bytes).expect("write command");
    }

    #[test]
    fn unauthenticated_control_command_is_ignored() {
        let dir = temp_runtime_dir("unauth-command");
        with_runtime_dir(&dir, || {
            let mut server = IpcServer::new();
            let mut client = connect_client();
            write_command(&mut client, &ShellCommand::Quit);

            let poll = server.poll();
            assert_eq!(poll.accepted_clients, 1);
            assert!(poll.commands.is_empty());
        });
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn authenticated_shell_control_command_is_accepted() {
        let dir = temp_runtime_dir("auth-command");
        with_runtime_dir(&dir, || {
            let mut server = IpcServer::new();
            let token = server.auth_token().to_string();
            let mut client = connect_client();
            write_command(
                &mut client,
                &ShellCommand::Authenticate {
                    role: "shell".to_string(),
                    token,
                },
            );
            write_command(&mut client, &ShellCommand::Quit);

            let poll = server.poll();
            assert_eq!(poll.commands, vec![ShellCommand::Quit]);
        });
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn broadcasts_only_reach_authenticated_shell_clients() {
        let dir = temp_runtime_dir("broadcast-auth");
        with_runtime_dir(&dir, || {
            let mut server = IpcServer::new();
            let mut public_client = connect_client();
            let mut shell_client = connect_client();
            write_command(
                &mut shell_client,
                &ShellCommand::Authenticate {
                    role: "shell".to_string(),
                    token: server.auth_token().to_string(),
                },
            );
            server.poll();

            public_client
                .set_nonblocking(true)
                .expect("set public nonblocking");
            shell_client
                .set_nonblocking(true)
                .expect("set shell nonblocking");
            let sent = server.broadcast(&ShellEvent::ConfigReloaded { success: true });
            assert_eq!(sent, 1);

            let mut shell_buf = [0_u8; 512];
            let n = shell_client.read(&mut shell_buf).expect("read shell event");
            let line = std::str::from_utf8(&shell_buf[..n]).expect("utf8 event");
            assert!(matches!(
                meridian_ipc::decode_event(line).expect("decode event"),
                ShellEvent::ConfigReloaded { success: true }
            ));

            let mut public_buf = [0_u8; 64];
            let err = public_client
                .read(&mut public_buf)
                .expect_err("public client must not receive broadcast");
            assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);
        });
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_check_matches_original_socket_identity() {
        let dir = std::env::temp_dir().join(format!(
            "meridian-ipc-cleanup-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        let socket_path = dir.join("meridian.sock");

        let listener = UnixListener::bind(&socket_path).expect("bind test socket");
        let identity = socket_identity_for_path(&socket_path).expect("capture socket identity");

        assert!(
            should_cleanup_socket_path(&socket_path, identity).expect("validate cleanup target")
        );

        drop(listener);
        let _ = fs::remove_file(&socket_path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn cleanup_check_rejects_replaced_non_socket_path() {
        let dir = std::env::temp_dir().join(format!(
            "meridian-ipc-cleanup-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        let socket_path = dir.join("meridian.sock");

        let listener = UnixListener::bind(&socket_path).expect("bind test socket");
        let identity = socket_identity_for_path(&socket_path).expect("capture socket identity");
        drop(listener);
        fs::remove_file(&socket_path).expect("remove socket");

        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&socket_path)
            .expect("create replacement file");
        writeln!(file, "replacement").expect("write replacement file");
        file.flush().expect("flush replacement file");

        assert!(
            !should_cleanup_socket_path(&socket_path, identity).expect("validate replacement path")
        );

        let _ = fs::remove_file(&socket_path);
        let _ = fs::remove_dir(&dir);
    }
}
