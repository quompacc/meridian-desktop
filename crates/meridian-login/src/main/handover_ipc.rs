fn bootsplash_handover() -> std::io::Result<()> {
    send_command(&bootsplash_socket_path(), b"handover\n").map(|_| ())
}

fn bootsplash_exit() -> std::io::Result<()> {
    send_command(&bootsplash_socket_path(), b"exit\n").map(|_| ())
}

fn bootsplash_socket_path() -> String {
    std::env::var(BOOTSPLASH_SOCKET_ENV).unwrap_or_else(|_| BOOTSPLASH_SOCKET.to_string())
}

fn send_command(path: &str, cmd: &[u8]) -> std::io::Result<String> {
    let mut s = UnixStream::connect(path)?;
    // 10s read timeout: covers the synchronous bootsplash handover which
    // only acks after release_master_lock, and that itself waits until
    // bootsplash's render loop reaches HANDOVER_MIN_T (~3s from boot).
    // 500ms was correct for the old fire-and-forget protocol but too
    // short for the new synchronous one.
    s.set_read_timeout(Some(Duration::from_secs(10)))?;
    s.set_write_timeout(Some(Duration::from_millis(500)))?;
    s.write_all(cmd)?;
    let mut buf = [0u8; 256];
    let n = s.read(&mut buf).unwrap_or(0);
    let resp = String::from_utf8_lossy(&buf[..n]).into_owned();
    if resp.starts_with("ok") {
        Ok(resp)
    } else {
        Err(std::io::Error::other(format!(
            "peer refused: {}",
            resp.trim()
        )))
    }
}

// ---- Phase 8 IPC: login as handover server ----

/// Events the compositor (or any client) can signal over the login IPC
/// socket. The login main loop reacts to these by dropping its DRM master /
/// fd (Handover) or by logging that the new compositor is fully visible
/// (Exit).
enum IpcEvent {
    /// Compositor is ready to take the screen. Login must release DRM
    /// master and close its card fd so the compositor's libseat acquire
    /// succeeds. This is the moment that gates the visible handover.
    Handover,
    /// Compositor's first frame is on screen. Informational; lets us log
    /// the handover latency and could later be used to suppress the
    /// fallback timeout.
    Exit,
}

/// Bind the login IPC socket and spawn an accept thread that forwards
/// each parsed command as an [`IpcEvent`] into `tx`. Returns the socket
/// path and identity so the caller can unlink exactly this socket on shutdown.
///
/// `owner_uid`/`owner_gid` are the authenticated user — we chown the
/// socket to them and clamp to mode 0600 so only the spawned compositor
/// (running as that user) can connect. Without this clamp the socket
/// would be world-connectable and any local non-root account could send
/// `handover` to make us drop DRM master prematurely.
fn spawn_login_ipc_server(
    tx: mpsc::Sender<IpcEvent>,
    owner_uid: u32,
    owner_gid: u32,
) -> std::io::Result<(PathBuf, SocketIdentity)> {
    let path = PathBuf::from(LOGIN_SOCKET);
    let _ = fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    // chown FIRST, then chmod: shrinking the access window during the
    // tiny gap between bind and lockdown.
    nix::unistd::chown(
        &path,
        Some(nix::unistd::Uid::from_raw(owner_uid)),
        Some(nix::unistd::Gid::from_raw(owner_gid)),
    )
    .map_err(|e| std::io::Error::other(format!("chown ipc socket: {}", e)))?;
    let socket_identity = secure_socket_permissions(&path)?;
    info!(
        path = %path.display(),
        owner_uid,
        owner_gid,
        "login ipc: listening (chown'd to owner, mode 0600)"
    );

    thread::spawn(move || {
        for conn in listener.incoming() {
            match conn {
                Ok(stream) => {
                    let tx2 = tx.clone();
                    thread::spawn(move || handle_login_ipc_client(stream, tx2));
                }
                Err(e) => {
                    warn!(error = %e, "login ipc: accept failed");
                    break;
                }
            }
        }
    });
    Ok((path, socket_identity))
}

fn handle_login_ipc_client(mut stream: UnixStream, tx: mpsc::Sender<IpcEvent>) {
    let read_clone = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let reader = BufReader::new(read_clone);
    for line in reader.lines() {
        let Ok(cmd) = line else { return };
        match cmd.trim() {
            "" => continue,
            "handover" => {
                let _ = tx.send(IpcEvent::Handover);
                let _ = writeln!(stream, "ok handover");
            }
            "exit" => {
                let _ = tx.send(IpcEvent::Exit);
                let _ = writeln!(stream, "ok exit");
            }
            other => {
                let _ = writeln!(stream, "err unknown command: {}", other);
            }
        }
    }
}
