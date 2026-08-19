fn build_sound_content(ctx: &SettingsContentContext<'_>) -> Box<dyn Widget> {
    let row_w = ctx.content_w as i32;
    let mut rows: Vec<Box<dyn Widget>> = vec![Box::new(SoundSummaryCard {
        snapshot: ctx.audio_snapshot.clone(),
        row_width: row_w,
        accent: ctx.pal.accent,
    }) as Box<dyn Widget>];

    // Default-output controls: volume preset chips + a mute toggle.
    // Only meaningful when a default sink is present.
    if let Some(default) = ctx.audio_snapshot.default_output.as_ref() {
        let current = default.volume_percent;
        let mut chips: Vec<Box<dyn Widget>> = VOLUME_PRESET_OPTIONS
            .iter()
            .map(|(pct, id, label)| {
                let accent = if current == Some(*pct) {
                    ctx.pal.accent
                } else {
                    ctx.pal.surface
                };
                Box::new(Button::with_id(id, label, accent, 64, 32)) as Box<dyn Widget>
            })
            .collect();
        // Mute toggle accents when currently muted.
        let mute_accent = if default.muted {
            ctx.pal.accent
        } else {
            ctx.pal.surface
        };
        let mute_label = if default.muted { "Stumm: an" } else { "Stumm" };
        chips.push(Box::new(Button::with_id(
            "mute-toggle",
            mute_label,
            mute_accent,
            96,
            32,
        )) as Box<dyn Widget>);
        rows.push(Box::new(SidebarSectionLabel {
            text: "STANDARD-AUSGABE",
            width: row_w,
            pad_top: 0,
        }));
        rows.push(Box::new(Container::row(8, chips)));
    }

    if ctx.audio_snapshot.service != AudioServiceState::Running {
        rows.push(Box::new(SettingsPlaceholder {
            width: row_w,
            text: "PipeWire/WirePlumber status is not available",
        }));
    } else if ctx.audio_snapshot.outputs.is_empty() && ctx.audio_snapshot.inputs.is_empty() {
        rows.push(Box::new(SettingsPlaceholder {
            width: row_w,
            text: "No audio devices reported",
        }));
    } else {
        for (i, device) in ctx
            .audio_snapshot
            .outputs
            .iter()
            .take(SOUND_MAX)
            .enumerate()
        {
            rows.push(Box::new(SoundDeviceRow {
                label: "Output",
                device: device.clone(),
                index: i,
                ids: AUDIO_OUTPUT_IDS,
                row_width: row_w,
                accent: ctx.pal.accent,
            }));
        }
        for (i, device) in ctx.audio_snapshot.inputs.iter().take(SOUND_MAX).enumerate() {
            rows.push(Box::new(SoundDeviceRow {
                label: "Input",
                device: device.clone(),
                index: i,
                ids: AUDIO_INPUT_IDS,
                row_width: row_w,
                accent: ctx.pal.accent,
            }));
        }
    }

    Box::new(Container::top_viewport(
        ctx.content_w,
        ctx.content_h,
        14,
        16,
        4,
        vec![Box::new(Container::column(4, rows)) as Box<dyn Widget>],
    ))
}
