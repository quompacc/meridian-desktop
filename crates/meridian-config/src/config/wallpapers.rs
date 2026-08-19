const WALLPAPER_SKIP: &[&str] = &[
    "usr",
    "share",
    "wallpapers",
    "backgrounds",
    "contents",
    "images",
    "pictures",
    "home",
];

fn collect_images_by_dir(
    dir: &std::path::Path,
    out: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<(u64, String)>>,
    depth: usize,
) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_images_by_dir(&path, out, depth - 1);
        } else {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase());
            if matches!(ext.as_deref(), Some("png" | "jpg" | "jpeg" | "webp")) {
                let sz = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                if let (Some(s), Some(parent)) = (path.to_str(), path.parent()) {
                    out.entry(parent.to_path_buf())
                        .or_default()
                        .push((sz, s.to_string()));
                }
            }
        }
    }
}

fn wallpaper_entry_display_name(path: &str) -> String {
    let meaningful: Vec<&str> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .filter(|p| {
            let lo = p.to_ascii_lowercase();
            !WALLPAPER_SKIP.contains(&lo.as_str())
        })
        .collect();
    let filename = meaningful.last().copied().unwrap_or(path);
    let stem = filename.rsplitn(2, '.').last().unwrap_or(filename);
    if meaningful.len() >= 2 {
        format!("{} \u{00b7} {}", meaningful[meaningful.len() - 2], stem)
    } else {
        stem.to_string()
    }
}

fn wallpaper_dir_display_name(dir: &std::path::Path) -> String {
    let s = dir.to_str().unwrap_or("");
    let meaningful: Vec<&str> = s
        .split('/')
        .filter(|c| !c.is_empty())
        .filter(|p| {
            let lo = p.to_ascii_lowercase();
            !WALLPAPER_SKIP.contains(&lo.as_str())
        })
        .collect();
    meaningful.last().copied().unwrap_or(s).to_string()
}

fn has_general_section(raw: &str) -> bool {
    raw.lines().any(|l| l.trim() == "[general]")
}

/// Returns the line index of the first `theme = ...` key that appears
/// after a `[general]` section header.
fn find_theme_line(raw: &str) -> Option<usize> {
    find_general_key_line(raw, "theme")
}

/// Line index of `<key> = ...` inside the `[general]` section, if present.
/// Matches the key exactly (not as a substring) so e.g. `theme` does not match
/// a hypothetical `theme_variant`.
fn find_general_key_line(raw: &str, key: &str) -> Option<usize> {
    let mut in_general = false;
    for (i, line) in raw.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('[') {
            in_general = t == "[general]";
            continue;
        }
        if in_general && t.starts_with(key) {
            let after = t[key.len()..].trim_start();
            if after.starts_with('=') {
                return Some(i);
            }
        }
    }
    None
}

/// Replace (or insert/remove) the `idle_timeout_secs` key in `[general]`,
/// preserving the rest of the file. `None` removes the key (idle blanking off;
/// the parsed value falls back to `None` when the key is absent). Pure string
/// transform so it can be unit-tested without a real config directory.
fn set_idle_timeout_in_toml(raw: &str, secs: Option<u64>) -> String {
    const KEY: &str = "idle_timeout_secs";
    let nl = '\n';

    let Some(secs) = secs else {
        // Drop the key if present; otherwise leave the file untouched.
        let Some(pos) = find_general_key_line(raw, KEY) else {
            return raw.to_string();
        };
        let mut out = String::new();
        for (i, l) in raw.lines().enumerate() {
            if i == pos {
                continue;
            }
            out.push_str(l);
            out.push(nl);
        }
        return out;
    };

    let new_line = format!("{} = {}", KEY, secs);

    if let Some(pos) = find_general_key_line(raw, KEY) {
        let mut out = String::new();
        for (i, l) in raw.lines().enumerate() {
            if i == pos {
                out.push_str(&new_line);
            } else {
                out.push_str(l);
            }
            out.push(nl);
        }
        out
    } else if has_general_section(raw) {
        let mut out = String::new();
        let mut inserted = false;
        for line in raw.lines() {
            out.push_str(line);
            out.push(nl);
            if !inserted && line.trim() == "[general]" {
                out.push_str(&new_line);
                out.push(nl);
                inserted = true;
            }
        }
        out
    } else {
        let mut out = raw.to_string();
        if !out.ends_with(nl) && !out.is_empty() {
            out.push(nl);
        }
        out.push_str("\n[general]\n");
        out.push_str(&new_line);
        out.push(nl);
        out
    }
}
