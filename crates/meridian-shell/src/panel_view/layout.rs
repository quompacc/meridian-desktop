fn draw_circle(canvas: &mut PixmapMut<'_>, cx: f32, cy: f32, radius: f32, color: Color) {
    use tiny_skia::{FillRule, Paint, PathBuilder, Transform};
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, radius);
    if let Some(path) = pb.finish() {
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color_rgba8(color.r, color.g, color.b, color.a);
        canvas.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn windows_for_pinned_app(app: &PinnedApp, windows: &[PanelWindowEntry]) -> (usize, bool) {
    let program_base = std::path::Path::new(&app.program)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&app.program)
        .to_lowercase();
    let label_lower = app.label.to_lowercase();

    windows.iter().fold((0usize, false), |(count, focused), w| {
        let matches = if let Some(ref app_id) = w.app_id {
            let aid = app_id.to_lowercase();
            aid == program_base
                || aid.ends_with(&format!(".{}", program_base))
                || aid == label_lower
                || aid.ends_with(&format!(".{}", label_lower))
        } else {
            // Fallback: title-based matching for when compositor hasn't sent app_id yet
            let title_lower = w.title.to_lowercase();
            !program_base.is_empty() && title_lower.contains(&program_base)
                || !label_lower.is_empty() && title_lower.contains(&label_lower)
        };
        if matches {
            (count + 1, focused || w.focused)
        } else {
            (count, focused)
        }
    })
}

// ── build_panel_widget_tree ─────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
/// Recolour a premultiplied-RGBA pixmap to `color`, keeping its alpha (shape).
/// Used to tint the symbolic battery icon to the active power-profile colour.
fn tint_pixmap_premul(pm: &mut Pixmap, color: Color) {
    for px in pm.data_mut().chunks_exact_mut(4) {
        let a = px[3] as u16;
        px[0] = ((color.r as u16 * a) / 255) as u8;
        px[1] = ((color.g as u16 * a) / 255) as u8;
        px[2] = ((color.b as u16 * a) / 255) as u8;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_panel_widget_tree(
    width: u32,
    pinned_apps: &[PinnedApp],
    window_entries: &[PanelWindowEntry],
    network_state: &NetworkState,
    audio_snapshot: &AudioSnapshot,
    status_notifier_items: &[StatusNotifierItem],
    network_popup_open: bool,
    audio_popup_open: bool,
    battery: &crate::battery::BatterySnapshot,
    power_profile: Option<crate::power_profile::PowerProfile>,
    active_workspace: u8,
    total_workspaces: u8,
    clock: &str,
    icon_cache: &IconCache,
    screenshot_icon: Option<Pixmap>,
    theme: &Theme,
) -> Box<dyn Widget> {
    let network_icon = icon_cache
        .lookup(network_state.icon_name(), STATUS_ICON_SIZE)
        .and_then(icon_image_to_pixmap)
        .map(|mut icon| {
            tint_pixmap_premul(&mut icon, theme.palette.text_dim);
            icon
        });
    let audio_icon = icon_cache
        .lookup(audio_snapshot.icon_name(), STATUS_ICON_SIZE)
        .and_then(icon_image_to_pixmap)
        .map(|mut icon| {
            tint_pixmap_premul(&mut icon, theme.palette.text_dim);
            icon
        })
        .or_else(|| build_audio_icon(audio_snapshot, theme));
    // Battery icon, tinted by the active power profile so the choice is visible
    // at a glance: Eco = green, Performance = amber, Standard = neutral (default
    // text tint). Colours come from the theme palette (centralised).
    let battery_icon = icon_cache
        .lookup(battery.icon_name(), STATUS_ICON_SIZE)
        .and_then(icon_image_to_pixmap)
        .map(|mut pm| {
            use crate::power_profile::PowerProfile;
            tint_pixmap_premul(&mut pm, theme.palette.text_dim);
            let tint = match power_profile {
                Some(PowerProfile::Eco) => Some(theme.palette.success),
                Some(PowerProfile::Performance) => Some(theme.palette.warning),
                _ => None,
            };
            if let Some(c) = tint {
                tint_pixmap_premul(&mut pm, c);
            }
            pm
        });

    // Left cluster
    let mut left_children: Vec<Box<dyn Widget>> = Vec::new();
    let launcher_icon = build_launcher_icon(theme);
    left_children.push(Box::new(PanelChip::new(
        "panel-launcher",
        "Apps".into(),
        launcher_icon,
        LAUNCHER_W,
        false,
    )));
    if !pinned_apps.is_empty() {
        left_children.push(Box::new(PanelDivider));
    }
    for (idx, app) in pinned_apps.iter().enumerate() {
        let (window_count, has_focused) = windows_for_pinned_app(app, window_entries);
        let icon = app
            .icon_name
            .as_deref()
            .and_then(|name| icon_cache.lookup(name, APP_ICON_SIZE))
            .and_then(icon_image_to_pixmap);
        left_children.push(Box::new(PanelPinnedChip {
            idx,
            label: app.label.clone().into_boxed_str(),
            icon,
            program: app.program.clone().into_boxed_str(),
            args: app.args.clone(),
            window_count,
            has_focused,
        }));
    }
    let left_cluster = Container::new(
        WidgetStyle {
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Center),
            gap: UiSize {
                width: ui_length(GAP as f32),
                height: ui_length(0.0),
            },
            ..Default::default()
        },
        left_children,
    );

    // Center cluster — empty spacer; window indicators are now shown as badges on pinned icons
    let center_children: Vec<Box<dyn Widget>> = Vec::new();
    let center_cluster = Container::new(
        WidgetStyle {
            flex_direction: FlexDirection::Row,
            flex_grow: 1.0,
            align_items: Some(AlignItems::Center),
            gap: UiSize {
                width: ui_length(GAP as f32),
                height: ui_length(0.0),
            },
            overflow: TaffyPoint {
                x: Overflow::Hidden,
                y: Overflow::Hidden,
            },
            ..Default::default()
        },
        center_children,
    );

    // Right cluster
    let mut right_children: Vec<Box<dyn Widget>> = Vec::new();
    for (idx, item) in status_notifier_items
        .iter()
        .take(SNI_PANEL_IDS.len())
        .enumerate()
    {
        let icon = item
            .icon_name
            .as_deref()
            .and_then(|name| icon_cache.lookup(name, STATUS_ICON_SIZE))
            .and_then(icon_image_to_pixmap)
            .map(|mut icon| {
                tint_pixmap_premul(&mut icon, theme.palette.text_dim);
                icon
            });
        right_children.push(Box::new(PanelChip::new(
            SNI_PANEL_IDS[idx],
            status_notifier_label(item).into_boxed_str(),
            icon,
            SNI_W,
            false,
        )));
    }
    if !right_children.is_empty() {
        right_children.push(Box::new(PanelDivider));
    }
    let screenshot_icon = screenshot_icon.map(|mut icon| {
        tint_pixmap_premul(&mut icon, theme.palette.text_dim);
        icon
    });
    right_children.push(Box::new(PanelChip::new(
        "panel-screenshot",
        "📷".into(),
        screenshot_icon,
        SCREENSHOT_W,
        false,
    )));
    right_children.push(Box::new(PanelDivider));

    let mut status_children: Vec<Box<dyn Widget>> = vec![
        Box::new(PanelStatusIcon {
            label: "NET".into(),
            icon: network_icon,
        }),
        Box::new(PanelStatusIcon {
            label: audio_snapshot.panel_label().into_boxed_str(),
            icon: audio_icon,
        }),
    ];
    if battery.present {
        status_children.push(Box::new(PanelStatusIcon {
            label: battery.label().into_boxed_str(),
            icon: battery_icon,
        }));
    }
    right_children.push(Box::new(PanelStatusGroup {
        active: network_popup_open || audio_popup_open,
        children: status_children,
    }));
    right_children.extend([
        Box::new(PanelDivider) as Box<dyn Widget>,
        Box::new(PanelWorkspaceChip {
            active: active_workspace,
            total: total_workspaces,
        }),
        Box::new(PanelClockChip {
            value: clock.into(),
        }),
    ]);
    let right_cluster = Container::new(
        WidgetStyle {
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Center),
            gap: UiSize {
                width: ui_length(GAP as f32),
                height: ui_length(0.0),
            },
            ..Default::default()
        },
        right_children,
    );

    // The bar holds the three clusters and is inset to the island content box.
    let inner_w = (width as i32 - 2 * SIDE_MARGIN).max(0);
    let bar = Container::new(
        WidgetStyle {
            flex_direction: FlexDirection::Row,
            justify_content: Some(JustifyContent::SpaceBetween),
            align_items: Some(AlignItems::Center),
            size: UiSize {
                width: ui_length(inner_w as f32),
                height: ui_length(PANEL_H as f32),
            },
            padding: TaffyRect {
                left: ui_length(LEFT_PADDING as f32),
                right: ui_length(RIGHT_PADDING as f32),
                top: ui_length(0.0),
                bottom: ui_length(0.0),
            },
            ..Default::default()
        },
        vec![
            Box::new(left_cluster) as Box<dyn Widget>,
            Box::new(center_cluster) as Box<dyn Widget>,
            Box::new(right_cluster) as Box<dyn Widget>,
        ],
    );

    // Outer surface wrapper: full width, inset on the sides + a bottom gap so
    // the island floats. The padding offsets the bar so render() and the click
    // zones inherit the inset automatically.
    Box::new(Container::new(
        WidgetStyle {
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::FlexStart),
            size: UiSize {
                width: ui_length(width as f32),
                height: ui_length(SURFACE_H as f32),
            },
            padding: TaffyRect {
                left: ui_length(SIDE_MARGIN as f32),
                right: ui_length(SIDE_MARGIN as f32),
                top: ui_length(ISLAND_TOP as f32),
                bottom: ui_length(BOTTOM_GAP as f32),
            },
            ..Default::default()
        },
        vec![Box::new(bar) as Box<dyn Widget>],
    ))
}

// ── collect_click_zones ─────────────────────────────────────────────────────
