use std::{cell::RefCell, collections::HashSet, time::Instant};

use meridian_config::{MeridianConfig, ThemeManager};
use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    reexports::{calloop::EventLoop, calloop_wayland_source::WaylandSource},
    registry::RegistryState,
    seat::SeatState,
    shell::{
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm},
};
use wayland_protocols::ext::{
    image_capture_source::v1::client::ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
    image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1,
};
use tracing::{debug, info, warn};
use wayland_client::{globals::registry_queue_init, Connection, QueueHandle};

use crate::{
    default_pinned_apps, icons::IconCache, launcher, network::NetworkController, panel,
    panel::PinnedApp,
    TextRenderer, CALENDAR_POPUP_HEIGHT, CALENDAR_POPUP_WIDTH, LAUNCHER_HEIGHT, LAUNCHER_WIDTH,
    NETWORK_POPUP_HEIGHT, NETWORK_POPUP_RIGHT_MARGIN, NETWORK_POPUP_WIDTH, PANEL_HEIGHT,
    SHELL_POPUP_BOTTOM_MARGIN, WORKSPACE_POPUP_HEIGHT, WORKSPACE_POPUP_WIDTH,
};

use super::{
    calendar::CalendarDisplayPolicy, CommitReason, CommitStats, CommitSurfaceKind, IpcClient,
    MeridianShell, SurfaceKind,
};

pub(crate) fn initialize(
    event_loop: &mut EventLoop<'_, MeridianShell>,
) -> Result<(MeridianShell, QueueHandle<MeridianShell>), Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    info!("Connected to Wayland display");
    let (globals, event_queue) = registry_queue_init(&conn)?;
    info!("Registry initialized");
    let qh = event_queue.handle();
    WaylandSource::new(conn.clone(), event_queue).insert(event_loop.handle())?;

    let screencopy_manager = globals
        .bind::<ExtImageCopyCaptureManagerV1, _, _>(&qh, 1..=1, ())
        .ok();
    let capture_source_manager = globals
        .bind::<ExtOutputImageCaptureSourceManagerV1, _, _>(&qh, 1..=1, ())
        .ok();

    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor is not available");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("wlr layer shell is not available");
    info!("Layer shell protocol bound");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm is not available");

    let panel_surface = compositor.create_surface(&qh);
    let panel = layer_shell.create_layer_surface(
        &qh,
        panel_surface,
        Layer::Top,
        Some("meridian-panel"),
        None,
    );
    panel.set_anchor(Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    panel.set_size(0, PANEL_HEIGHT);
    panel.set_exclusive_zone(PANEL_HEIGHT as i32);
    panel.set_keyboard_interactivity(KeyboardInteractivity::None);
    info!("Panel surface created");

    let launcher_surface = compositor.create_surface(&qh);
    let launcher_layer = layer_shell.create_layer_surface(
        &qh,
        launcher_surface,
        Layer::Overlay,
        Some("meridian-launcher"),
        None,
    );
    launcher_layer.set_anchor(Anchor::BOTTOM | Anchor::LEFT);
    launcher_layer.set_margin(0, 0, SHELL_POPUP_BOTTOM_MARGIN, 8);
    launcher_layer.set_size(LAUNCHER_WIDTH, LAUNCHER_HEIGHT);
    launcher_layer.set_exclusive_zone(0);
    launcher_layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
    debug!(
        "Launcher surface created: namespace=meridian-launcher layer=Overlay anchor=Bottom|Left size={}x{} margin_bottom={} margin_left=8 exclusive_zone=0 keyboard_interactivity=Exclusive",
        LAUNCHER_WIDTH,
        LAUNCHER_HEIGHT,
        SHELL_POPUP_BOTTOM_MARGIN
    );

    let calendar_surface = compositor.create_surface(&qh);
    // Reuse the launcher namespace bucket so popup stacking matches launcher behavior.
    let calendar_layer = layer_shell.create_layer_surface(
        &qh,
        calendar_surface,
        Layer::Overlay,
        Some("meridian-launcher"),
        None,
    );
    calendar_layer.set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
    calendar_layer.set_margin(0, 12, SHELL_POPUP_BOTTOM_MARGIN, 0);
    calendar_layer.set_size(CALENDAR_POPUP_WIDTH, CALENDAR_POPUP_HEIGHT);
    calendar_layer.set_exclusive_zone(0);
    calendar_layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    debug!(
        "Calendar popup surface created: namespace=meridian-launcher layer=Overlay anchor=Bottom|Right size={}x{} margin_bottom={} margin_right=12 exclusive_zone=0 keyboard_interactivity=OnDemand",
        CALENDAR_POPUP_WIDTH,
        CALENDAR_POPUP_HEIGHT,
        SHELL_POPUP_BOTTOM_MARGIN
    );

    let workspace_surface = compositor.create_surface(&qh);
    let workspace_layer = layer_shell.create_layer_surface(
        &qh,
        workspace_surface,
        Layer::Overlay,
        Some("meridian-workspace-popup"),
        None,
    );
    workspace_layer.set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
    workspace_layer.set_margin(0, 160, SHELL_POPUP_BOTTOM_MARGIN, 0);
    workspace_layer.set_size(WORKSPACE_POPUP_WIDTH, WORKSPACE_POPUP_HEIGHT);
    workspace_layer.set_exclusive_zone(0);
    workspace_layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    debug!(
        "Workspace popup surface created: namespace=meridian-workspace-popup layer=Overlay anchor=Bottom|Right size={}x{} margin_bottom={} margin_right=160 exclusive_zone=0 keyboard_interactivity=OnDemand",
        WORKSPACE_POPUP_WIDTH,
        WORKSPACE_POPUP_HEIGHT,
        SHELL_POPUP_BOTTOM_MARGIN
    );

    let network_surface = compositor.create_surface(&qh);
    let network_layer = layer_shell.create_layer_surface(
        &qh,
        network_surface,
        Layer::Overlay,
        Some("meridian-network-popup"),
        None,
    );
    network_layer.set_anchor(Anchor::BOTTOM | Anchor::RIGHT);
    network_layer.set_margin(0, NETWORK_POPUP_RIGHT_MARGIN, SHELL_POPUP_BOTTOM_MARGIN, 0);
    network_layer.set_size(NETWORK_POPUP_WIDTH, NETWORK_POPUP_HEIGHT);
    network_layer.set_exclusive_zone(0);
    network_layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    debug!(
        "Network popup surface created: namespace=meridian-network-popup layer=Overlay anchor=Bottom|Right size={}x{} margin_bottom={} margin_right={} exclusive_zone=0 keyboard_interactivity=OnDemand",
        NETWORK_POPUP_WIDTH,
        NETWORK_POPUP_HEIGHT,
        SHELL_POPUP_BOTTOM_MARGIN,
        NETWORK_POPUP_RIGHT_MARGIN
    );

    // Phase A1.3: notification popup, anchored top-right, no keyboard
    // input (purely informational). Stays unmapped (1x1 commit) when
    // the notification queue is empty.
    let notification_surface = compositor.create_surface(&qh);
    let notification_layer = layer_shell.create_layer_surface(
        &qh,
        notification_surface,
        Layer::Overlay,
        Some("meridian-notification"),
        None,
    );
    notification_layer.set_anchor(Anchor::TOP | Anchor::RIGHT);
    notification_layer.set_margin(
        crate::NOTIFICATION_TOP_MARGIN,
        crate::NOTIFICATION_RIGHT_MARGIN,
        0,
        0,
    );
    notification_layer.set_size(crate::NOTIFICATION_WIDTH, crate::NOTIFICATION_HEIGHT);
    notification_layer.set_exclusive_zone(0);
    notification_layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    debug!(
        "Notification surface created: namespace=meridian-notification layer=Overlay anchor=Top|Right size={}x{} margin_top={} margin_right={} exclusive_zone=0 keyboard_interactivity=None",
        crate::NOTIFICATION_WIDTH,
        crate::NOTIFICATION_HEIGHT,
        crate::NOTIFICATION_TOP_MARGIN,
        crate::NOTIFICATION_RIGHT_MARGIN
    );

    let meridian_config = MeridianConfig::load();
    let mut theme_manager = ThemeManager::new();
    if !meridian_config.general.theme.trim().is_empty()
        && meridian_config.general.theme != theme_manager.current().name
    {
        if let Err(err) = theme_manager.set_theme(&meridian_config.general.theme) {
            warn!(
                "Failed to load theme {:?} from config: {} — using current theme {:?}",
                meridian_config.general.theme,
                err,
                theme_manager.current().name
            );
        }
    }
    let available_themes = theme_manager.available_themes();
    let theme = theme_manager.current().config.clone();
    info!("Theme loaded");

    if let Err(err) = conn.flush() {
        warn!("Failed to flush Wayland connection: {}", err);
    }
    info!("Wayland connection flushed, entering event loop");

    let font = TextRenderer::new(&theme.fonts.ui, 13);
    let pool = SlotPool::new(1024 * 1024 * 4, &shm)?;
    let mut icon_cache = IconCache::new();
    // Panel pinned-app icons at 22px. Includes both chromium (the new
    // default Web entry) and firefox so users with the older custom
    // config still get an icon. Without warming, IconCache::lookup
    // returns None even if the file exists on disk.
    icon_cache.warm(
        &[
            "utilities-terminal",
            "chromium",
            "firefox",
            "org.kde.dolphin",
        ],
        22,
    );
    icon_cache.warm(
        &[
            "network-wired-symbolic",
            "network-wired-disconnected-symbolic",
            "network-wireless-signal-excellent-symbolic",
            "network-wireless-signal-good-symbolic",
            "network-wireless-signal-none-symbolic",
            "network-wireless-disconnected-symbolic",
            "network-vpn-symbolic",
            "network-offline-symbolic",
            "camera-photo-symbolic",
        ],
        22,
    );
    icon_cache.warm(
        &[
            "thunderbird",
            "chromium",
            "system-file-manager",
            "gwenview",
            "amarok",
            "marble",
            "akregator",
            "org.kde.discover",
            "org.kde.korganizer",
            "org.kde.kweather",
            "org.kde.knotes",
        ],
        64,
    );
    icon_cache.warm(
        &[
            "system-shutdown",
            "system-reboot",
            "system-suspend",
            "system-lock-screen",
            "system-log-out",
        ],
        32,
    );
    let launcher_apps = launcher::DesktopApp::load_system();
    let mut seen_icons = HashSet::new();
    let mut launcher_icons = Vec::new();
    for app in &launcher_apps {
        if let Some(icon_name) = app.icon_name.as_deref() {
            if !icon_name.is_empty() && seen_icons.insert(icon_name.to_string()) {
                launcher_icons.push(icon_name.to_string());
            }
        }
    }
    if !launcher_icons.is_empty() {
        let icon_refs = launcher_icons
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        icon_cache.warm(&icon_refs, 24);
        icon_cache.warm(&icon_refs, 96);
        icon_cache.warm(&icon_refs, 192);
    }
    let mut network_controller = NetworkController::new();
    network_controller.poll();

    let commit_stats_enabled = std::env::var("MERIDIAN_SHELL_COMMIT_STATS")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);
    let render_stats_enabled = std::env::var("MERIDIAN_SHELL_RENDER_STATS")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);

    let mut shell = MeridianShell {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        panel,
        launcher_layer,
        calendar_layer,
        workspace_layer,
        network_layer,
        notification_layer,
        panel_configured: false,
        launcher_configured: false,
        calendar_configured: false,
        workspace_configured: false,
        network_configured: false,
        notification_configured: false,
        panel_buffer: None,
        launcher_buffer: None,
        calendar_buffer: None,
        workspace_buffer: None,
        network_buffer: None,
        notification_buffer: None,
        pool,
        width: 1024,
        launcher_width: LAUNCHER_WIDTH,
        launcher_height: LAUNCHER_HEIGHT,
        launcher_is_fullscreen: false,
        launcher_visual_x: 8,
        launcher_visual_y: 0,
        calendar_width: CALENDAR_POPUP_WIDTH,
        calendar_height: CALENDAR_POPUP_HEIGHT,
        workspace_width: WORKSPACE_POPUP_WIDTH,
        workspace_height: WORKSPACE_POPUP_HEIGHT,
        network_width: NETWORK_POPUP_WIDTH,
        network_height: NETWORK_POPUP_HEIGHT,
        notification_width: crate::NOTIFICATION_WIDTH,
        notification_height: crate::NOTIFICATION_HEIGHT,
        notifications: std::collections::VecDeque::new(),
        notification_dirty: false,
        settings_category: crate::settings_view::SettingsCategory::default(),
        keyboard: None,
        keyboard_focus: SurfaceKind::None,
        pointer: None,
        pointer_position: (0.0, 0.0),
        pointer_surface: SurfaceKind::None,
        available_themes,
        theme_name: theme_manager.current().name.clone(),
        available_wallpapers: meridian_config::MeridianConfig::scan_wallpaper_dirs(),
        wallpaper_thumbnails: Vec::new(),
        wallpaper_picker_rx: None,
        wallpaper_path: meridian_config.wallpaper.as_ref().map(|w| w.path.clone()),
        wallpaper_mode: meridian_config.wallpaper.as_ref().map(|w| w.mode).unwrap_or_default(),
        theme,
        font: RefCell::new(font),
        icon_cache,
        network_controller,
        ipc: IpcClient::connect(),
        panel_state: panel::PanelState::new(),
        pinned_apps: if meridian_config.panel.pinned.is_empty() {
            default_pinned_apps()
        } else {
            meridian_config
                .panel
                .pinned
                .iter()
                .map(|app| PinnedApp {
                    label: app.label.clone(),
                    program: app.program.clone(),
                    args: vec![],
                    terminal: false,
                    icon_name: app.icon.clone(),
                })
                .collect()
        },
        launcher_state: launcher::LauncherState::new_with_apps(launcher_apps),
        workspace_state: crate::workspaces::WorkspacePopupState::new(),
        focused_window_id: None,
        focused_title: None,
        windows: Vec::new(),
        active_workspace: 1,
        focused_output_id: None,
        output_workspaces: Vec::new(),
        output_workspace_state_available: false,
        workspace_window_counts: [0; 9],
        occupied_workspaces: [false; 9],
        occupied_state_available: false,
        workspace_state_received: false,
        workspace_indicator_dirty: true,
        workspace_ipc_unavailable_logged: false,
        occupied_unavailable_logged: false,
        panel_dirty: true,
        launcher_dirty: true,
        ui_preview_widget_state: None,
        panel_widget_state: None,
        app_view_open: false,
        launcher_settings_open: false,
        app_view_category: Default::default(),
        context_menu: None,
        search_query: String::new(),
        calendar_dirty: true,
        workspace_dirty: true,
        network_dirty: true,
        calendar_popup_open: false,
        workspace_popup_open: false,
        network_popup_open: false,
        calendar_display_policy: CalendarDisplayPolicy::default(),
        panel_last_signature: None,
        repaint_stats: Default::default(),
        repaint_stats_enabled: std::env::var("MERIDIAN_SHELL_REPAINT_STATS")
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false),
        last_repaint_stats_log: Instant::now(),
        commit_stats: CommitStats::default(),
        commit_stats_enabled,
        last_commit_stats_log: Instant::now(),
        render_stats: Default::default(),
        render_stats_enabled,
        last_render_stats_log: Instant::now(),
        commit_info_until: Instant::now()
            + if commit_stats_enabled {
                std::time::Duration::from_secs(5)
            } else {
                std::time::Duration::ZERO
            },
        last_clock: String::new(),
        last_tick: Instant::now(),
        exit: false,
        screencopy_manager,
        capture_source_manager,
        screenshot_capture: None,
    };

    shell.commit_surface(CommitSurfaceKind::Panel, CommitReason::InitialCreate);
    info!("Panel surface created and committed");
    shell.commit_surface(CommitSurfaceKind::Launcher, CommitReason::InitialCreate);
    info!("Launcher surface created and committed");
    shell.calendar_layer.commit();
    info!("Calendar popup surface created and committed");
    shell.workspace_layer.commit();
    info!("Workspace popup surface created and committed");
    shell.network_layer.commit();
    info!("Network popup surface created and committed");
    shell.notification_layer.commit();
    info!("Notification surface created and committed");
    Ok((shell, qh))
}
