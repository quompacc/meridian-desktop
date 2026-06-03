//! Default application management — the data layer behind the Settings
//! "Standard-Apps" page.
//!
//! The Linux convention for "what app opens what MIME type" lives in
//! `~/.config/mimeapps.list` (user override) and `/usr/share/applications/`
//! (system-wide). The supported way to read/write it is the `xdg-mime` CLI:
//!
//!     xdg-mime query default <mime>          -> "<app>.desktop\n"
//!     xdg-mime default       <app> <mime>... -> updates mimeapps.list
//!
//! We wrap that with strong-typed categories (web browser, email, etc.) so
//! the settings UI doesn't have to think about individual MIME strings, plus
//! a sensible-defaults heuristic that picks an installed app for empty
//! categories.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const XDG_DATA_DIRS_DEFAULT: &str = "/usr/local/share:/usr/share";

/// One of the user-facing default-app categories the settings page exposes.
/// Each category is a cluster of related MIME types plus a single
/// "representative" MIME used to query the current default; setting a default
/// stamps the chosen app across every MIME in the cluster so e.g. picking an
/// image viewer applies to `.png`, `.jpg`, `.webp` etc. in one go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DefaultAppCategory {
    WebBrowser,
    Email,
    FileManager,
    TextEditor,
    ImageViewer,
    VideoPlayer,
    AudioPlayer,
    Pdf,
    Archive,
}

impl DefaultAppCategory {
    pub const ALL: &'static [DefaultAppCategory] = &[
        Self::WebBrowser,
        Self::Email,
        Self::FileManager,
        Self::TextEditor,
        Self::ImageViewer,
        Self::VideoPlayer,
        Self::AudioPlayer,
        Self::Pdf,
        Self::Archive,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::WebBrowser => "Web-Browser",
            Self::Email => "E-Mail",
            Self::FileManager => "Dateimanager",
            Self::TextEditor => "Texteditor",
            Self::ImageViewer => "Bildbetrachter",
            Self::VideoPlayer => "Videoplayer",
            Self::AudioPlayer => "Audioplayer",
            Self::Pdf => "PDF",
            Self::Archive => "Archive",
        }
    }

    /// The single MIME used to query the current default for this category.
    /// (Setting writes to every MIME in `all_mimes`.)
    pub fn representative_mime(self) -> &'static str {
        match self {
            Self::WebBrowser => "x-scheme-handler/https",
            Self::Email => "x-scheme-handler/mailto",
            Self::FileManager => "inode/directory",
            Self::TextEditor => "text/plain",
            Self::ImageViewer => "image/jpeg",
            Self::VideoPlayer => "video/mp4",
            Self::AudioPlayer => "audio/mpeg",
            Self::Pdf => "application/pdf",
            Self::Archive => "application/zip",
        }
    }

    /// Every MIME type stamped when the user picks an app for this category.
    /// Picking an image viewer should cover png/jpeg/gif/webp/etc. in one go;
    /// the user shouldn't have to set each one separately.
    pub fn all_mimes(self) -> &'static [&'static str] {
        match self {
            Self::WebBrowser => &[
                "x-scheme-handler/http",
                "x-scheme-handler/https",
                "x-scheme-handler/about",
                "x-scheme-handler/unknown",
                "text/html",
                "application/xhtml+xml",
            ],
            Self::Email => &["x-scheme-handler/mailto"],
            Self::FileManager => &["inode/directory"],
            Self::TextEditor => &["text/plain", "text/markdown", "text/x-readme", "text/x-log"],
            Self::ImageViewer => &[
                "image/jpeg",
                "image/png",
                "image/gif",
                "image/webp",
                "image/svg+xml",
                "image/bmp",
                "image/tiff",
                "image/heif",
                "image/heic",
            ],
            Self::VideoPlayer => &[
                "video/mp4",
                "video/x-matroska",
                "video/webm",
                "video/quicktime",
                "video/x-msvideo",
                "video/mpeg",
                "video/ogg",
            ],
            Self::AudioPlayer => &[
                "audio/mpeg",
                "audio/flac",
                "audio/ogg",
                "audio/wav",
                "audio/x-wav",
                "audio/aac",
                "audio/mp4",
                "audio/x-vorbis+ogg",
                "audio/opus",
            ],
            Self::Pdf => &["application/pdf"],
            Self::Archive => &[
                "application/zip",
                "application/x-tar",
                "application/x-rar",
                "application/vnd.rar",
                "application/x-7z-compressed",
                "application/gzip",
                "application/x-bzip2",
                "application/x-xz",
            ],
        }
    }

    /// Heuristic ranking of known-good apps per category, by preference.
    /// First installed match wins when sensible-defaults runs.
    pub fn preferred_desktop_ids(self) -> &'static [&'static str] {
        match self {
            Self::WebBrowser => &[
                "firefox.desktop",
                "org.mozilla.firefox.desktop",
                "firefox-esr.desktop",
                "chromium.desktop",
                "google-chrome.desktop",
                "brave-browser.desktop",
                "vivaldi-stable.desktop",
                "org.gnome.Epiphany.desktop",
                "org.kde.falkon.desktop",
            ],
            Self::Email => &[
                "thunderbird.desktop",
                "org.mozilla.Thunderbird.desktop",
                "org.kde.kmail2.desktop",
                "org.gnome.Geary.desktop",
                "evolution.desktop",
            ],
            Self::FileManager => &[
                "org.gnome.Nautilus.desktop",
                "org.kde.dolphin.desktop",
                "thunar.desktop",
                "pcmanfm.desktop",
                "nemo.desktop",
            ],
            Self::TextEditor => &[
                "org.gnome.TextEditor.desktop",
                "org.gnome.gedit.desktop",
                "code.desktop",
                "code-oss.desktop",
                "org.kde.kate.desktop",
                "org.kde.kwrite.desktop",
                "mousepad.desktop",
                "xed.desktop",
            ],
            Self::ImageViewer => &[
                "org.gnome.Loupe.desktop",
                "org.gnome.eog.desktop",
                "org.kde.gwenview.desktop",
                "feh.desktop",
                "qimgv.desktop",
            ],
            Self::VideoPlayer => &[
                "mpv.desktop",
                "io.mpv.Mpv.desktop",
                "org.videolan.VLC.desktop",
                "vlc.desktop",
                "org.gnome.Totem.desktop",
                "org.kde.haruna.desktop",
                "org.kde.dragonplayer.desktop",
            ],
            Self::AudioPlayer => &[
                "io.bassi.Amberol.desktop",
                "org.gnome.Rhythmbox3.desktop",
                "rhythmbox.desktop",
                "org.kde.elisa.desktop",
                "audacious.desktop",
                "org.videolan.VLC.desktop",
                "vlc.desktop",
                "mpv.desktop",
            ],
            Self::Pdf => &[
                "org.gnome.Evince.desktop",
                "org.kde.okular.desktop",
                "xreader.desktop",
                "qpdfview.desktop",
                "atril.desktop",
            ],
            Self::Archive => &[
                "org.gnome.FileRoller.desktop",
                "file-roller.desktop",
                "org.kde.ark.desktop",
                "xarchiver.desktop",
            ],
        }
    }
}

/// A `.desktop` file that declares it handles at least one MIME type. The
/// settings UI shows a dropdown of these per category; the user picks one
/// and `set_default_for_mimes` stamps it onto every MIME of the category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MimeAppCandidate {
    /// e.g. `firefox.desktop`. Identifies the app inside `xdg-mime default`.
    pub desktop_id: String,
    /// Human-readable name (`Name=` field).
    pub name: String,
    /// `Icon=` value, when present.
    pub icon: Option<String>,
    /// All MIME types this app advertises (`MimeType=...`).
    pub mime_types: Vec<String>,
}

/// Snapshot of every desktop entry on the system that handles at least one
/// MIME type, indexed for quick "apps for MIME" lookup. Built once and reused
/// across UI ticks; `xdg-mime` reads + writes still go to disk per call.
#[derive(Debug, Clone, Default)]
pub struct MimeAppIndex {
    apps: Vec<MimeAppCandidate>,
    by_mime: HashMap<String, Vec<usize>>,
}

impl MimeAppIndex {
    pub fn load_system() -> Self {
        let mut apps = Vec::new();
        let mut by_mime: HashMap<String, Vec<usize>> = HashMap::new();
        for dir in desktop_app_dirs() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.extension().map(|e| e == "desktop").unwrap_or(false) {
                    continue;
                }
                let Some(app) = parse_mime_candidate(&path) else {
                    continue;
                };
                if app.mime_types.is_empty() {
                    continue;
                }
                // Dedupe by desktop_id (user override > /usr/local > /usr).
                if apps
                    .iter()
                    .any(|a: &MimeAppCandidate| a.desktop_id == app.desktop_id)
                {
                    continue;
                }
                let idx = apps.len();
                for mime in &app.mime_types {
                    by_mime.entry(mime.clone()).or_default().push(idx);
                }
                apps.push(app);
            }
        }
        Self { apps, by_mime }
    }

    /// All candidate apps that advertise `mime` in their `MimeType=` line,
    /// sorted by display name (alphabetical, case-insensitive).
    pub fn apps_for_mime(&self, mime: &str) -> Vec<&MimeAppCandidate> {
        let mut out: Vec<&MimeAppCandidate> = self
            .by_mime
            .get(mime)
            .map(|idxs| idxs.iter().filter_map(|i| self.apps.get(*i)).collect())
            .unwrap_or_default();
        out.sort_by_key(|a| a.name.to_lowercase());
        out
    }

    /// Lookup the displayed app candidate behind a `desktop_id`.
    pub fn lookup(&self, desktop_id: &str) -> Option<&MimeAppCandidate> {
        self.apps.iter().find(|a| a.desktop_id == desktop_id)
    }
}

/// Snapshot the current default `.desktop` id for every category in one
/// pass. Categories with no default are absent from the map. Each lookup
/// spawns an `xdg-mime` subprocess so this isn't free; call it on page
/// transitions / after a write, not on every render.
pub fn snapshot_current_defaults() -> HashMap<DefaultAppCategory, String> {
    let mut out = HashMap::with_capacity(DefaultAppCategory::ALL.len());
    for cat in DefaultAppCategory::ALL {
        if let Some(id) = query_default(cat.representative_mime()) {
            out.insert(*cat, id);
        }
    }
    out
}

/// Pick a file-manager command line for `inode/directory`. Probes a
/// curated list of known managers in `/usr/bin`; falls back to `gio open`
/// (which honours mimeapps.list via GIO). Returns `(program, args)` ready
/// for `ShellCommand::LaunchApp`; the path argument is the user's home
/// directory.
pub fn pick_file_manager() -> (String, Vec<String>) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    for fm in ["dolphin", "nautilus", "thunar", "pcmanfm", "nemo", "caja"] {
        let path = std::path::PathBuf::from("/usr/bin").join(fm);
        if path.exists() {
            return (fm.to_string(), vec![home]);
        }
    }
    ("gio".to_string(), vec!["open".to_string(), home])
}

/// Run `xdg-mime query default <mime>` and return the desktop-id (e.g.
/// `firefox.desktop`) or `None` if nothing is currently registered.
pub fn query_default(mime: &str) -> Option<String> {
    let out = Command::new("xdg-mime")
        .env("LC_ALL", "C")
        .args(["query", "default", mime])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let id = String::from_utf8(out.stdout).ok()?.trim().to_string();
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

/// Stamp `desktop_id` as the default for every MIME in `mimes`. Returns
/// `true` on success. Logged failures on the caller side.
pub fn set_default_for_mimes(desktop_id: &str, mimes: &[&str]) -> bool {
    if mimes.is_empty() {
        return true;
    }
    let mut argv: Vec<String> = Vec::with_capacity(mimes.len() + 2);
    argv.push("default".to_string());
    argv.push(desktop_id.to_string());
    argv.extend(mimes.iter().map(|m| (*m).to_string()));
    let arg_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    Command::new("xdg-mime")
        .env("LC_ALL", "C")
        .args(&arg_refs)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// For each category with no current default, pick the first preferred app
/// that's actually installed and capable of handling the representative MIME,
/// and stamp it onto every MIME in the category. Returns the list of changes
/// applied (category, chosen desktop-id) so the caller can log / surface them.
pub fn apply_sensible_defaults_for_empty(
    index: &MimeAppIndex,
) -> Vec<(DefaultAppCategory, String)> {
    let mut applied = Vec::new();
    for cat in DefaultAppCategory::ALL {
        if query_default(cat.representative_mime()).is_some() {
            continue;
        }
        let chosen = cat
            .preferred_desktop_ids()
            .iter()
            .find(|id| {
                index.lookup(id).is_some_and(|app| {
                    app.mime_types
                        .iter()
                        .any(|m| m == cat.representative_mime())
                })
            })
            .copied();
        let Some(desktop_id) = chosen else {
            continue;
        };
        if set_default_for_mimes(desktop_id, cat.all_mimes()) {
            applied.push((*cat, desktop_id.to_string()));
        }
    }
    applied
}

// ─── Internals ───────────────────────────────────────────────────────────────

fn desktop_app_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let mut user = PathBuf::from(home);
        user.push(".local/share/applications");
        out.push(user);
    }
    let data_dirs =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| XDG_DATA_DIRS_DEFAULT.to_string());
    for dir in data_dirs.split(':').filter(|s| !s.is_empty()) {
        let mut path = PathBuf::from(dir);
        path.push("applications");
        out.push(path);
    }
    out
}

fn parse_mime_candidate(path: &std::path::Path) -> Option<MimeAppCandidate> {
    let raw = fs::read_to_string(path).ok()?;
    let desktop_id = path.file_name()?.to_str()?.to_string();
    let mut name: Option<String> = None;
    let mut icon: Option<String> = None;
    let mut mime_types: Vec<String> = Vec::new();
    let mut in_desktop_entry = false;
    let mut no_display = false;
    let mut hidden = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if let Some(section) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            in_desktop_entry = section.eq_ignore_ascii_case("Desktop Entry");
            continue;
        }
        if !in_desktop_entry {
            continue;
        }
        let Some((raw_key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = raw_key.trim();
        // Localized keys look like Name[de]=...; for now we keep only the
        // unlocalized fallback. A proper LC_MESSAGES match is a follow-on.
        let bare_key = key.split('[').next().unwrap_or(key);
        let value = value.trim();
        match bare_key {
            "Name" if !key.contains('[') && name.is_none() => name = Some(value.to_string()),
            "Icon" if !key.contains('[') && icon.is_none() => icon = Some(value.to_string()),
            "MimeType" => {
                for m in value.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                    if !mime_types.iter().any(|existing| existing == m) {
                        mime_types.push(m.to_string());
                    }
                }
            }
            "NoDisplay" => no_display = value.eq_ignore_ascii_case("true"),
            "Hidden" => hidden = value.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }
    if hidden || no_display {
        return None;
    }
    Some(MimeAppCandidate {
        desktop_id,
        name: name.unwrap_or_else(|| "Unbenannte App".to_string()),
        icon,
        mime_types,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_metadata_matches_per_variant() {
        for cat in DefaultAppCategory::ALL {
            assert!(!cat.label().is_empty(), "label missing for {:?}", cat);
            assert!(
                !cat.representative_mime().is_empty(),
                "representative mime missing for {:?}",
                cat
            );
            assert!(
                cat.all_mimes()
                    .iter()
                    .any(|m| *m == cat.representative_mime()),
                "all_mimes must include representative_mime for {:?}",
                cat
            );
            assert!(
                !cat.preferred_desktop_ids().is_empty(),
                "preferred desktop ids missing for {:?}",
                cat
            );
        }
    }

    #[test]
    fn parse_minimal_desktop_entry_with_mimes() {
        let raw = r#"
[Desktop Entry]
Type=Application
Name=Demo Viewer
Exec=demo %U
Icon=demo
MimeType=image/png;image/jpeg;
"#;
        let path = std::path::PathBuf::from("/tmp/demo.desktop");
        std::fs::write(&path, raw).unwrap();
        let app = parse_mime_candidate(&path).expect("parsed");
        assert_eq!(app.desktop_id, "demo.desktop");
        assert_eq!(app.name, "Demo Viewer");
        assert_eq!(app.icon.as_deref(), Some("demo"));
        assert_eq!(
            app.mime_types,
            vec!["image/png".to_string(), "image/jpeg".to_string()]
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn parse_hidden_entry_is_skipped() {
        let raw =
            "[Desktop Entry]\nType=Application\nName=Hidden\nMimeType=text/plain\nNoDisplay=true\n";
        let path = std::path::PathBuf::from("/tmp/hidden-default-apps-test.desktop");
        std::fs::write(&path, raw).unwrap();
        assert!(parse_mime_candidate(&path).is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn index_apps_for_mime_returns_only_matching() {
        let mut idx = MimeAppIndex::default();
        idx.apps.push(MimeAppCandidate {
            desktop_id: "a.desktop".to_string(),
            name: "Alpha".to_string(),
            icon: None,
            mime_types: vec!["image/png".to_string()],
        });
        idx.apps.push(MimeAppCandidate {
            desktop_id: "b.desktop".to_string(),
            name: "Bravo".to_string(),
            icon: None,
            mime_types: vec!["text/plain".to_string()],
        });
        idx.by_mime.insert("image/png".to_string(), vec![0]);
        idx.by_mime.insert("text/plain".to_string(), vec![1]);
        let png = idx.apps_for_mime("image/png");
        assert_eq!(png.len(), 1);
        assert_eq!(png[0].desktop_id, "a.desktop");
        let none = idx.apps_for_mime("audio/mpeg");
        assert!(none.is_empty());
    }
}
