fn collect_click_zones(
    widget: &dyn Widget,
    node: &LayoutNode,
    parent_x: i32,
    parent_y: i32,
    out: &mut Vec<ClickZone>,
) {
    let abs_x = parent_x + node.rect.x;
    let abs_y = parent_y + node.rect.y;

    let action = widget
        .id()
        .and_then(action_for_id_as_click)
        .or_else(|| widget.pinned_app_idx().map(ClickAction::LaunchPinnedApp))
        .or_else(|| {
            widget
                .focus_window_id()
                .map(|id| ClickAction::FocusWindow(id.to_string()))
        });

    if let Some(action) = action {
        out.push(ClickZone {
            id: widget.id().map(str::to_string),
            rect: ShellRect {
                x: abs_x,
                y: abs_y,
                w: node.rect.width,
                h: node.rect.height,
            },
            action,
        });
    }

    for (child, child_node) in widget.children().iter().zip(node.children.iter()) {
        collect_click_zones(child.as_ref(), child_node, abs_x, abs_y, out);
    }
}

// ── draw_panel_ui ───────────────────────────────────────────────────────────

/// Add a fine, position-deterministic brightness grain to a rectangular
/// region of a tiny-skia RGBA(premultiplied) buffer. Deterministic so it does
/// not shimmer between redraws; only touches pixels that belong to the island
/// (alpha > 0). Kept at zero strength while the compositor provides live blur.
fn apply_frost_noise(data: &mut [u8], w: usize, _h: usize, x0: i32, y0: i32, x1: i32, y1: i32) {
    for y in y0.max(0)..y1 {
        for x in x0.max(0)..x1 {
            let idx = (y as usize * w + x as usize) * 4;
            if idx + 4 > data.len() || data[idx + 3] == 0 {
                continue;
            }
            let mut n = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
            n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
            n ^= n >> 16;
            let delta = ((n & 0xff) as i32 - 128) * PANEL_NOISE_STRENGTH / 128;
            for k in 0..3 {
                data[idx + k] = (data[idx + k] as i32 + delta).clamp(0, 255) as u8;
            }
        }
    }
}

fn blit_rgba_to_argb(src: &[u8], dst: &mut [u8]) {
    if src.len() != dst.len() || !src.len().is_multiple_of(4) {
        return;
    }
    for (rgba, argb) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        argb[0] = rgba[2];
        argb[1] = rgba[1];
        argb[2] = rgba[0];
        argb[3] = rgba[3];
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_panel_ui(
    canvas: &mut [u8],
    width: u32,
    height: u32,
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
    theme_config: &meridian_config::ThemeConfig,
    state_fn: &dyn Fn(&[usize]) -> WidgetState,
    clicks_out: &mut Vec<ClickZone>,
) {
    let expected_len = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if canvas.len() != expected_len {
        tracing::warn!(
            "draw_panel_ui: canvas size mismatch, expected {} got {}",
            expected_len,
            canvas.len()
        );
        return;
    }

    let theme = glass_theme_from_config(theme_config);
    let treatment = theme_config
        .decorations
        .surface_treatment(ThemeSurface::Panel);
    let island_radius = treatment.radius.round() as i32;

    let root = build_panel_widget_tree(
        width,
        pinned_apps,
        window_entries,
        network_state,
        audio_snapshot,
        status_notifier_items,
        network_popup_open,
        audio_popup_open,
        battery,
        power_profile,
        active_workspace,
        total_workspaces,
        clock,
        icon_cache,
        screenshot_icon,
        &theme,
    );

    let Ok(layout) = compute_layout(&*root, PixelSize { width, height }) else {
        return;
    };

    let Some(mut pixmap) = Pixmap::new(width, height) else {
        return;
    };
    // Transparent everywhere; only the inset island is painted, so the
    // wallpaper shows through the side margins and the bottom gap.
    pixmap.fill(tiny_skia::Color::TRANSPARENT);

    // Frosted-glass island. The compositor owns the live blurred backdrop AND
    // renders it at the theme's fill_alpha; so when glass is on the shell must
    // NOT paint an opaque body on top (that would double the opacity and hide
    // the blur — the panel would read as solid). Keep the body transparent and
    // let the compositor glass show, exactly like the popups. The non-glass
    // fallback still paints a solid island so it stays legible.
    let inner_w = (width as i32 - 2 * SIDE_MARGIN).max(0);
    let base = theme.palette.surface_alt;
    let glass = theme_config.decorations.glass && theme_config.decorations.glass_blur;
    let body_alpha = if glass { 0 } else { treatment.fill_alpha };
    let body_col = Color::rgba(base.r, base.g, base.b, body_alpha);
    let outline = Rect {
        x: SIDE_MARGIN,
        y: ISLAND_TOP,
        width: inner_w,
        height: PANEL_H,
    };
    let body = Rect {
        x: SIDE_MARGIN + 1,
        y: ISLAND_TOP + 1,
        width: (inner_w - 2).max(0),
        height: PANEL_H - 2,
    };
    // Soft drop shadow around the floating island (cast up onto the desktop).
    crate::soft_shadow::draw_soft_shadow(
        pixmap.data_mut(),
        width as i32,
        height as i32,
        outline.x,
        outline.y,
        outline.width,
        outline.height,
        island_radius as f32,
        Elevation::PANEL.blur,
        Elevation::PANEL.alpha,
        Elevation::PANEL.offset_y,
        true,
    );
    {
        let mut pc = pixmap.as_mut();
        if let Some(path) = rounded_rect_path(body, island_radius.saturating_sub(1)) {
            paint_fill(&mut pc, &path, body_col);
        }
        let frost_alpha = ((treatment.frame_alpha as f32) * 0.10).round() as u8;
        // guard:allow: glass specular frost — white sheen is a material highlight
        // (manifest §7.1 "leichte Innenaufhellung"), theme-independent; its
        // strength is already theme-driven via `treatment.frame_alpha`.
        let frost = Color::rgba(0xFF, 0xFF, 0xFF, frost_alpha);
        if let Some(path) = rounded_rect_path(body, island_radius.saturating_sub(1)) {
            paint_fill(&mut pc, &path, frost);
        }
    }
    apply_frost_noise(
        pixmap.data_mut(),
        width as usize,
        height as usize,
        body.x,
        body.y,
        body.x + body.width,
        body.y + body.height,
    );
    {
        let mut pc = pixmap.as_mut();
        // guard:allow: glass specular top-edge highlight — white sheen (material,
        // not a theme colour); strength is theme-driven via `treatment.frame_alpha`.
        let hl = Color::rgba(0xFF, 0xFF, 0xFF, treatment.frame_alpha / 4);
        let highlight = Rect {
            x: SIDE_MARGIN + island_radius,
            y: ISLAND_TOP + 1,
            width: (inner_w - 2 * island_radius).max(0),
            height: 1,
        };
        if let Some(path) = rounded_rect_path(highlight, 0) {
            paint_fill(&mut pc, &path, hl);
        }
        let _ = render(&*root, &layout, &mut pc, &theme, state_fn);
    }

    blit_rgba_to_argb(pixmap.data(), canvas);

    clicks_out.clear();
    collect_click_zones(&*root, &layout.root, 0, 0, clicks_out);
}
