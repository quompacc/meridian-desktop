use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    io::Error,
    sync::Arc,
    time::{Duration, Instant},
};

use meridian_config::{MeridianConfig, OutputEntry, ThemeManager};
use meridian_wm::WmWorkspace;
use smithay::{
    backend::{allocator::Format, drm::DrmDevice},
    desktop::{layer_map_for_output, PopupManager},
    input::SeatState,
    output::{Mode as OutputMode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, Mode, PostAction},
        drm::control::Device as _,
        wayland_server::Display,
    },
    utils::Transform,
    wayland::{
        compositor::CompositorState,
        dmabuf::DmabufState,
        fractional_scale::FractionalScaleManagerState,
        idle_inhibit::IdleInhibitManagerState,
        idle_notify::IdleNotifierState,
        input_method::InputMethodManagerState,
        output::OutputManagerState,
        presentation::PresentationState,
        selection::{data_device::DataDeviceState, primary_selection::PrimarySelectionState},
        session_lock::SessionLockManagerState,
        shell::{wlr_layer::WlrLayerShellState, xdg::XdgShellState},
        shm::ShmState,
        socket::ListeningSocketSource,
        text_input::TextInputManagerState,
        viewporter::ViewporterState,
        xdg_activation::XdgActivationState,
        xwayland_shell::XWaylandShellState,
    },
};

use crate::{
    backend::drm::{DisabledDrmOutput, DrmOutput},
    decoration::DecorationManager,
    wallpaper::WallpaperManager,
    workspace::WorkspaceManager,
};
use smithay::wayland::{
    image_capture_source::{ImageCaptureSourceState, OutputCaptureSourceState},
    image_copy_capture::ImageCopyCaptureState,
};
use wayland_protocols_wlr::output_power_management::v1::server::zwlr_output_power_manager_v1::ZwlrOutputPowerManagerV1;

use super::{
    detect_output_reload_diff, parse_output_transform, ClientState, ConnectedOutput,
    IdleInhibitorSet, IpcServer, LockManager, MeridianState, OutputGeometry, OutputId,
    OutputPowerManager, OutputReconfigure, OutputRegistration, OutputRegistry,
    WorkspaceOutputState,
};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ThemeOverrideChanges {
    pub theme_changed: bool,
    pub cursor_changed: bool,
    pub wallpaper_changed: bool,
}

pub(crate) fn apply_config_overrides(
    theme_manager: &mut ThemeManager,
    meridian_config: &MeridianConfig,
) -> ThemeOverrideChanges {
    let prev_theme_name = theme_manager.current().name.clone();
    let prev_cursor = (
        theme_manager.current().config.cursor.theme.clone(),
        theme_manager.current().config.cursor.size,
    );
    let prev_wallpaper = theme_manager
        .current()
        .config
        .wallpaper
        .as_ref()
        .map(|w| (w.path.clone(), w.mode));

    let requested_theme = if meridian_config.general.theme.trim().is_empty() {
        "default"
    } else {
        meridian_config.general.theme.trim()
    };
    if let Err(err) = theme_manager.set_theme(requested_theme) {
        tracing::warn!(
            "Failed to load theme {:?} from config: {} — keeping current theme {:?}",
            requested_theme,
            err,
            theme_manager.current().name
        );
    }

    if let Some(cursor) = &meridian_config.cursor {
        theme_manager.current_mut().config.cursor.theme = cursor.theme.clone();
        theme_manager.current_mut().config.cursor.size = cursor.size;
    }
    if meridian_config.wallpaper.is_some() {
        theme_manager.current_mut().config.wallpaper = meridian_config.wallpaper_override();
    }

    ThemeOverrideChanges {
        theme_changed: prev_theme_name != theme_manager.current().name,
        cursor_changed: prev_cursor
            != (
                theme_manager.current().config.cursor.theme.clone(),
                theme_manager.current().config.cursor.size,
            ),
        wallpaper_changed: prev_wallpaper
            != theme_manager
                .current()
                .config
                .wallpaper
                .as_ref()
                .map(|w| (w.path.clone(), w.mode)),
    }
}

/// Read the system keyboard layout settings, returning
/// `(model, layout, variant, options)`. Reads `/etc/vconsole.conf` (Arch /
/// systemd-localed) first, then `/etc/default/keyboard` (Debian). Any field that
/// cannot be found is returned empty, which lets libxkbcommon use its default.
pub(crate) fn system_xkb_settings() -> (String, String, String, String) {
    for path in ["/etc/vconsole.conf", "/etc/default/keyboard"] {
        if let Ok(contents) = std::fs::read_to_string(path) {
            let layout = xkb_value(&contents, "XKBLAYOUT");
            if !layout.is_empty() {
                return (
                    xkb_value(&contents, "XKBMODEL"),
                    layout,
                    xkb_value(&contents, "XKBVARIANT"),
                    xkb_value(&contents, "XKBOPTIONS"),
                );
            }
        }
    }
    (String::new(), String::new(), String::new(), String::new())
}

/// Extract `KEY=value` (optionally quoted) from a shell-style config file,
/// ignoring comments. Returns "" if the key is absent.
fn xkb_value(contents: &str, key: &str) -> String {
    contents
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .find_map(|l| {
            let rest = l.strip_prefix(key)?.trim_start().strip_prefix('=')?;
            Some(rest.trim().trim_matches('"').trim().to_string())
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod xkb_tests {
    use super::xkb_value;

    #[test]
    fn parses_vconsole_layout_and_options() {
        let c = "# comment\nKEYMAP=de\nXKBLAYOUT=de\nXKBMODEL=pc105\nXKBOPTIONS=terminate:ctrl_alt_bksp\n";
        assert_eq!(xkb_value(c, "XKBLAYOUT"), "de");
        assert_eq!(xkb_value(c, "XKBMODEL"), "pc105");
        assert_eq!(xkb_value(c, "XKBOPTIONS"), "terminate:ctrl_alt_bksp");
        assert_eq!(xkb_value(c, "XKBVARIANT"), "");
    }

    #[test]
    fn handles_quotes_and_ignores_comments() {
        let c = "#XKBLAYOUT=us\nXKBLAYOUT=\"de\"\n";
        assert_eq!(xkb_value(c, "XKBLAYOUT"), "de");
    }
}

/// Map a theme to light/dark by background luminance for the persisted boot-chain appearance.
pub(crate) fn theme_appearance(theme: &meridian_config::Theme) -> meridian_boot_common::Appearance {
    let bg = theme.config.colors.background;
    let lum = 0.299 * bg.r as f32 + 0.587 * bg.g as f32 + 0.114 * bg.b as f32;
    if lum > 140.0 {
        meridian_boot_common::Appearance::Light
    } else {
        meridian_boot_common::Appearance::Dark
    }
}

include!("setup/layout.rs");
include!("setup/dirty_and_construction.rs");
include!("setup/output_lifecycle.rs");

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
