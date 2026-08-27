#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_settings_launcher(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    selected: SettingsCategory,
    search: &str,
    available_themes: &[String],
    current_theme: &str,
    available_wallpapers: &[WallpaperEntry],
    wallpaper_thumbnails: &[Option<(u32, u32, Vec<u8>)>],
    current_wallpaper: Option<&str>,
    wallpaper_mode: WallpaperMode,
    cursor_size: u32,
    available_cursor_themes: &[String],
    current_cursor_theme: &str,
    idle_timeout_secs: Option<u64>,
    pinned_apps: &[PinnedApp],
    output_workspaces: &[OutputWorkspaceState],
    display_mode_dropdown_open: Option<usize>,
    printer_snapshot: &PrinterSnapshot,
    audio_snapshot: &AudioSnapshot,
    system_info: &SystemInfo,
    network_state: &NetworkState,
    network_profiles: &[ConnectionProfile],
    bluetooth_snapshot: &BluetoothSnapshot,
    wifi_networks: &[WifiNetwork],
    pinned_adding: bool,
    all_apps: &[DesktopApp],
    icon_cache: &IconCache,
    _armed_power: Option<(&str, f32)>,
    default_apps_index: Option<&crate::default_apps::MimeAppIndex>,
    default_apps_current: &std::collections::HashMap<
        crate::default_apps::DefaultAppCategory,
        String,
    >,
    default_apps_picker_open: Option<crate::default_apps::DefaultAppCategory>,
    theme_config: &ThemeConfig,
    state_fn: &dyn Fn(&[usize]) -> WidgetState,
) {
    let expected = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if canvas.len() != expected {
        return;
    }
    let Some(mut pixmap) = Pixmap::new(width, height) else {
        return;
    };
    let theme = settings_theme_from_config(theme_config);
    pixmap.fill(tiny_skia::Color::from_rgba8(
        theme.palette.surface.r,
        theme.palette.surface.g,
        theme.palette.surface.b,
        theme.palette.surface.a,
    ));
    let root = build_settings_widget_tree(
        width,
        height,
        selected,
        search,
        available_themes,
        current_theme,
        available_wallpapers,
        wallpaper_thumbnails,
        current_wallpaper,
        wallpaper_mode,
        cursor_size,
        available_cursor_themes,
        current_cursor_theme,
        idle_timeout_secs,
        pinned_apps,
        output_workspaces,
        display_mode_dropdown_open,
        printer_snapshot,
        audio_snapshot,
        system_info,
        network_state,
        network_profiles,
        bluetooth_snapshot,
        wifi_networks,
        pinned_adding,
        all_apps,
        icon_cache,
        None,
        default_apps_index,
        default_apps_current,
        default_apps_picker_open,
        &theme,
    );
    if let Ok(layout) =
        meridian_ui::compute_layout(&*root, meridian_ui::PixelSize { width, height })
    {
        let mut pm = pixmap.as_mut();
        let _ = meridian_ui::render(&*root, &layout, &mut pm, &theme, state_fn);
    }
    blit_rgba_to_argb(pixmap.data(), canvas);
}

fn printer_service_message(service: PrinterServiceState) -> &'static str {
    match service {
        PrinterServiceState::Running => "",
        PrinterServiceState::Stopped => "CUPS scheduler is not running",
        PrinterServiceState::Unavailable => "lpstat is not available",
    }
}

fn blit_rgba_to_argb(src: &[u8], dst: &mut [u8]) {
    if src.len() != dst.len() || !src.len().is_multiple_of(4) {
        return;
    }
    for (s, d) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        d[0] = s[2];
        d[1] = s[1];
        d[2] = s[0];
        d[3] = s[3];
    }
}
