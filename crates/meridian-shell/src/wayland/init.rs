use std::{cell::RefCell, time::Instant};

use meridian_config::{MeridianConfig, ThemeManager};
use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    reexports::{calloop::EventLoop, calloop_wayland_source::WaylandSource},
    registry::RegistryState,
    seat::SeatState,
    shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
    shm::{slot::SlotPool, Shm},
};
use tracing::{debug, info, warn};
use wayland_client::{globals::registry_queue_init, Connection, QueueHandle};

use crate::{launcher, panel, TextRenderer, LAUNCHER_HEIGHT, LAUNCHER_WIDTH, PANEL_HEIGHT};

use super::{CommitReason, CommitStats, CommitSurfaceKind, IpcClient, MeridianShell, SurfaceKind};

pub(crate) fn initialize(
    event_loop: &mut EventLoop<'_, MeridianShell>,
) -> Result<(MeridianShell, QueueHandle<MeridianShell>), Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    info!("Connected to Wayland display");
    let (globals, event_queue) = registry_queue_init(&conn)?;
    info!("Registry initialized");
    let qh = event_queue.handle();
    WaylandSource::new(conn.clone(), event_queue).insert(event_loop.handle())?;

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
    launcher_layer.set_margin(0, 0, PANEL_HEIGHT as i32, 8);
    launcher_layer.set_size(LAUNCHER_WIDTH, LAUNCHER_HEIGHT);
    launcher_layer.set_exclusive_zone(0);
    launcher_layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
    debug!(
        "Launcher surface created: namespace=meridian-launcher layer=Overlay anchor=Bottom|Left size={}x{} margin_bottom={} margin_left=8 exclusive_zone=0 keyboard_interactivity=Exclusive",
        LAUNCHER_WIDTH,
        LAUNCHER_HEIGHT,
        PANEL_HEIGHT
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
    let theme = theme_manager.current().config.clone();
    info!("Theme loaded");

    if let Err(err) = conn.flush() {
        warn!("Failed to flush Wayland connection: {}", err);
    }
    info!("Wayland connection flushed, entering event loop");

    let font = TextRenderer::new(&theme.fonts.ui, 13);
    let pool = SlotPool::new(1024 * 1024 * 4, &shm)?;

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
        panel_configured: false,
        launcher_configured: false,
        panel_buffer: None,
        launcher_buffer: None,
        pool,
        width: 1024,
        launcher_width: LAUNCHER_WIDTH,
        launcher_height: LAUNCHER_HEIGHT,
        keyboard: None,
        keyboard_focus: SurfaceKind::None,
        pointer: None,
        pointer_position: (0.0, 0.0),
        pointer_surface: SurfaceKind::None,
        theme_name: theme_manager.current().name.clone(),
        theme,
        font: RefCell::new(font),
        ipc: IpcClient::connect(),
        panel_state: panel::PanelState::new(),
        launcher_state: launcher::LauncherState::new(),
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
        panel_last_signature: None,
        launcher_last_signature: None,
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
    };

    shell.commit_surface(CommitSurfaceKind::Panel, CommitReason::InitialCreate);
    info!("Panel surface created and committed");
    shell.commit_surface(CommitSurfaceKind::Launcher, CommitReason::InitialCreate);
    info!("Launcher surface created and committed");

    Ok((shell, qh))
}
