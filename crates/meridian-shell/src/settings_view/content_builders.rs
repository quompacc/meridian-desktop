include!("content/theme.rs");
include!("content/wallpaper.rs");
include!("content/display.rs");
include!("content/pinned_apps.rs");
include!("content/default_apps.rs");
include!("content/system_overview.rs");
include!("content/network.rs");
include!("content/power.rs");
include!("content/users.rs");
include!("content/bluetooth.rs");
include!("content/updates.rs");
include!("content/sound.rs");
include!("content/printers.rs");
include!("content/cursor.rs");

struct SettingsContentContext<'a> {
    content_h: u32,
    content_w: u32,
    query: &'a str,
    available_themes: &'a [String],
    current_theme: &'a str,
    available_wallpapers: &'a [WallpaperEntry],
    wallpaper_thumbnails: &'a [Option<(u32, u32, Vec<u8>)>],
    current_wallpaper: Option<&'a str>,
    wallpaper_mode: WallpaperMode,
    cursor_size: u32,
    available_cursor_themes: &'a [String],
    current_cursor_theme: &'a str,
    idle_timeout_secs: Option<u64>,
    pinned_apps: &'a [PinnedApp],
    output_workspaces: &'a [OutputWorkspaceState],
    display_mode_dropdown_open: Option<usize>,
    printer_snapshot: &'a PrinterSnapshot,
    audio_snapshot: &'a AudioSnapshot,
    system_info: &'a SystemInfo,
    network_state: &'a NetworkState,
    network_profiles: &'a [ConnectionProfile],
    bluetooth_snapshot: &'a BluetoothSnapshot,
    wifi_networks: &'a [WifiNetwork],
    pinned_adding: bool,
    all_apps: &'a [DesktopApp],
    icon_cache: &'a IconCache,
    default_apps_index: Option<&'a crate::default_apps::MimeAppIndex>,
    default_apps_current:
        &'a std::collections::HashMap<crate::default_apps::DefaultAppCategory, String>,
    default_apps_picker_open: Option<crate::default_apps::DefaultAppCategory>,
    pal: &'a meridian_ui::style::Palette,
}

pub(crate) fn build_settings_widget_tree(
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
    theme: &Theme,
) -> Box<dyn Widget> {
    let pal = theme.palette;

    let header = Box::new(SettingsHeaderBar {
        width: width as i32,
        children: vec![
            Box::new(SettingsBackButton) as Box<dyn Widget>,
            Box::new(SettingsSearchField {
                width: width as i32 - SETTINGS_BACK_W,
                query: search.into(),
            }) as Box<dyn Widget>,
        ],
    }) as Box<dyn Widget>;

    let divider_color = Color::rgba(
        pal.accent.r,
        pal.accent.g,
        pal.accent.b,
        SETTINGS_DIVIDER_OPACITY,
    );
    // No root tabs anymore — the two groups live as labelled sections inside
    // one full-height sidebar.
    let content_h = height.saturating_sub(HEADER_HEIGHT + DIVIDER_HEIGHT);
    let content_w = width.saturating_sub(SIDEBAR_W + 1);

    // Left sidebar — grouped sections (Darstellung / System), all categories
    // listed, anchored to the top.
    let query = search.trim().to_lowercase();
    // Widened match: category label, static keywords, and dynamic content
    // (theme / wallpaper names) so the search reflects intent.
    let cat_matches = |cat: &SettingsCategory| -> bool {
        if query.is_empty() {
            return true;
        }
        if cat.label().to_lowercase().contains(&query) {
            return true;
        }
        if cat
            .search_keywords()
            .iter()
            .any(|kw| kw.contains(query.as_str()))
        {
            return true;
        }
        match cat {
            SettingsCategory::Theme => available_themes
                .iter()
                .any(|t| t.to_lowercase().contains(&query)),
            SettingsCategory::Wallpaper => available_wallpapers
                .iter()
                .any(|w| w.display_name.to_lowercase().contains(&query)),
            _ => false,
        }
    };
    // While searching, preview the first matching category if the stored
    // selection was filtered out — the whole view follows the query.
    let effective_selected = if query.is_empty() || cat_matches(&selected) {
        selected
    } else {
        SettingsCategory::DESKTOP
            .iter()
            .chain(SettingsCategory::SYSTEM.iter())
            .copied()
            .find(|cat| cat_matches(cat))
            .unwrap_or(selected)
    };
    let mut sidebar_children: Vec<Box<dyn Widget>> = Vec::new();
    let groups: [(&str, &[SettingsCategory], i32); 2] = [
        ("DARSTELLUNG", SettingsCategory::DESKTOP, 0),
        ("SYSTEM", SettingsCategory::SYSTEM, 14),
    ];
    let mut any_match = false;
    for (title, cats, pad_top) in groups {
        let matching: Vec<&SettingsCategory> = cats.iter().filter(|cat| cat_matches(cat)).collect();
        if matching.is_empty() {
            continue;
        }
        any_match = true;
        sidebar_children.push(Box::new(SidebarSectionLabel {
            text: title,
            width: SIDEBAR_W as i32,
            pad_top,
        }) as Box<dyn Widget>);
        for cat in matching {
            sidebar_children.push(Box::new(SettingsSidebarRow {
                cat: *cat,
                is_selected: *cat == effective_selected,
                accent: pal.accent,
                row_width: SIDEBAR_W as i32,
            }) as Box<dyn Widget>);
        }
    }
    if !any_match {
        sidebar_children.push(Box::new(SidebarSectionLabel {
            text: "KEINE TREFFER",
            width: SIDEBAR_W as i32,
            pad_top: 8,
        }) as Box<dyn Widget>);
    }
    let sidebar = Box::new(SidebarPanel {
        width: SIDEBAR_W as i32,
        height: content_h as i32,
        bg: pal.surface_alt,
        children: sidebar_children,
    }) as Box<dyn Widget>;

    let vsep = Box::new(VerticalDivider {
        height: content_h as i32,
        color: divider_color,
    }) as Box<dyn Widget>;

    let content_ctx = SettingsContentContext {
        content_h,
        content_w,
        query: &query,
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
        default_apps_index,
        default_apps_current,
        default_apps_picker_open,
        pal: &pal,
    };

    let content: Box<dyn Widget> = match effective_selected {
        SettingsCategory::Theme => build_theme_content(&content_ctx),
        SettingsCategory::Wallpaper => build_wallpaper_content(&content_ctx),
        SettingsCategory::Display => build_display_content(&content_ctx),
        SettingsCategory::PinnedApps => build_pinned_apps_content(&content_ctx),
        SettingsCategory::DefaultApps => build_default_apps_content(&content_ctx),
        SettingsCategory::SystemOverview => build_system_overview_content(&content_ctx),
        SettingsCategory::Network => build_network_content(&content_ctx),
        SettingsCategory::Power => build_power_content(&content_ctx),
        SettingsCategory::Users => build_users_content(&content_ctx),
        SettingsCategory::Bluetooth => build_bluetooth_content(&content_ctx),
        SettingsCategory::Updates => build_updates_content(&content_ctx),
        SettingsCategory::Sound => build_sound_content(&content_ctx),
        SettingsCategory::Printers => build_printers_content(&content_ctx),
        SettingsCategory::Cursor => build_cursor_content(&content_ctx),
    };

    let body = Box::new(Container::row(0, vec![sidebar, vsep, content])) as Box<dyn Widget>;

    // No footer: the header back arrow handles "return to launcher". The body
    // (sidebar + content) runs to the bottom edge for a grounded look.
    let divider = Box::new(Divider {
        width: width as i32,
        color: divider_color,
    }) as Box<dyn Widget>;

    Box::new(Container::column(0, vec![header, divider, body]))
}
