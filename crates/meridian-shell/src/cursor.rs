//! Cursor settings for the Settings "Mauszeiger" page. Both the cursor *theme*
//! and *size* are adjustable here: the page lists the installed cursor themes
//! and the size presets. The active cursor is rendered by the compositor —
//! a change persists to the config and asks the compositor to reload.

use std::collections::BTreeSet;
use std::path::PathBuf;

use meridian_config::{CursorConfig, MeridianConfig};

/// Selectable cursor sizes, paired with their static widget id and chip label.
/// The ids are matched by `widget_action::action_for_id` (the `cursor-size-`
/// prefix) so they must stay in sync with that parser.
pub const CURSOR_SIZE_OPTIONS: &[(u32, &str, &str)] = &[
    (16, "cursor-size-16", "16 px"),
    (24, "cursor-size-24", "24 px"),
    (32, "cursor-size-32", "32 px"),
    (48, "cursor-size-48", "48 px"),
];

/// Static widget ids for the cursor-theme rows, indexed by position. The list
/// is bounded by this length (extra themes are not shown). The ids are matched
/// by `widget_action::action_for_id` (the `cursor-theme-` prefix), so they must
/// stay in sync with that parser.
pub const CURSOR_THEME_WIDGET_IDS: &[&str] = &[
    "cursor-theme-0",
    "cursor-theme-1",
    "cursor-theme-2",
    "cursor-theme-3",
    "cursor-theme-4",
    "cursor-theme-5",
    "cursor-theme-6",
    "cursor-theme-7",
    "cursor-theme-8",
    "cursor-theme-9",
    "cursor-theme-10",
    "cursor-theme-11",
];

/// The cursor theme name to show as the currently-selected one. Empty config
/// theme falls back to the `CursorConfig` default so the picker can highlight
/// a sensible row rather than nothing.
pub fn current_cursor_theme() -> String {
    let cursor = MeridianConfig::load().cursor.unwrap_or_default();
    if cursor.theme.trim().is_empty() {
        CursorConfig::default().theme
    } else {
        cursor.theme
    }
}

/// Standard XDG icon-theme search roots that may hold cursor themes.
fn cursor_theme_dirs() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    vec![
        PathBuf::from(format!("{}/.icons", home)),
        PathBuf::from(format!("{}/.local/share/icons", home)),
        PathBuf::from("/usr/share/icons"),
    ]
}

/// Scan the icon-theme roots for installed cursor themes — a directory counts
/// when it contains a `cursors/` subdirectory (the XDG cursor convention).
/// Names are de-duplicated and sorted; an entry earlier in the search path
/// wins, matching XCURSOR_PATH precedence. Bounded to the id-array length.
pub fn scan_cursor_themes() -> Vec<String> {
    scan_dirs_for_cursor_themes(&cursor_theme_dirs())
}

/// Pure scan over the given roots, so it can be tested against a temp fixture.
fn scan_dirs_for_cursor_themes(dirs: &[PathBuf]) -> Vec<String> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if !entry.path().join("cursors").is_dir() {
                continue;
            }
            if let Some(name) = entry.file_name().to_str() {
                names.insert(name.to_string());
            }
        }
    }
    names
        .into_iter()
        .take(CURSOR_THEME_WIDGET_IDS.len())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_options_ids_match_prefix_and_value() {
        for (px, id, label) in CURSOR_SIZE_OPTIONS {
            assert_eq!(*id, format!("cursor-size-{}", px));
            assert_eq!(*label, format!("{} px", px));
        }
    }

    #[test]
    fn theme_widget_ids_match_prefix_and_index() {
        for (i, id) in CURSOR_THEME_WIDGET_IDS.iter().enumerate() {
            assert_eq!(*id, format!("cursor-theme-{}", i));
        }
    }

    #[test]
    fn scan_cursor_themes_finds_dirs_with_cursors_subdir_only() {
        // Build a fake icon root: one theme with a cursors/ dir, one without.
        let root =
            std::env::temp_dir().join(format!("meridian-cursor-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("HasCursors/cursors")).unwrap();
        std::fs::create_dir_all(root.join("NoCursors/foo")).unwrap();

        let found = scan_dirs_for_cursor_themes(std::slice::from_ref(&root));
        assert!(found.contains(&"HasCursors".to_string()));
        assert!(!found.contains(&"NoCursors".to_string()));

        let _ = std::fs::remove_dir_all(&root);
    }
}
