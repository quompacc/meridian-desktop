fn build_network_content(ctx: &SettingsContentContext<'_>) -> Box<dyn Widget> {
    let row_w = settings_group_inner_width(ctx.content_w);
    let mut rows: Vec<Box<dyn Widget>> = ctx
        .network_state
        .settings_rows()
        .into_iter()
        .map(|(label, value)| {
            Box::new(SystemInfoRow {
                label: label.into(),
                value: value.into(),
                row_width: row_w,
            }) as Box<dyn Widget>
        })
        .collect();

    // Saved profiles: the active one shows a badge and is inert; the
    // rest are clickable to activate. Bounded to the id-array length.
    rows.push(Box::new(SidebarSectionLabel {
        text: "GESPEICHERTE VERBINDUNGEN",
        width: row_w,
        pad_top: 12,
    }));
    if ctx.network_profiles.is_empty() {
        rows.push(Box::new(SettingsPlaceholder {
            width: row_w,
            text: "Keine gespeicherten Verbindungen (oder nmcli fehlt)",
        }));
    } else {
        for (i, profile) in ctx
            .network_profiles
            .iter()
            .take(NETWORK_PROFILE_IDS.len())
            .enumerate()
        {
            rows.push(Box::new(NetworkProfileRow {
                index: i,
                name: profile.name.as_str().into(),
                type_label: profile.type_label.as_str().into(),
                active: profile.active,
                accent: ctx.pal.accent,
                row_width: row_w,
            }));
        }
    }

    // WLAN: scanned networks. The in-use one is inert; others connect
    // on click (a secured-unknown one opens the centered password modal
    // in dispatch).
    rows.push(Box::new(SidebarSectionLabel {
        text: "WLAN-NETZWERKE",
        width: row_w,
        pad_top: 12,
    }));
    if ctx.wifi_networks.is_empty() {
        rows.push(Box::new(SettingsPlaceholder {
            width: row_w,
            text: "Keine WLAN-Netzwerke gefunden",
        }));
    } else {
        for (i, net) in ctx
            .wifi_networks
            .iter()
            .take(WIFI_NETWORK_IDS.len())
            .enumerate()
        {
            rows.push(Box::new(WifiRow {
                index: i,
                ssid: net.ssid.as_str().into(),
                signal: net.signal,
                secured: net.secured,
                in_use: net.in_use,
                accent: ctx.pal.accent,
                row_width: row_w,
            }));
        }
    }
    build_settings_group_page(
        ctx.content_w,
        ctx.content_h,
        "Verbindungen",
        "Aktiver Netzwerkstatus, gespeicherte Profile und verfügbare WLANs.",
        Box::new(Container::column(4, rows)),
    )
}
