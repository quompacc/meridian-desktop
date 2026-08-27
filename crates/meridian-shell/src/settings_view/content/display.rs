fn build_display_content(ctx: &SettingsContentContext<'_>) -> Box<dyn Widget> {
    let row_w = settings_group_inner_width(ctx.content_w);
    let mut rows: Vec<Box<dyn Widget>> = Vec::new();
    if ctx.output_workspaces.is_empty() {
        rows.push(Box::new(SettingsPlaceholder {
            width: row_w,
            text: "No output snapshot received yet",
        }));
    } else {
        for (idx, output) in ctx
            .output_workspaces
            .iter()
            .take(DISPLAY_OUTPUT_MAX)
            .enumerate()
        {
            let name = output
                .output_name
                .as_deref()
                .map(str::to_string)
                .unwrap_or_else(|| format!("Output {}", output.output_id));
            let identity = Box::new(DisplayOutputRow {
                output_id: output.output_id,
                name: name.into(),
                workspace: output.active_workspace,
                primary: output.primary,
                focused: output.focused,
                x: output.x,
                y: output.y,
                width: output.width,
                height: output.height,
                scale_millis: output.scale_millis,
                transform: output.transform.as_deref().map(Into::into),
                refresh_millihz: output.refresh_millihz,
                mode_count: output.modes.len(),
                row_width: row_w,
                accent: ctx.pal.accent,
            }) as Box<dyn Widget>;
            let combo_label = selected_display_mode(output)
                .map(display_mode_label)
                .unwrap_or_else(|| "No modes".to_string());
            let expanded = ctx.display_mode_dropdown_open == Some(idx);
            let mode_combo = Box::new(DisplayModeComboButton {
                index: idx,
                label: combo_label.into(),
                expanded,
                enabled: !output.modes.is_empty(),
                accent: ctx.pal.accent,
                width: row_w,
            }) as Box<dyn Widget>;
            let primary_button = Box::new(DisplayPrimaryButton {
                index: idx,
                active: output.primary,
                accent: ctx.pal.accent,
                width: (row_w - SETTINGS_CHROME.display_control_gap * 2) / 3,
            }) as Box<dyn Widget>;
            // Current scale label, e.g. "1.5×".
            let scale_label = {
                let s = output.scale_millis as f64 / 1000.0;
                if (s - s.round()).abs() < f64::EPSILON {
                    format!("{}×", s.round() as i64)
                } else {
                    format!("{}×", s)
                }
            };
            let scale_button = DISPLAY_SCALE_IDS.get(idx).map(|id| {
                Box::new(DisplayCycleButton {
                    id,
                    caption: "Skalierung",
                    value: scale_label.into(),
                    accent: ctx.pal.accent,
                    width: (row_w - SETTINGS_CHROME.display_control_gap * 2) / 3,
                }) as Box<dyn Widget>
            });
            // Current rotation label from the transform string.
            let rotate_label = match output.transform.as_deref() {
                Some("90") => "90°",
                Some("180") => "180°",
                Some("270") => "270°",
                _ => "0°",
            };
            let rotate_button = DISPLAY_ROTATE_IDS.get(idx).map(|id| {
                Box::new(DisplayCycleButton {
                    id,
                    caption: "Drehung",
                    value: rotate_label.into(),
                    accent: ctx.pal.accent,
                    width: (row_w - SETTINGS_CHROME.display_control_gap * 2) / 3,
                }) as Box<dyn Widget>
            });
            rows.push(identity);
            rows.push(mode_combo);
            let mut control_items: Vec<Box<dyn Widget>> = vec![primary_button];
            control_items.extend(scale_button);
            control_items.extend(rotate_button);
            rows.push(Box::new(Container::row(
                SETTINGS_CHROME.display_control_gap,
                control_items,
            )));

            if expanded {
                for (mode_idx, mode) in output
                    .modes
                    .iter()
                    .take(DISPLAY_MODE_OPTION_MAX)
                    .enumerate()
                {
                    rows.push(Box::new(DisplayModeOptionRow {
                        output_index: idx,
                        mode_index: mode_idx,
                        label: display_mode_label(mode).into(),
                        selected: mode.current,
                        row_width: row_w,
                        accent: ctx.pal.accent,
                    }));
                }
            }
        }
    }
    build_settings_group_page(
        ctx.content_w,
        ctx.content_h,
        "Bildschirme",
        "Auflösung, Skalierung, Ausrichtung und primäre Anzeige.",
        Box::new(Container::column(4, rows)),
    )
}
