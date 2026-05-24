use std::collections::HashMap;
use tokio::process::Command;
use tracing::{debug, info, warn};
use zbus::zvariant::{Array, ObjectPath, OwnedValue, Signature, Value};

type Asv = HashMap<String, OwnedValue>;
const FILE_PICKER_ENV: &str = "MERIDIAN_FILE_PICKER";
const DEFAULT_FILE_PICKER: &str = "/usr/local/bin/meridian-file-picker";

pub struct FileChooserImpl;

#[zbus::interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooserImpl {
    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }

    async fn open_file(
        &self,
        _handle: ObjectPath<'_>,
        _app_id: &str,
        _parent_window: &str,
        title: &str,
        options: Asv,
    ) -> (u32, Asv) {
        let multiple = bool_opt(&options, "multiple");
        let directory = bool_opt(&options, "directory");
        debug!("OpenFile title={title:?} multiple={multiple} directory={directory}");

        let mut cmd = Command::new(file_picker_path());
        cmd.arg("--title").arg(title);
        if multiple {
            cmd.arg("--multiple");
        }
        if directory {
            cmd.arg("--directory");
        }
        forward_env(&mut cmd);

        match cmd.output().await {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let uris: Vec<String> = stdout
                    .lines()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(path_to_uri)
                    .collect();
                info!("OpenFile: {} file(s) selected", uris.len());
                (0, uris_asv(uris))
            }
            Ok(out) => {
                let code = out.status.code().unwrap_or(-1);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                info!("OpenFile: cancelled (exit={code}) stderr={stderr:?} stdout={stdout:?}");
                (1, Asv::new())
            }
            Err(e) => {
                warn!("OpenFile: picker error: {e}");
                (2, Asv::new())
            }
        }
    }

    async fn save_file(
        &self,
        _handle: ObjectPath<'_>,
        _app_id: &str,
        _parent_window: &str,
        title: &str,
        options: Asv,
    ) -> (u32, Asv) {
        let current_name = str_opt(&options, "current_name").map(str::to_owned);
        debug!("SaveFile title={title:?}");

        let mut cmd = Command::new(file_picker_path());
        cmd.arg("--save").arg("--title").arg(title);
        if let Some(ref name) = current_name {
            cmd.arg("--filename").arg(name);
        }
        forward_env(&mut cmd);

        match cmd.output().await {
            Ok(out) if out.status.success() => {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                if path.is_empty() {
                    return (1, Asv::new());
                }
                let uri = path_to_uri(&path);
                info!("SaveFile: selected {uri:?}");
                (0, str_asv("uri", uri))
            }
            Ok(_) => (1, Asv::new()),
            Err(e) => {
                warn!("SaveFile: picker error: {e}");
                (2, Asv::new())
            }
        }
    }

    async fn save_files(
        &self,
        _handle: ObjectPath<'_>,
        _app_id: &str,
        _parent_window: &str,
        title: &str,
        _options: Asv,
    ) -> (u32, Asv) {
        debug!("SaveFiles title={title:?} (directory picker)");
        let mut cmd = Command::new(file_picker_path());
        cmd.arg("--directory").arg("--title").arg(title);
        forward_env(&mut cmd);

        match cmd.output().await {
            Ok(out) if out.status.success() => {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                if path.is_empty() {
                    (1, Asv::new())
                } else {
                    (0, str_asv("destination", path))
                }
            }
            Ok(_) => (1, Asv::new()),
            Err(e) => {
                warn!("SaveFiles: picker error: {e}");
                (2, Asv::new())
            }
        }
    }
}

// Build a{sv} with a single string value.
fn str_asv(key: &str, value: String) -> Asv {
    let mut m = Asv::new();
    if let Ok(owned) = Value::from(value).try_to_owned() {
        m.insert(key.into(), owned);
    }
    m
}

// Build a{sv} with a "uris" key holding an array of strings.
fn uris_asv(uris: Vec<String>) -> Asv {
    let mut m = Asv::new();
    let sig: Signature = "s".try_into().expect("valid sig");
    let mut arr = Array::new(&sig);
    for uri in uris {
        let _ = arr.append(Value::from(uri));
    }
    if let Ok(owned) = Value::Array(arr).try_to_owned() {
        m.insert("uris".into(), owned);
    }
    m
}

fn bool_opt(opts: &Asv, key: &str) -> bool {
    opts.get(key)
        .and_then(|v| bool::try_from(v).ok())
        .unwrap_or(false)
}

fn str_opt<'a>(opts: &'a Asv, key: &str) -> Option<&'a str> {
    opts.get(key).and_then(|v| <&str>::try_from(v).ok())
}

fn path_to_uri(path: &str) -> String {
    if path.starts_with("file://") {
        path.to_string()
    } else {
        format!("file://{}", percent_encode_path(path))
    }
}

fn file_picker_path() -> String {
    std::env::var(FILE_PICKER_ENV).unwrap_or_else(|_| DEFAULT_FILE_PICKER.to_string())
}

fn percent_encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn forward_env(cmd: &mut Command) {
    for var in ["WAYLAND_DISPLAY", "DISPLAY", "XDG_RUNTIME_DIR"] {
        if let Ok(val) = std::env::var(var) {
            cmd.env(var, val);
        }
    }
    // GTK3 GtkFileChooserDialog — never touches the portal.
    cmd.env("GDK_BACKEND", "wayland");
}

#[cfg(test)]
mod tests {
    use super::path_to_uri;

    #[test]
    fn path_to_uri_preserves_existing_file_uri() {
        assert_eq!(
            path_to_uri("file:///tmp/already%20encoded"),
            "file:///tmp/already%20encoded"
        );
    }

    #[test]
    fn path_to_uri_percent_encodes_path_bytes() {
        assert_eq!(
            path_to_uri("/tmp/Meridian Test/ä.png"),
            "file:///tmp/Meridian%20Test/%C3%A4.png"
        );
    }
}
