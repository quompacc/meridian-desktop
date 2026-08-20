//! Thin GTK/WebKit host boundary.
//!
//! Product UI, state and policy stay outside this module. GTK owns only the
//! native process/window lifecycle required by WebKitGTK and layer-shell.

use std::{rc::Rc, time::Instant};

use gtk::glib::object::Cast;
use gtk::prelude::*;
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use meridian_app_catalog::DesktopApp;
use meridian_tokens::{Color, Elevation, Launcher, Panel};
use webkit2gtk::{
    LoadEvent, NavigationPolicyDecision, NavigationPolicyDecisionExt, PolicyDecisionExt,
    PolicyDecisionType, SettingsExt, URIRequestExt, UserContentManager, UserContentManagerExt,
    WebContext, WebContextExt, WebView, WebViewExt, WebViewExtManual,
};

use crate::bridge::{self, Command};
use crate::{
    document::{diagnostic_html, is_allowed_top_level_uri, panel_html, ThemeChoice},
    icon_service, ipc, SurfaceChoice,
};

pub(crate) fn run(theme: ThemeChoice, surface: SurfaceChoice) {
    let application_id = match surface {
        SurfaceChoice::Launcher => "org.meridian.UiRuntimeLauncher",
        SurfaceChoice::Panel => "org.meridian.UiRuntimePanel",
    };
    let application = gtk::Application::new(Some(application_id), Default::default());
    application.connect_activate(move |application| build_window(application, theme, surface));
    application.run_with_args(&["meridian-ui-runtime"]);
}

fn build_window(application: &gtk::Application, theme: ThemeChoice, surface: SurfaceChoice) {
    let launcher = Launcher::DEFAULT;
    let panel = Panel::DEFAULT;
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
    };
    webview.load_html(&html, Some(base_uri));
    install_bridge(&content_manager, &window, Rc::clone(&apps), surface);
    match surface {
        SurfaceChoice::Launcher => configure_launcher_surface(&window, launcher),
        SurfaceChoice::Panel => configure_panel_surface(&window, panel),
    }
    window.add(&webview);
    window.show_all();
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

fn install_bridge(
    content_manager: &UserContentManager,
    window: &gtk::ApplicationWindow,
    apps: Rc<Vec<DesktopApp>>,
    surface: SurfaceChoice,
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
                        if let Some(window) = window.upgrade() {
                            window.close();
                        }
                    }
                }
                Ok(Command::ToggleLauncher) => {
                    if let Err(error) = ipc::toggle_launcher() {
                        eprintln!("meridian-ui-runtime: failed to toggle launcher: {error}");
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
                            if let Some(window) = window.upgrade() {
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

fn configure_launcher_surface(window: &gtk::ApplicationWindow, launcher: Launcher) {
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
    window.set_keyboard_mode(KeyboardMode::Exclusive);
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
