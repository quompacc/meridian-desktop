//! `org.freedesktop.impl.portal.Settings` — exposes the desktop appearance so
//! toolkits/apps follow the Meridian theme. The key one is
//! `org.freedesktop.appearance` → `color-scheme` (0 = no preference, 1 = prefer
//! dark, 2 = prefer light), which Firefox, Chromium, GTK4/libadwaita and Qt6
//! (via the xdg-desktop-portal platform theme) read to pick a light/dark UI.
//!
//! The value is derived from the active Meridian theme's background luminance,
//! so it is the SAME single source as the shell — switch the Meridian theme and
//! apps match. A watcher task in `run()` polls this value and emits the
//! `SettingChanged` signal so already-running apps update live (no relaunch).

use std::collections::HashMap;

use tracing::info;
use zbus::zvariant::{OwnedValue, Value};

type Asv = HashMap<String, OwnedValue>;

const APPEARANCE: &str = "org.freedesktop.appearance";
const COLOR_SCHEME: &str = "color-scheme";

pub struct SettingsImpl;

pub(crate) const APPEARANCE_NS: &str = APPEARANCE;
pub(crate) const COLOR_SCHEME_KEY: &str = COLOR_SCHEME;

impl SettingsImpl {
    /// 0 = no preference, 1 = prefer dark, 2 = prefer light.
    pub(crate) fn color_scheme() -> u32 {
        let mut config = meridian_config::MeridianConfig::default();
        if let Err(err) = config.reload() {
            // A broken config must not silently fall back to the default
            // theme (P3-4, AUDIT_2026-08-19).
            tracing::warn!("Settings: config reload failed, using defaults: {err}");
        }
        let name = config.general.theme.trim();
        let name = if name.is_empty() { "dark" } else { name };
        let mut manager = meridian_config::ThemeManager::new();
        if let Err(err) = manager.set_theme(name) {
            tracing::warn!("Settings: theme {name:?} failed to load, using default: {err}");
        }
        if manager.current().config.appearance_is_light() {
            2
        } else {
            1
        }
    }

    fn color_scheme_value() -> Option<OwnedValue> {
        OwnedValue::try_from(Value::U32(Self::color_scheme())).ok()
    }

    fn namespace_requested(namespaces: &[String], ns: &str) -> bool {
        namespaces.is_empty()
            || namespaces.iter().any(|n| {
                n == ns
                    || n.strip_suffix('*')
                        .map(|prefix| ns.starts_with(prefix))
                        .unwrap_or(false)
            })
    }
}

/// Sorted mtimes (ms since epoch) of everything `color_scheme()` reads: the
/// user config plus every theme file ThemeManager could load. The watcher
/// compares this fingerprint each poll and skips the re-parse while nothing
/// changed on disk (P3-4, AUDIT_2026-08-19). Missing/unreadable files
/// contribute nothing; appearing or changing files alter the fingerprint.
pub(crate) fn appearance_source_mtimes() -> Vec<u128> {
    let mut mtimes = Vec::new();
    push_mtime(
        &mut mtimes,
        &meridian_config::config_directory().join("config.toml"),
    );
    let manager = meridian_config::ThemeManager::new();
    for dir in manager.theme_dirs() {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            push_mtime(&mut mtimes, &entry.path().join("theme.toml"));
        }
    }
    mtimes.sort_unstable();
    mtimes
}

fn push_mtime(out: &mut Vec<u128>, path: &std::path::Path) {
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    let Ok(modified) = meta.modified() else {
        return;
    };
    if let Ok(age) = modified.duration_since(std::time::UNIX_EPOCH) {
        out.push(age.as_millis());
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Settings")]
impl SettingsImpl {
    #[zbus(property)]
    fn version(&self) -> u32 {
        2
    }

    /// `ReadAll(as namespaces) -> a{sa{sv}}`
    fn read_all(&self, namespaces: Vec<String>) -> HashMap<String, Asv> {
        let mut out: HashMap<String, Asv> = HashMap::new();
        if Self::namespace_requested(&namespaces, APPEARANCE) {
            if let Some(value) = Self::color_scheme_value() {
                let mut group = Asv::new();
                group.insert(COLOR_SCHEME.to_string(), value);
                out.insert(APPEARANCE.to_string(), group);
            }
        }
        info!("Settings.ReadAll namespaces={namespaces:?} -> appearance/color-scheme");
        out
    }

    /// `ReadOne(s namespace, s key) -> v` (Settings v2).
    fn read_one(&self, namespace: &str, key: &str) -> zbus::fdo::Result<OwnedValue> {
        if namespace == APPEARANCE && key == COLOR_SCHEME {
            return Self::color_scheme_value()
                .ok_or_else(|| zbus::fdo::Error::Failed("color-scheme encode failed".into()));
        }
        Err(zbus::fdo::Error::Failed(format!(
            "unknown setting {namespace}/{key}"
        )))
    }

    /// `Read(s namespace, s key) -> v` (deprecated alias kept for older clients).
    fn read(&self, namespace: &str, key: &str) -> zbus::fdo::Result<OwnedValue> {
        self.read_one(namespace, key)
    }

    /// Emitted when a setting changes so already-running apps update live (e.g.
    /// dark/light switch). Fired by the watcher task in `run()`.
    #[zbus(signal)]
    pub(crate) async fn setting_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        namespace: &str,
        key: &str,
        value: zbus::zvariant::Value<'_>,
    ) -> zbus::Result<()>;
}
