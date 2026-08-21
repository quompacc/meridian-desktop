//! Thin GTK/WebKit host boundary.
//!
//! Product UI, state and policy stay outside this module. GTK owns only the
//! native process/window lifecycle required by WebKitGTK and layer-shell.

use std::{
    io::{self, BufRead},
    os::unix::io::AsRawFd,
    rc::Rc,
    time::Instant,
};

use gtk::glib::object::Cast;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use meridian_app_catalog::DesktopApp;
use meridian_tokens::{Color, Elevation, Launcher, Panel, QuickSettings};
use webkit2gtk::{
    LoadEvent, NavigationPolicyDecision, NavigationPolicyDecisionExt, PolicyDecisionExt,
    PolicyDecisionType, SettingsExt, URIRequestExt, UserContentManager, UserContentManagerExt,
    WebContext, WebContextExt, WebView, WebViewExt, WebViewExtManual,
};

use crate::bridge::{self, Command};
use crate::{
    document::{
        diagnostic_html, is_allowed_top_level_uri, panel_html, quick_settings_html, ThemeChoice,
    },
    icon_service, ipc, SurfaceChoice,
};

#[path = "gtk_host/control_state.rs"]
mod control_state;
use control_state::{canonical_appearance_state, canonical_quick_settings_state};
#[path = "gtk_host/wallpaper_picker.rs"]
mod wallpaper_picker;

pub(crate) fn run(theme: ThemeChoice, surface: SurfaceChoice, persistent_surface: bool) {
    let application_id = match surface {
        SurfaceChoice::Launcher => "org.meridian.UiRuntimeLauncher",
        SurfaceChoice::Panel => "org.meridian.UiRuntimePanel",
        SurfaceChoice::QuickSettings => "org.meridian.UiRuntimeQuickSettings",
    };
    let application = gtk::Application::new(Some(application_id), Default::default());
    application.connect_activate(move |application| {
        build_window(application, theme, surface, persistent_surface)
    });
    application.run_with_args(&["meridian-ui-runtime"]);
}

fn build_window(
    application: &gtk::Application,
    theme: ThemeChoice,
    surface: SurfaceChoice,
    persistent_surface: bool,
) {
    let launcher = Launcher::DEFAULT;
    let panel = Panel::DEFAULT;
    let quick_settings = QuickSettings::DEFAULT;
    let (title, width, height) = match surface {
        SurfaceChoice::Launcher => {
            let shadow_extent = Elevation::LAUNCHER.outer_extent();
            (
                "Meridian Launcher Preview",
                launcher.width + shadow_extent * 2,
                launcher.height + shadow_extent * 2,
            )
        }
        SurfaceChoice::Panel => (
            "Meridian Panel Preview",
            1,
            i32::try_from(panel.surface_height()).expect("panel geometry fits an i32"),
        ),
        SurfaceChoice::QuickSettings => (
            "Meridian Quick Settings Preview",
            quick_settings.width,
            quick_settings.height,
        ),
    };
    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .title(title)
        .default_width(width)
        .default_height(height)
        .decorated(false)
        .resizable(false)
        .build();
    let catalog_started = Instant::now();
    let apps = Rc::new(DesktopApp::load_system());
    eprintln!(
        "meridian-ui-runtime: catalogued {} applications in {} ms",
        apps.len(),
        catalog_started.elapsed().as_millis()
    );
    let context = WebContext::new_ephemeral();
    context.set_automation_allowed(false);
    icon_service::install(&context, &apps, window.scale_factor());
    let content_manager = UserContentManager::new();
    assert!(
        content_manager.register_script_message_handler(bridge::HANDLER_NAME),
        "bridge handler registration must succeed"
    );

    let webview = WebView::new_with_context_and_user_content_manager(&context, &content_manager);
    let [red, green, blue, alpha] = Color::TRANSPARENT.as_f32_array();
    webview.set_background_color(&gtk::gdk::RGBA::new(
        f64::from(red),
        f64::from(green),
        f64::from(blue),
        f64::from(alpha),
    ));
    let settings = WebViewExt::settings(&webview).expect("WebKit settings must exist");
    harden_settings(&settings);
    install_navigation_policy(&webview);

    let started = Instant::now();
    webview.connect_load_changed(move |_, event| {
        if event == LoadEvent::Finished {
            eprintln!(
                "meridian-ui-runtime: first {surface:?} document load finished in {} ms",
                started.elapsed().as_millis()
            );
        }
    });
    let (html, base_uri) = match surface {
        SurfaceChoice::Launcher => (diagnostic_html(theme, &apps), "meridian://diagnostic/index"),
        SurfaceChoice::Panel => (panel_html(theme, &apps), "meridian://panel/index"),
        SurfaceChoice::QuickSettings => (
            quick_settings_html(theme),
            "meridian://quick-settings/index",
        ),
    };
    webview.load_html(&html, Some(base_uri));
    install_bridge(
        &content_manager,
        &window,
        Rc::clone(&apps),
        surface,
        persistent_surface,
    );
    match surface {
        SurfaceChoice::Launcher => {
            configure_launcher_surface(&window, launcher, persistent_surface)
        }
        SurfaceChoice::Panel => configure_panel_surface(&window, panel),
        SurfaceChoice::QuickSettings => {
            configure_quick_settings_surface(&window, quick_settings, persistent_surface)
        }
    }
    window.add(&webview);
    if persistent_surface {
        if surface == SurfaceChoice::Panel {
            window.show_all();
            install_persistent_surface_control(&window, &webview, surface);
            return;
        }
        // Map exactly once. Repeatedly destroying and recreating GTK's
        // layer-surface backing store made WebKit's translucent CSS shadow
        // accumulate over several opens. A transparent, input-pass-through
        // mapped surface keeps the prewarmed document stable while hidden.
        webview.set_opacity(host_opacity(Color::TRANSPARENT.a));
        window.show_all();
        set_launcher_input(&window, false);
        install_persistent_surface_control(&window, &webview, surface);
    } else {
        window.show_all();
    }
}

fn install_persistent_surface_control(
    window: &gtk::ApplicationWindow,
    webview: &webkit2gtk::WebView,
    surface: SurfaceChoice,
) {
    let stdin = io::stdin();
    let fd = stdin.as_raw_fd();
    let mut input = io::BufReader::new(stdin);
    let window = window.downgrade();
    let webview = webview.downgrade();
    gtk::glib::source::unix_fd_add_local(
        fd,
        gtk::glib::IOCondition::IN | gtk::glib::IOCondition::HUP | gtk::glib::IOCondition::ERR,
        move |_, condition| {
            if condition.intersects(gtk::glib::IOCondition::HUP | gtk::glib::IOCondition::ERR) {
                return gtk::glib::ControlFlow::Break;
            }
            let mut command = String::new();
            match input.read_line(&mut command) {
                Ok(0) => return gtk::glib::ControlFlow::Break,
                Ok(_) => {}
                Err(error) => {
                    eprintln!("meridian-ui-runtime: launcher control read failed: {error}");
                    return gtk::glib::ControlFlow::Break;
                }
            }

            let command = command.trim_end();
            let Some(window) = window.upgrade() else {
                return gtk::glib::ControlFlow::Break;
            };
            let (command, quick_snapshot, appearance_snapshot, theme) =
                if let Some(snapshot) = command.strip_prefix("show ") {
                    ("show", Some(snapshot), None, None)
                } else if let Some(snapshot) = command.strip_prefix("update ") {
                    ("update", Some(snapshot), None, None)
                } else if let Some(snapshot) = command.strip_prefix("show-settings ") {
                    ("show-settings", None, Some(snapshot), None)
                } else if let Some(snapshot) = command.strip_prefix("appearance ") {
                    ("appearance", None, Some(snapshot), None)
                } else if let Some(theme) = command.strip_prefix("theme ") {
                    ("theme", None, None, Some(theme))
                } else {
                    (command, None, None, None)
                };
            if let Some(snapshot) = quick_snapshot {
                match (canonical_quick_settings_state(snapshot), webview.upgrade()) {
                    (Ok(snapshot), Some(webview)) => {
                        let script =
                            format!("window.meridianQuickSettings?.applyState?.({snapshot});");
                        webview.run_javascript(&script, None::<&gtk::gio::Cancellable>, |_| {});
                    }
                    (Err(error), _) => {
                        eprintln!("meridian-ui-runtime: denied Quick Settings state: {error}");
                        return gtk::glib::ControlFlow::Continue;
                    }
                    (Ok(_), None) => {}
                }
            }
            if let Some(snapshot) = appearance_snapshot {
                match (canonical_appearance_state(snapshot), webview.upgrade()) {
                    (Ok(snapshot), Some(webview)) => {
                        let script =
                            format!("window.meridianLauncher?.applyAppearanceState?.({snapshot});");
                        webview.run_javascript(&script, None::<&gtk::gio::Cancellable>, |_| {});
                    }
                    (Err(error), _) => {
                        eprintln!("meridian-ui-runtime: denied appearance state: {error}");
                        return gtk::glib::ControlFlow::Continue;
                    }
                    (Ok(_), None) => {}
                }
            }
            if let Some(theme) = theme {
                if !matches!(theme, "dark" | "light") {
                    eprintln!("meridian-ui-runtime: denied theme value {theme:?}");
                    return gtk::glib::ControlFlow::Continue;
                }
                if let Some(webview) = webview.upgrade() {
                    // A document-root color-scheme flip clears WebKit's
                    // transparent backing store for one frame. The always
                    // visible panel changes only its scoped color tokens.
                    let target = if surface == SurfaceChoice::Panel {
                        "document.querySelector('.meridian-theme-scope')"
                    } else {
                        "document.documentElement"
                    };
                    let script = format!("{target}.dataset.meridianTheme = {theme:?};");
                    webview.run_javascript(&script, None::<&gtk::gio::Cancellable>, |_| {});
                }
            }
            match command {
                "show" => {
                    set_launcher_input(&window, true);
                    if let Some(webview) = webview.upgrade() {
                        webview.run_javascript(
                            "window.meridianLauncher?.showApps?.();",
                            None::<&gtk::gio::Cancellable>,
                            |_| {},
                        );
                        webview.set_opacity(host_opacity(u8::MAX));
                    }
                    window.present();
                }
                "show-settings" => {
                    set_launcher_input(&window, true);
                    if let Some(webview) = webview.upgrade() {
                        webview.run_javascript(
                            "window.meridianLauncher?.showSettings?.(true);",
                            None::<&gtk::gio::Cancellable>,
                            |_| {},
                        );
                        webview.set_opacity(host_opacity(u8::MAX));
                    }
                    window.present();
                }
                "hide" => {
                    if let Some(webview) = webview.upgrade() {
                        webview.set_opacity(host_opacity(Color::TRANSPARENT.a));
                        webview.run_javascript(
                            "document.activeElement?.blur();",
                            None::<&gtk::gio::Cancellable>,
                            |_| {},
                        );
                    }
                    set_launcher_input(&window, false);
                }
                "update" => {}
                "appearance" | "theme" => {}
                _ => eprintln!("meridian-ui-runtime: ignored launcher control {command:?}"),
            }
            gtk::glib::ControlFlow::Continue
        },
    );
}

fn host_opacity(alpha: u8) -> f64 {
    f64::from(alpha) / f64::from(u8::MAX)
}

fn set_launcher_input(window: &gtk::ApplicationWindow, enabled: bool) {
    window.set_keyboard_mode(if enabled {
        KeyboardMode::Exclusive
    } else {
        KeyboardMode::None
    });
    if enabled {
        window.input_shape_combine_region(None);
    } else {
        // GDK pass-through alone is not reliably committed by WebKitGTK's
        // layer-shell toplevel. An explicit empty Wayland input region keeps
        // the prewarmed transparent surface mapped without intercepting apps.
        let empty_region = gtk::cairo::Region::create();
        window.input_shape_combine_region(Some(&empty_region));
    }
    if let Some(surface) = window.window() {
        surface.set_pass_through(!enabled);
    }
}

fn configure_panel_surface(window: &gtk::ApplicationWindow, panel: Panel) {
    configure_transparent_surface(window);
    // WebKitGTK reports a 200 px natural height before its document is laid
    // out. Override that host requisition so layer-shell receives the exact
    // token-owned panel surface height instead of positioning a 200 px layer.
    window.set_size_request(
        1,
        i32::try_from(panel.surface_height()).expect("panel geometry fits an i32"),
    );
    window.init_layer_shell();
    window.set_namespace("meridian-panel-web");
    window.set_layer(Layer::Top);
    window.set_anchor(Edge::Bottom, true);
    window.set_anchor(Edge::Left, true);
    window.set_anchor(Edge::Right, true);
    // The shell owns a bufferless reservation surface. Ignoring exclusive
    // zones places this visible surface at the physical edge without moving
    // application work areas.
    window.set_exclusive_zone(-1);
    window.set_keyboard_mode(KeyboardMode::None);
}

fn configure_quick_settings_surface(
    window: &gtk::ApplicationWindow,
    quick_settings: QuickSettings,
    persistent: bool,
) {
    configure_transparent_surface(window);
    window.set_size_request(quick_settings.width, quick_settings.height);
    window.init_layer_shell();
    window.set_namespace("meridian-quick-settings");
    window.set_layer(Layer::Top);
    window.set_anchor(Edge::Bottom, true);
    window.set_anchor(Edge::Right, true);
    window.set_exclusive_zone(0);
    window.set_layer_shell_margin(Edge::Bottom, quick_settings.panel_gap);
    window.set_layer_shell_margin(
        Edge::Right,
        i32::try_from(Panel::DEFAULT.side_margin).expect("panel geometry fits an i32"),
    );
    window.set_keyboard_mode(if persistent {
        KeyboardMode::None
    } else {
        KeyboardMode::Exclusive
    });
}

fn install_bridge(
    content_manager: &UserContentManager,
    window: &gtk::ApplicationWindow,
    apps: Rc<Vec<DesktopApp>>,
    surface: SurfaceChoice,
    persistent_surface: bool,
) {
    let window = window.downgrade();
    content_manager.connect_script_message_received(
        Some(bridge::HANDLER_NAME),
        move |_, result| {
            let Some(raw) = result.js_value().map(|value| value.to_string()) else {
                eprintln!("meridian-ui-runtime: denied bridge message without a value");
                return;
            };
            match bridge::decode(&raw) {
                Ok(Command::CloseLauncher) => {
                    if surface == SurfaceChoice::Launcher {
                        if persistent_surface {
                            if let Err(error) = ipc::toggle_launcher() {
                                eprintln!(
                                    "meridian-ui-runtime: failed to hide launcher: {error}"
                                );
                            }
                        } else if let Some(window) = window.upgrade() {
                            window.close();
                        }
                    }
                }
                Ok(Command::ToggleLauncher) => {
                    if let Err(error) = ipc::toggle_launcher() {
                        eprintln!("meridian-ui-runtime: failed to toggle launcher: {error}");
                    }
                }
                Ok(Command::ToggleQuickSettings | Command::CloseQuickSettings) => {
                    if let Err(error) = ipc::toggle_quick_settings() {
                        eprintln!("meridian-ui-runtime: failed to toggle Quick Settings: {error}");
                    }
                }
                Ok(Command::OpenSystemSettings) => {
                    if let Err(error) = ipc::open_system_settings() {
                        eprintln!(
                            "meridian-ui-runtime: failed to open System Settings: {error}"
                        );
                    }
                }
                Ok(Command::RefreshAppearance) => {
                    if let Err(error) = ipc::refresh_appearance() {
                        eprintln!("meridian-ui-runtime: failed to refresh appearance: {error}");
                    }
                }
                Ok(Command::SetAppearanceTheme { theme }) => {
                    if let Err(error) = ipc::set_appearance_theme(theme) {
                        eprintln!("meridian-ui-runtime: failed to set appearance theme: {error}");
                    }
                }
                Ok(Command::PickAppearanceWallpaper) => {
                    if let Some(window) = window.upgrade() {
                        wallpaper_picker::open(&window);
                    }
                }
                Ok(Command::SetAppearanceWallpaperMode { mode }) => {
                    if let Err(error) = ipc::set_appearance_wallpaper_mode(mode) {
                        eprintln!(
                            "meridian-ui-runtime: failed to set wallpaper mode: {error}"
                        );
                    }
                }
                Ok(Command::RefreshNetwork) => {
                    if let Err(error) = ipc::refresh_network() {
                        eprintln!("meridian-ui-runtime: failed to refresh network: {error}");
                    }
                }
                Ok(Command::ConnectNetwork { ssid, password }) => {
                    if let Err(error) = ipc::connect_network(ssid, password) {
                        eprintln!("meridian-ui-runtime: failed to connect network: {error}");
                    }
                }
                Ok(Command::DisconnectNetwork) => {
                    if let Err(error) = ipc::disconnect_network() {
                        eprintln!("meridian-ui-runtime: failed to disconnect network: {error}");
                    }
                }
                Ok(Command::SetAudioVolume { percent }) => {
                    if let Err(error) = ipc::set_audio_volume(percent) {
                        eprintln!("meridian-ui-runtime: failed to set audio volume: {error}");
                    }
                }
                Ok(Command::ToggleAudioMute) => {
                    if let Err(error) = ipc::toggle_audio_mute() {
                        eprintln!("meridian-ui-runtime: failed to toggle audio mute: {error}");
                    }
                }
                Ok(Command::SetPowerProfile { profile }) => {
                    if let Err(error) = ipc::set_power_profile(profile) {
                        eprintln!("meridian-ui-runtime: failed to set power profile: {error}");
                    }
                }
                Ok(Command::LaunchApp { desktop_id }) => {
                    let Some(app) = apps.iter().find(|app| app.desktop_id == desktop_id) else {
                        eprintln!(
                            "meridian-ui-runtime: denied launch for unknown desktop id {desktop_id:?}"
                        );
                        return;
                    };
                    match ipc::launch(app) {
                        Ok(()) if surface == SurfaceChoice::Launcher => {
                            if persistent_surface {
                                if let Err(error) = ipc::toggle_launcher() {
                                    eprintln!(
                                        "meridian-ui-runtime: failed to hide launcher after app launch: {error}"
                                    );
                                }
                            } else if let Some(window) = window.upgrade() {
                                window.close();
                            }
                        }
                        Ok(()) => {}
                        Err(error) => eprintln!(
                            "meridian-ui-runtime: failed to launch {:?}: {error}",
                            app.name
                        ),
                    }
                }
                Err(error) => eprintln!("meridian-ui-runtime: denied bridge message: {error}"),
            }
        },
    );
}

fn configure_launcher_surface(
    window: &gtk::ApplicationWindow,
    launcher: Launcher,
    persistent: bool,
) {
    configure_transparent_surface(window);

    let panel = Panel::DEFAULT;
    let shadow_extent = Elevation::LAUNCHER.outer_extent();
    let left_margin =
        i32::try_from(panel.side_margin).expect("panel geometry fits an i32") - shadow_extent;

    window.init_layer_shell();
    window.set_namespace("meridian-launcher");
    window.set_layer(Layer::Top);
    window.set_anchor(Edge::Bottom, true);
    window.set_anchor(Edge::Left, true);
    window.set_exclusive_zone(0);
    // The compositor already applies the panel's exclusive zone to this Top
    // layer surface; adding its height here would reserve it twice.
    window.set_layer_shell_margin(Edge::Bottom, launcher.panel_gap - shadow_extent);
    window.set_layer_shell_margin(Edge::Left, left_margin);
    window.set_keyboard_mode(if persistent {
        KeyboardMode::None
    } else {
        KeyboardMode::Exclusive
    });
}

fn configure_transparent_surface(window: &gtk::ApplicationWindow) {
    window.set_app_paintable(true);
    install_transparent_host_style(window);
    if let Some(visual) =
        gtk::prelude::WidgetExt::screen(window).and_then(|screen| screen.rgba_visual())
    {
        window.set_visual(Some(&visual));
    }
}

fn install_transparent_host_style(window: &gtk::ApplicationWindow) {
    // Host-only transparency: product colours remain owned by generated CSS.
    let provider = gtk::CssProvider::new();
    provider
        .load_from_data(b"window { background-color: transparent; }")
        .expect("static GTK host CSS must parse");
    window
        .style_context()
        .add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
}

fn harden_settings(settings: &webkit2gtk::Settings) {
    settings.set_enable_javascript(true);
    settings.set_enable_developer_extras(false);
    settings.set_enable_dns_prefetching(false);
    settings.set_enable_html5_database(false);
    settings.set_enable_html5_local_storage(false);
    settings.set_enable_media_stream(false);
    settings.set_javascript_can_access_clipboard(false);
    settings.set_javascript_can_open_windows_automatically(false);
}

fn install_navigation_policy(webview: &WebView) {
    webview.connect_decide_policy(|_, decision, decision_type| {
        if decision_type == PolicyDecisionType::NewWindowAction {
            decision.ignore();
            return true;
        }
        if decision_type != PolicyDecisionType::NavigationAction {
            return false;
        }
        let Some(navigation) = decision.dynamic_cast_ref::<NavigationPolicyDecision>() else {
            decision.ignore();
            return true;
        };
        let uri = navigation
            .navigation_action()
            .and_then(|action| action.request())
            .and_then(|request| request.uri());
        if uri
            .as_deref()
            .is_some_and(|uri| is_allowed_top_level_uri(uri))
        {
            return false;
        }
        eprintln!("meridian-ui-runtime: denied top-level navigation to {uri:?}");
        decision.ignore();
        true
    });
}
