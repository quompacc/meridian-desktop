fn build_bluetooth_content(ctx: &SettingsContentContext<'_>) -> Box<dyn Widget> {
    let row_w = settings_group_inner_width(ctx.content_w);
    let mut rows: Vec<Box<dyn Widget>> = Vec::new();

    if !ctx.bluetooth_snapshot.adapter_present {
        rows.push(Box::new(SettingsPlaceholder {
            width: row_w,
            text: "Kein Bluetooth-Adapter gefunden",
        }));
    } else {
        // Power toggle + scan toggle, accented to current state.
        rows.push(Box::new(SidebarSectionLabel {
            text: "ADAPTER",
            width: row_w,
            pad_top: 0,
        }));
        let power_accent = if ctx.bluetooth_snapshot.powered {
            ctx.pal.accent
        } else {
            ctx.pal.surface
        };
        let power_label = if ctx.bluetooth_snapshot.powered {
            "Bluetooth: an"
        } else {
            "Bluetooth: aus"
        };
        let scan_accent = if ctx.bluetooth_snapshot.scanning {
            ctx.pal.accent
        } else {
            ctx.pal.surface
        };
        let scan_label = if ctx.bluetooth_snapshot.scanning {
            "Suche läuft"
        } else {
            "Suchen"
        };
        rows.push(Box::new(Container::row(
            8,
            vec![
                Box::new(Button::with_id(
                    "bt-power-toggle",
                    power_label,
                    power_accent,
                    128,
                    32,
                )) as Box<dyn Widget>,
                Box::new(Button::with_id(
                    "bt-scan-toggle",
                    scan_label,
                    scan_accent,
                    112,
                    32,
                )) as Box<dyn Widget>,
            ],
        )));

        // Device list (only meaningful while powered).
        rows.push(Box::new(SidebarSectionLabel {
            text: "GERÄTE",
            width: row_w,
            pad_top: 12,
        }));
        if !ctx.bluetooth_snapshot.powered {
            rows.push(Box::new(SettingsPlaceholder {
                width: row_w,
                text: "Bluetooth ist ausgeschaltet",
            }));
        } else if ctx.bluetooth_snapshot.devices.is_empty() {
            rows.push(Box::new(SettingsPlaceholder {
                width: row_w,
                text: "Keine Geräte — auf Suchen tippen",
            }));
        } else {
            for (i, dev) in ctx
                .bluetooth_snapshot
                .devices
                .iter()
                .take(BT_DEVICE_IDS.len())
                .enumerate()
            {
                rows.push(Box::new(BluetoothDeviceRow {
                    index: i,
                    name: dev.name.as_str().into(),
                    address: dev.address.as_str().into(),
                    paired: dev.paired,
                    connected: dev.connected,
                    accent: ctx.pal.accent,
                    row_width: row_w,
                }));
            }
        }
    }
    build_settings_group_page(
        ctx.content_w,
        ctx.content_h,
        "Bluetooth-Geräte",
        "Adapterstatus, Suche und gekoppelte Geräte.",
        Box::new(Container::column(4, rows)),
    )
}
