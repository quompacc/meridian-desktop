//! Bundled diagnostic document. No filesystem or network lookup is needed at
//! runtime: HTML and component CSS are compiled into the executable.

use std::fmt::Write;

use meridian_app_catalog::DesktopApp;

use crate::icon_service;

const HTML: &str = include_str!("../assets/diagnostic.html");
const COMPONENT_CSS: &str = include_str!("../assets/diagnostic.css");
const ICON_SPRITE: &str = include_str!("../assets/icons.svg");
const LAUNCHER_SCRIPT: &str = include_str!("../assets/launcher.js");
const PANEL_HTML: &str = include_str!("../assets/panel.html");
const PANEL_CSS: &str = include_str!("../assets/panel.css");
const PANEL_SCRIPT: &str = include_str!("../assets/panel.js");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThemeChoice {
    Dark,
    Light,
}

impl ThemeChoice {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "dark" => Ok(Self::Dark),
            "light" => Ok(Self::Light),
            _ => Err(format!(
                "unsupported theme {value:?}; expected dark or light"
            )),
        }
    }

    fn attribute(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

pub(crate) fn diagnostic_html(theme: ThemeChoice, apps: &[DesktopApp]) -> String {
    HTML.replace(
        "@@MERIDIAN_TOKENS@@",
        &meridian_config::builtin_css_token_stylesheet(),
    )
    .replace("@@MERIDIAN_COMPONENT_CSS@@", COMPONENT_CSS)
    .replace("@@MERIDIAN_ICON_SPRITE@@", ICON_SPRITE)
    .replace("@@MERIDIAN_LAUNCHER_SCRIPT@@", LAUNCHER_SCRIPT)
    .replace("@@MERIDIAN_APP_CATALOG@@", &catalog_markup(apps))
    .replace("@@MERIDIAN_THEME@@", theme.attribute())
}

pub(crate) fn panel_html(theme: ThemeChoice, apps: &[DesktopApp]) -> String {
    PANEL_HTML
        .replace(
            "@@MERIDIAN_TOKENS@@",
            &meridian_config::builtin_css_token_stylesheet(),
        )
        .replace("@@MERIDIAN_PANEL_CSS@@", PANEL_CSS)
        .replace("@@MERIDIAN_ICON_SPRITE@@", ICON_SPRITE)
        .replace("@@MERIDIAN_PANEL_SCRIPT@@", PANEL_SCRIPT)
        .replace("@@MERIDIAN_PANEL_APPS@@", &panel_app_markup(apps))
        .replace("@@MERIDIAN_THEME@@", theme.attribute())
}

fn panel_app_markup(apps: &[DesktopApp]) -> String {
    const PREFERRED: [&str; 5] = ["thunar", "foot", "firefox", "chromium", "text-editor"];
    let mut selected = Vec::new();
    for preferred in PREFERRED {
        if let Some((index, app)) = apps.iter().enumerate().find(|(_, app)| {
            app.desktop_id.to_ascii_lowercase().contains(preferred)
                || app.program.to_ascii_lowercase().contains(preferred)
        }) {
            if !selected.iter().any(|(chosen, _)| *chosen == index) {
                selected.push((index, app));
            }
        }
        if selected.len() == 4 {
            break;
        }
    }
    for (index, app) in apps.iter().enumerate() {
        if selected.len() == 4 {
            break;
        }
        if !selected.iter().any(|(chosen, _)| *chosen == index) {
            selected.push((index, app));
        }
    }

    let mut markup = String::new();
    for (index, app) in selected {
        writeln!(
            markup,
            "<button class=\"panel-app\" type=\"button\" data-app-id=\"{}\" aria-label=\"{}\" title=\"{}\"><img src=\"{}\" alt=\"\"></button>",
            escape_html(&app.desktop_id),
            escape_html(&app.name),
            escape_html(&app.name),
            icon_service::uri(index),
        )
        .expect("writing to String cannot fail");
    }
    markup
}

fn catalog_markup(apps: &[DesktopApp]) -> String {
    let mut markup = String::new();
    for (index, app) in apps.iter().enumerate() {
        let category = launcher_category(&app.categories);
        let selected = if index == 0 { " is-selected" } else { "" };
        let favorite = index < 6;
        let name = escape_html(&app.name);
        let desktop_id = escape_html(&app.desktop_id);
        let search = escape_html(&format!("{} {}", app.name, app.categories.join(" ")));
        let icon_uri = icon_service::uri(index);
        writeln!(
            markup,
            "<button class=\"app{selected}\" type=\"button\" data-app-id=\"{desktop_id}\" data-category=\"{category}\" data-favorite=\"{favorite}\" data-search=\"{search}\"><span class=\"app__icon\"><svg class=\"icon\" aria-hidden=\"true\"><use href=\"#all-apps\"></use></svg><img class=\"app__image\" src=\"{icon_uri}\" alt=\"\"></span><span class=\"app__copy\"><strong>{name}</strong><small>{}</small></span></button>",
            category_label(category)
        )
        .expect("writing to String cannot fail");
    }
    markup
}

fn launcher_category(categories: &[String]) -> &'static str {
    for category in categories {
        let mapped = match category.as_str() {
            "network" | "webbrowser" | "email" => Some("internet"),
            "office" | "wordprocessor" | "spreadsheet" | "education" => Some("office"),
            "development" | "ide" => Some("development"),
            "graphics" | "photography" => Some("graphics"),
            "settings" | "system" => Some("system"),
            "utility" | "filemanager" | "accessibility" => Some("utilities"),
            _ => None,
        };
        if let Some(mapped) = mapped {
            return mapped;
        }
    }
    "utilities"
}

fn category_label(category: &str) -> &'static str {
    match category {
        "internet" => "Internet",
        "office" => "Büro",
        "development" => "Entwicklung",
        "graphics" => "Grafik",
        "system" => "System",
        _ => "Dienstprogramm",
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// The diagnostic document has no navigation UI. This allowlist is still
/// enforced at the WebView boundary so future markup cannot silently turn the
/// privileged-looking shell surface into a browser.
pub(crate) fn is_allowed_top_level_uri(uri: &str) -> bool {
    uri == "about:blank"
        || uri.starts_with("meridian://diagnostic/")
        || uri.starts_with("meridian://panel/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_apps() -> Vec<DesktopApp> {
        vec![DesktopApp::new(
            "Firefox".to_string(),
            vec!["firefox".to_string()],
            false,
        )]
    }

    #[test]
    fn document_contains_generated_tokens_and_selected_theme() {
        let html = diagnostic_html(ThemeChoice::Light, &sample_apps());
        assert!(html.contains("schema v1"));
        assert!(html.contains("data-meridian-theme=\"light\""));
        assert!(html.contains("id=\"favorites\""));
        assert!(!html.contains("@@MERIDIAN_"));
    }

    #[test]
    fn launcher_has_stable_search_catalog_and_session_regions() {
        let html = diagnostic_html(ThemeChoice::Dark, &sample_apps());
        for required in [
            "type=\"search\"",
            "Favoriten",
            "Alle Anwendungen",
            "aria-label=\"Kategorien\"",
            "class=\"launcher__footer\"",
            "Einstellungen",
            "Sperren",
            "Sitzung",
        ] {
            assert!(
                html.contains(required),
                "missing launcher region: {required}"
            );
        }
        for forbidden_branding in ["Kompass", "Meridian-Logo", "Weltkarte"] {
            assert!(!html.contains(forbidden_branding));
        }
    }

    #[test]
    fn bundled_document_has_no_remote_or_file_references() {
        let html = diagnostic_html(ThemeChoice::Dark, &sample_apps()).to_ascii_lowercase();
        for forbidden in ["http://", "https://", "file://", "@import"] {
            assert!(
                !html.contains(forbidden),
                "forbidden document input: {forbidden}"
            );
        }
    }

    #[test]
    fn component_css_uses_generated_values_only() {
        assert!(!COMPONENT_CSS.contains('#'));
        assert!(!COMPONENT_CSS.contains("px"));
        assert!(!COMPONENT_CSS.contains("color-mix"));
        assert!(COMPONENT_CSS.contains(".app[hidden]"));
    }

    #[test]
    fn launcher_script_is_bundled_and_has_no_ambient_io() {
        let html = diagnostic_html(ThemeChoice::Dark, &sample_apps()).to_ascii_lowercase();
        assert!(html.contains("applyfilter"));
        for forbidden in ["fetch(", "xmlhttprequest", "websocket", "localstorage"] {
            assert!(!html.contains(forbidden), "ambient script API: {forbidden}");
        }
    }

    #[test]
    fn panel_is_token_driven_and_contains_expected_regions() {
        let html = panel_html(ThemeChoice::Dark, &sample_apps());
        for required in [
            "Desktop-Leiste",
            "Anwendungen öffnen",
            "Angeheftete Anwendungen",
            "Systemstatus",
            "meridian-icon://app/0",
        ] {
            assert!(html.contains(required), "missing panel region: {required}");
        }
        assert!(!PANEL_CSS.contains('#'));
        assert!(!PANEL_CSS.contains("px"));
        assert!(!html.contains("@@MERIDIAN_"));
    }

    #[test]
    fn catalog_markup_escapes_metadata_and_uses_opaque_icon_ids() {
        let mut app = DesktopApp::new(
            "Bad <name> & quote\"".to_string(),
            vec!["bad".to_string()],
            false,
        );
        app.desktop_id = "bad\".desktop".to_string();
        app.icon_name = Some("/secret/icon.png".to_string());
        let html = diagnostic_html(ThemeChoice::Dark, &[app]);
        assert!(html.contains("Bad &lt;name&gt; &amp; quote&quot;"));
        assert!(html.contains("data-app-id=\"bad&quot;.desktop\""));
        assert!(html.contains("meridian-icon://app/0"));
        assert!(!html.contains("/secret/icon.png"));
    }

    #[test]
    fn navigation_allowlist_is_deny_by_default() {
        assert!(is_allowed_top_level_uri("about:blank"));
        assert!(is_allowed_top_level_uri("meridian://diagnostic/index"));
        assert!(is_allowed_top_level_uri("meridian://panel/index"));
        for denied in [
            "https://example.com",
            "http://localhost",
            "file:///etc/passwd",
            "data:text/html,hello",
            "javascript:alert(1)",
        ] {
            assert!(!is_allowed_top_level_uri(denied), "allowed {denied}");
        }
    }

    #[test]
    fn theme_parser_accepts_exactly_two_themes() {
        assert_eq!(ThemeChoice::parse("dark"), Ok(ThemeChoice::Dark));
        assert_eq!(ThemeChoice::parse("light"), Ok(ThemeChoice::Light));
        assert!(ThemeChoice::parse("system").is_err());
        assert!(ThemeChoice::parse("custom").is_err());
    }
}
