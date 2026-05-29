//! Read-only cursor settings (theme + size) for the Settings "Mauszeiger"
//! page. The active cursor is rendered by the compositor; this page reflects
//! the configured values from the Meridian config.

use meridian_config::{CursorConfig, MeridianConfig};

pub fn cursor_rows() -> Vec<(String, String)> {
    rows_from(&MeridianConfig::load().cursor.unwrap_or_default())
}

fn rows_from(cursor: &CursorConfig) -> Vec<(String, String)> {
    let theme = if cursor.theme.trim().is_empty() {
        "—".to_string()
    } else {
        cursor.theme.clone()
    };
    vec![
        ("Theme".to_string(), theme),
        ("Größe".to_string(), format!("{} px", cursor.size)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_show_theme_and_size() {
        let cursor = CursorConfig {
            theme: "Breeze_Light".to_string(),
            size: 24,
        };
        assert_eq!(
            rows_from(&cursor),
            vec![
                ("Theme".to_string(), "Breeze_Light".to_string()),
                ("Größe".to_string(), "24 px".to_string()),
            ]
        );
    }

    #[test]
    fn rows_dash_for_empty_theme() {
        let cursor = CursorConfig {
            theme: "  ".to_string(),
            size: 32,
        };
        assert_eq!(
            rows_from(&cursor)[0],
            ("Theme".to_string(), "—".to_string())
        );
    }
}
