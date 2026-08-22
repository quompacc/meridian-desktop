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

// On every platform where we implement a real peer-credential check, both
// ends of a socketpair live in this process, so the peer's effective uid
// must equal ours. This covers the Linux SO_PEERCRED path and the BSD/macOS
// getpeereid path; targets with the no-op fallback return None and are
// excluded.
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "macos",
    target_os = "ios",
))]
#[test]
fn peer_uid_of_socketpair_matches_own_euid() {
    let (a, _b) = UnixStream::pair().expect("socketpair");
    // SAFETY: geteuid has no preconditions and just reads our effective uid.
    let euid = unsafe { libc::geteuid() } as u32;
    let peer = super::peer_effective_uid(&a).expect("peer credentials");
    assert_eq!(peer, Some(euid));
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
fn event_listener_clone_observes_new_connections() {
    let dir = temp_runtime_dir("listener-clone");
    with_runtime_dir(&dir, || {
        let server = IpcServer::new();
        let listener = server
            .event_listener_clone()
            .expect("listener clone must be available");
        let _client = connect_client();
        let (_stream, _address) = listener.accept().expect("pending client connection");
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

    assert!(should_cleanup_socket_path(&socket_path, identity).expect("validate cleanup target"));

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

    assert!(!should_cleanup_socket_path(&socket_path, identity).expect("validate replacement path"));

    let _ = fs::remove_file(&socket_path);
    let _ = fs::remove_dir(&dir);
}
