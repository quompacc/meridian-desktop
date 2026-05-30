//! Cursor settings for the Settings "Mauszeiger" page. The cursor *theme*
//! stays read-only (theme enumeration is a separate task); the cursor *size*
//! is now adjustable from the page via the size chips below. The active cursor
//! is rendered by the compositor — changing the size here persists it to the
//! config and asks the compositor to reload.

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

/// The cursor theme name to show in the (read-only) theme row.
pub fn cursor_theme_label() -> String {
    theme_label_from(&MeridianConfig::load().cursor.unwrap_or_default())
}

fn theme_label_from(cursor: &CursorConfig) -> String {
    if cursor.theme.trim().is_empty() {
        "—".to_string()
    } else {
        cursor.theme.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_label_shows_theme_name() {
        let cursor = CursorConfig {
            theme: "Breeze_Light".to_string(),
            size: 24,
        };
        assert_eq!(theme_label_from(&cursor), "Breeze_Light");
    }

    #[test]
    fn theme_label_dash_for_empty_theme() {
        let cursor = CursorConfig {
            theme: "  ".to_string(),
            size: 32,
        };
        assert_eq!(theme_label_from(&cursor), "—");
    }

    #[test]
    fn size_options_ids_match_prefix_and_value() {
        for (px, id, label) in CURSOR_SIZE_OPTIONS {
            assert_eq!(*id, format!("cursor-size-{}", px));
            assert_eq!(*label, format!("{} px", px));
        }
    }
}
