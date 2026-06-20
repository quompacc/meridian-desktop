//! Export the active Meridian theme to the *legacy* desktop-config that GTK and
//! KDE/Qt applications read — derived entirely from the central design source
//! ([`meridian_config::ThemeConfig`] / `meridian-tokens`), with **no third-party
//! theme dependency**.
//!
//! Meridian serves `org.freedesktop.appearance color-scheme` over the Settings
//! portal, but most toolkits do not reconstruct a palette from it:
//!   - **KDE/Qt** (Breeze / `KColorScheme`, e.g. former defaults) read
//!     `~/.config/kdeglobals` — so we generate its `[Colors:*]` from the tokens.
//!   - **GTK** apps read a *theme*. Rather than depend on a shipped theme like
//!     Adwaita-dark, Meridian **generates its own GTK theme** from the tokens
//!     into `~/.local/share/themes/Meridian/` (gtk-3.0 + gtk-4.0 `gtk.css` from
//!     the templates in `assets/gtk/*.css.in`) and points `gtk-theme-name` at it.
//!     This keeps every app colour sourced from the single design pipeline.
//!
//! `KColorScheme`/GTK read these at application **startup**, so we export at
//! session start (before apps launch) and again on every live theme switch.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use meridian_config::{Color, ThemeConfig};

/// GTK theme name Meridian installs + selects. Its CSS is generated from tokens.
const GTK_THEME_NAME: &str = "Meridian";
const GTK3_TEMPLATE: &str = include_str!("../assets/gtk/gtk3.css.in");
const GTK4_TEMPLATE: &str = include_str!("../assets/gtk/gtk4.css.in");

/// Write every legacy theme artifact derived from `theme` and best-effort sync
/// the gsettings keys. Never fails the caller; problems are logged.
pub(crate) fn export_theme(theme: &ThemeConfig) {
    if let Some(cfg) = config_home() {
        write_file(&cfg.join("kdeglobals"), &kdeglobals_contents(theme));
        let gtk_ini = gtk_settings_ini(theme);
        write_file(&cfg.join("gtk-3.0").join("settings.ini"), &gtk_ini);
        write_file(&cfg.join("gtk-4.0").join("settings.ini"), &gtk_ini);
    } else {
        tracing::warn!("theme_export: no config dir; skipping kdeglobals/gtk settings");
    }

    if let Some(data) = data_home() {
        let theme_dir = data.join("themes").join(GTK_THEME_NAME);
        write_file(&theme_dir.join("index.theme"), &index_theme(theme));
        write_file(
            &theme_dir.join("gtk-3.0").join("gtk.css"),
            &substitute_tokens(GTK3_TEMPLATE, theme),
        );
        write_file(
            &theme_dir.join("gtk-4.0").join("gtk.css"),
            &substitute_tokens(GTK4_TEMPLATE, theme),
        );
    } else {
        tracing::warn!("theme_export: no data dir; skipping generated GTK theme");
    }

    apply_gsettings(theme);
    tracing::info!(
        "theme_export: Meridian theme exported (dark={})",
        !theme.appearance_is_light()
    );
}

/// `~/.config` from `XDG_CONFIG_HOME` (must be absolute) or `HOME`.
fn config_home() -> Option<PathBuf> {
    abs_env("XDG_CONFIG_HOME").or_else(|| home().map(|h| h.join(".config")))
}

/// `~/.local/share` from `XDG_DATA_HOME` (must be absolute) or `HOME`.
fn data_home() -> Option<PathBuf> {
    abs_env("XDG_DATA_HOME").or_else(|| home().map(|h| h.join(".local").join("share")))
}

fn abs_env(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).map(PathBuf::from).filter(|p| p.is_absolute())
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            tracing::warn!("theme_export: mkdir {:?}: {}", parent, e);
            return;
        }
    }
    if let Err(e) = fs::write(path, contents) {
        tracing::warn!("theme_export: write {:?}: {}", path, e);
    }
}

// ─── GTK theme ────────────────────────────────────────────────────────────────

/// Substitute the `@@TOKEN@@` placeholders in a GTK CSS template with the active
/// palette as `#rrggbb`. Longer names first so `@@SURFACE_ALT@@` is replaced
/// before `@@SURFACE@@` (defensive; the names are not substrings of each other).
fn substitute_tokens(template: &str, theme: &ThemeConfig) -> String {
    let c = &theme.colors;
    let d = &theme.decorations;
    let sel_fg = readable_on(c.accent, theme);
    template
        // Geometry tokens for the GTK CSD frame (border-radius / drop shadow) so
        // GTK apps render their OWN frame in the Meridian shape — we don't draw it.
        .replace("@@RADIUS@@", &d.corner_radius.to_string())
        .replace("@@SHADOW_RADIUS@@", &d.shadow_radius.to_string())
        .replace("@@SHADOW_OFFSET@@", &d.shadow_offset_y.to_string())
        .replace("@@SHADOW_ALPHA@@", &format!("{:.2}", d.shadow_alpha))
        .replace("@@SURFACE_ALT@@", &c.surface_alt.to_hex())
        .replace("@@SURFACE@@", &c.surface.to_hex())
        .replace("@@ACCENT_ALT@@", &c.accent_alt.to_hex())
        .replace("@@ACCENT@@", &c.accent.to_hex())
        .replace("@@TEXT_DIM@@", &c.text_dim.to_hex())
        .replace("@@TEXT@@", &c.text.to_hex())
        .replace("@@BG@@", &c.background.to_hex())
        .replace("@@BORDER@@", &c.border.to_hex())
        .replace("@@ERROR@@", &c.error.to_hex())
        .replace("@@WARNING@@", &c.warning.to_hex())
        .replace("@@SUCCESS@@", &c.success.to_hex())
        .replace("@@SEL_FG@@", &sel_fg.to_hex())
}

fn index_theme(theme: &ThemeConfig) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=X-GNOME-Metatheme\n\
         Name={name}\n\
         Comment=Generated from meridian-tokens — do not edit\n\n\
         [X-GNOME-Metatheme]\n\
         GtkTheme={name}\n\
         IconTheme={icons}\n",
        name = GTK_THEME_NAME,
        icons = app_icon_theme(theme),
    )
}

/// `~/.config/gtk-{3,4}.0/settings.ini` selecting the generated Meridian theme.
pub(crate) fn gtk_settings_ini(theme: &ThemeConfig) -> String {
    let prefer_dark = u8::from(!theme.appearance_is_light());
    format!(
        "[Settings]\n\
         gtk-theme-name={theme_name}\n\
         gtk-application-prefer-dark-theme={prefer_dark}\n\
         gtk-icon-theme-name={icons}\n\
         gtk-font-name={font}\n\
         gtk-cursor-theme-name={cursor}\n\
         gtk-cursor-theme-size={cursor_size}\n",
        theme_name = GTK_THEME_NAME,
        icons = app_icon_theme(theme),
        font = theme.fonts.ui,
        cursor = theme.cursor.theme,
        cursor_size = theme.cursor.size,
    )
}

// ─── kdeglobals (KDE/Qt) ──────────────────────────────────────────────────────

/// `kdeglobals` colour scheme; KColorScheme reads the `[Colors:*]` groups
/// directly, so no shipped `*.colors` file is needed.
pub(crate) fn kdeglobals_contents(theme: &ThemeConfig) -> String {
    let c = &theme.colors;
    let scheme = if theme.appearance_is_light() {
        "MeridianLight"
    } else {
        "MeridianDark"
    };
    let sel_fg = readable_on(c.accent, theme);

    let mut s = String::new();
    s.push_str("# Generated by Meridian (theme_export). Managed file.\n\n");
    push_color_group(&mut s, "Colors:Window", c.surface, theme);
    push_color_group(&mut s, "Colors:View", c.background, theme);
    push_color_group(&mut s, "Colors:Button", c.surface_alt, theme);
    push_color_group(&mut s, "Colors:Tooltip", c.surface_alt, theme);
    push_color_group(&mut s, "Colors:Complementary", c.background, theme);

    s.push_str("[Colors:Selection]\n");
    s.push_str(&format!("BackgroundNormal={}\n", triplet(c.accent)));
    s.push_str(&format!("ForegroundNormal={}\n", triplet(sel_fg)));
    s.push_str(&format!("ForegroundInactive={}\n", triplet(sel_fg)));
    s.push_str(&format!("DecorationFocus={}\n", triplet(c.accent)));
    s.push_str(&format!("DecorationHover={}\n", triplet(c.accent)));
    s.push('\n');

    s.push_str("[General]\n");
    s.push_str(&format!("ColorScheme={scheme}\n\n"));
    s.push_str("[Icons]\n");
    s.push_str(&format!("Theme={}\n\n", app_icon_theme(theme)));
    s.push_str("[KDE]\n");
    s.push_str("widgetStyle=Breeze\n");
    s
}

fn push_color_group(out: &mut String, group: &str, bg: Color, theme: &ThemeConfig) {
    let c = &theme.colors;
    out.push_str(&format!("[{group}]\n"));
    out.push_str(&format!("BackgroundNormal={}\n", triplet(bg)));
    out.push_str(&format!("BackgroundAlternate={}\n", triplet(c.surface_alt)));
    out.push_str(&format!("ForegroundNormal={}\n", triplet(c.text)));
    out.push_str(&format!("ForegroundInactive={}\n", triplet(c.text_dim)));
    out.push_str(&format!("ForegroundActive={}\n", triplet(c.accent)));
    out.push_str(&format!("ForegroundLink={}\n", triplet(c.accent)));
    out.push_str(&format!("ForegroundVisited={}\n", triplet(c.accent_alt)));
    out.push_str(&format!("ForegroundNegative={}\n", triplet(c.error)));
    out.push_str(&format!("ForegroundNeutral={}\n", triplet(c.warning)));
    out.push_str(&format!("ForegroundPositive={}\n", triplet(c.success)));
    out.push_str(&format!("DecorationFocus={}\n", triplet(c.accent)));
    out.push_str(&format!("DecorationHover={}\n", triplet(c.accent)));
    out.push('\n');
}

// ─── gsettings ────────────────────────────────────────────────────────────────

/// Best-effort sync of the interface gsettings so apps that read them at runtime
/// (libadwaita, Cinnamon/X-Apps) pick up the Meridian theme + dark preference.
fn apply_gsettings(theme: &ThemeConfig) {
    let dark = !theme.appearance_is_light();
    let scheme = if dark { "prefer-dark" } else { "default" };
    let icons = app_icon_theme(theme);
    for schema in ["org.gnome.desktop.interface", "org.cinnamon.desktop.interface"] {
        set_gsetting(schema, "gtk-theme", GTK_THEME_NAME);
        set_gsetting(schema, "icon-theme", &icons);
        // color-scheme only exists on the GNOME schema; harmless if absent.
        set_gsetting(schema, "color-scheme", scheme);
    }
}

fn set_gsetting(schema: &str, key: &str, value: &str) {
    if let Err(e) = Command::new("gsettings")
        .args(["set", schema, key, value])
        .status()
    {
        tracing::debug!("theme_export: gsettings {schema} {key}: {e}");
    }
}

// ─── helpers ──────────────────────────────────────────────────────────────────

/// The icon theme exported to OTHER apps (GTK / Qt / gsettings). Meridian's own
/// panel uses `theme.icons.theme` directly because it needs the recolourable
/// `-symbolic` set ("Papirus") — the Papirus-Dark/-Light variants lack those.
/// Apps want the full-colour dark/light variant instead, so map the base set to
/// it. Non-Papirus themes are exported unchanged.
fn app_icon_theme(theme: &ThemeConfig) -> String {
    let base = theme.icons.theme.trim();
    if base.eq_ignore_ascii_case("Papirus") {
        if theme.appearance_is_light() {
            "Papirus-Light".to_string()
        } else {
            "Papirus-Dark".to_string()
        }
    } else {
        base.to_string()
    }
}

/// `"r,g,b"` decimal triplet — the format KConfig expects for colour keys.
fn triplet(c: Color) -> String {
    format!("{},{},{}", c.r, c.g, c.b)
}

/// Pick whichever of the theme's text/background colour reads better on `bg`
/// (used for foregrounds painted on the accent, e.g. selection text).
fn readable_on(bg: Color, theme: &ThemeConfig) -> Color {
    let lum = 0.299 * bg.r as f32 + 0.587 * bg.g as f32 + 0.114 * bg.b as f32;
    if lum > 140.0 {
        theme.colors.background
    } else {
        theme.colors.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meridian_config::ThemeConfig;

    fn light_theme() -> ThemeConfig {
        let mut t = ThemeConfig::default();
        t.colors.background = Color::rgb(0xf2, 0xf2, 0xf2);
        t.colors.surface = Color::rgb(0xff, 0xff, 0xff);
        t.colors.text = Color::rgb(0x1a, 0x1a, 0x1a);
        t
    }

    #[test]
    fn triplet_formats_decimal_rgb() {
        assert_eq!(triplet(Color::rgb(0x20, 0x25, 0x2b)), "32,37,43");
        assert_eq!(triplet(Color::rgb(255, 255, 255)), "255,255,255");
    }

    #[test]
    fn readable_on_picks_contrasting_role() {
        let dark = ThemeConfig::default();
        assert_eq!(readable_on(Color::rgb(0xff, 0xff, 0xff), &dark), dark.colors.background);
        assert_eq!(readable_on(Color::rgb(0x10, 0x10, 0x10), &dark), dark.colors.text);
    }

    #[test]
    fn kdeglobals_dark_has_scheme_groups_and_breeze() {
        let s = kdeglobals_contents(&ThemeConfig::default());
        assert!(s.contains("[Colors:Window]"));
        assert!(s.contains("ColorScheme=MeridianDark"));
        assert!(s.contains("widgetStyle=Breeze"));
        assert!(s.contains("BackgroundNormal=32,37,43")); // surface 0x20252b
    }

    #[test]
    fn kdeglobals_light_switches_scheme_name() {
        assert!(kdeglobals_contents(&light_theme()).contains("ColorScheme=MeridianLight"));
    }

    #[test]
    fn gtk_ini_selects_meridian_theme_and_dark_flag() {
        let s = gtk_settings_ini(&ThemeConfig::default());
        assert!(s.contains("gtk-theme-name=Meridian"));
        assert!(s.contains("gtk-application-prefer-dark-theme=1"));
        // No third-party GTK *theme* dependency. (The Adwaita *cursor* is fine
        // and expected via gtk-cursor-theme-name.)
        assert!(!s.contains("Adwaita-dark"));
        assert!(s.contains("gtk-cursor-theme-name=Adwaita"));
    }

    #[test]
    fn gtk_ini_light_clears_dark_flag() {
        let s = gtk_settings_ini(&light_theme());
        assert!(s.contains("gtk-theme-name=Meridian"));
        assert!(s.contains("gtk-application-prefer-dark-theme=0"));
    }

    #[test]
    fn gtk_css_substitutes_all_tokens_with_hex() {
        let css3 = substitute_tokens(GTK3_TEMPLATE, &ThemeConfig::default());
        let css4 = substitute_tokens(GTK4_TEMPLATE, &ThemeConfig::default());
        // No placeholder may survive substitution.
        assert!(!css3.contains("@@"), "unsubstituted token in gtk3 css");
        assert!(!css4.contains("@@"), "unsubstituted token in gtk4 css");
        // Tokens resolved to the dark palette hexes.
        assert!(css3.contains("#14171b")); // background
        assert!(css3.contains("#4e99f3")); // accent
        // libadwaita named colour wired from tokens.
        assert!(css4.contains("@define-color window_bg_color #14171b"));
        assert!(css4.contains("@define-color accent_bg_color #4e99f3"));
    }

    #[test]
    fn index_theme_names_meridian() {
        let s = index_theme(&ThemeConfig::default());
        assert!(s.contains("Name=Meridian"));
        assert!(s.contains("GtkTheme=Meridian"));
    }

    #[test]
    fn app_icon_theme_maps_papirus_to_dark_light_variant() {
        // Panel keeps "Papirus" (symbolic); apps get the full-colour variant.
        let mut dark = ThemeConfig::default();
        dark.icons.theme = "Papirus".to_string();
        assert_eq!(app_icon_theme(&dark), "Papirus-Dark");

        let mut light = light_theme();
        light.icons.theme = "Papirus".to_string();
        assert_eq!(app_icon_theme(&light), "Papirus-Light");

        // Non-Papirus themes pass through unchanged.
        let mut custom = ThemeConfig::default();
        custom.icons.theme = "Adwaita".to_string();
        assert_eq!(app_icon_theme(&custom), "Adwaita");
    }
}
