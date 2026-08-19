impl MeridianConfig {
    /// Write (or update) only the theme key in the config file without
    /// touching any other settings the user may have made.
    pub fn save_theme(name: &str) {
        let config_path = config_directory().join("config.toml");
        let raw = if config_path.exists() {
            fs::read_to_string(&config_path).unwrap_or_default()
        } else {
            String::new()
        };

        let new_line = format!("theme = \"{}\"", name);
        let nl = '\n';

        // If the file already contains a `theme = ...` line inside
        // `[general]`, replace it in-place; otherwise append a
        // `[general]` section.
        let updated = if let Some(pos) = find_theme_line(&raw) {
            // Reconstruct with the theme line replaced.
            let mut out = String::new();
            for (i, l) in raw.lines().enumerate() {
                if i == pos {
                    out.push_str(&new_line);
                } else {
                    out.push_str(l);
                }
                out.push(nl);
            }
            out
        } else if has_general_section(&raw) {
            // Section exists but no theme key yet — insert after [general].
            let mut out = String::new();
            let mut inserted = false;
            for line in raw.lines() {
                out.push_str(line);
                out.push(nl);
                if !inserted && line.trim() == "[general]" {
                    out.push_str(&new_line);
                    out.push(nl);
                    inserted = true;
                }
            }
            out
        } else {
            // No [general] section at all — append it.
            let mut out = raw.clone();
            if !out.ends_with(nl) && !out.is_empty() {
                out.push(nl);
            }
            out.push_str("\n[general]\n");
            out.push_str(&new_line);
            out.push(nl);
            out
        };

        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(&config_path, updated.as_bytes()) {
            warn!("Failed to write theme to config: {}", e);
        } else {
            info!("Saved theme {:?} to {:?}", name, config_path);
        }
    }
    /// Write (or update) the [wallpaper] section in config.toml.
    /// Pass an empty `path` to remove the wallpaper section entirely.
    pub fn save_wallpaper(path: &str, mode: WallpaperMode) {
        let config_path = config_directory().join("config.toml");
        let raw = if config_path.exists() {
            fs::read_to_string(&config_path).unwrap_or_default()
        } else {
            String::new()
        };

        let stripped = strip_toml_section(&raw, "wallpaper");
        let updated = if path.is_empty() {
            stripped
        } else {
            let mut out = stripped;
            if !out.ends_with('\n') && !out.is_empty() {
                out.push('\n');
            }
            out.push_str("\n[wallpaper]\n");
            out.push_str(&format!("path = \"{}\"\n", path));
            out.push_str(&format!("mode = \"{}\"\n", mode));
            out
        };

        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(&config_path, updated.as_bytes()) {
            warn!("Failed to write wallpaper to config: {}", e);
        } else {
            info!(
                "Saved wallpaper {:?} mode={} to {:?}",
                path, mode, config_path
            );
        }
    }

    /// Write (or update) the [cursor] section in config.toml. Both the theme
    /// and the size are persisted together so the section round-trips cleanly;
    /// the caller passes the full desired cursor state.
    pub fn save_cursor(theme: &str, size: u32) {
        let config_path = config_directory().join("config.toml");
        let raw = if config_path.exists() {
            fs::read_to_string(&config_path).unwrap_or_default()
        } else {
            String::new()
        };

        let out = set_cursor_in_toml(&raw, theme, size);

        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(&config_path, out.as_bytes()) {
            warn!("Failed to write cursor to config: {}", e);
        } else {
            info!(
                "Saved cursor theme={:?} size={} to {:?}",
                theme, size, config_path
            );
        }
    }

    /// Write (or update) the `idle_timeout_secs` key in the [general] section.
    /// Pass `None` to disable idle blanking (the key is removed). Live-applied
    /// by the compositor on `ReloadConfig`.
    pub fn save_idle_timeout(secs: Option<u64>) {
        let config_path = config_directory().join("config.toml");
        let raw = if config_path.exists() {
            fs::read_to_string(&config_path).unwrap_or_default()
        } else {
            String::new()
        };

        let out = set_idle_timeout_in_toml(&raw, secs);

        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(&config_path, out.as_bytes()) {
            warn!("Failed to write idle timeout to config: {}", e);
        } else {
            info!("Saved idle_timeout_secs={:?} to {:?}", secs, config_path);
        }
    }

    /// Write (or update) the [panel] section in config.toml with the current pinned apps.
    pub fn save_pinned_apps(apps: &[PinnedAppConfig]) {
        let config_path = config_directory().join("config.toml");
        let raw = if config_path.exists() {
            fs::read_to_string(&config_path).unwrap_or_default()
        } else {
            String::new()
        };

        let stripped = strip_toml_section(&raw, "panel");
        let mut out = stripped;
        if !out.ends_with('\n') && !out.is_empty() {
            out.push('\n');
        }
        out.push_str("\n[panel]\npinned = [\n");
        for app in apps {
            let icon_part = app
                .icon
                .as_deref()
                .map_or(String::new(), |i| format!(", icon = {:?}", i));
            out.push_str(&format!(
                "  {{ label = {:?}, program = {:?}{} }},\n",
                app.label, app.program, icon_part
            ));
        }
        out.push_str("]\n");

        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(&config_path, out.as_bytes()) {
            warn!("Failed to write pinned apps to config: {}", e);
        } else {
            info!("Saved {} pinned app(s) to {:?}", apps.len(), config_path);
        }
    }

    /// Mark one output as primary in config.toml while preserving the rest of
    /// each output section.
    pub fn save_primary_output(output_name: &str) {
        let output_name = output_name.trim();
        if output_name.is_empty() {
            warn!("Refusing to save empty primary output name");
            return;
        }

        let config_path = config_directory().join("config.toml");
        let raw = if config_path.exists() {
            fs::read_to_string(&config_path).unwrap_or_default()
        } else {
            String::new()
        };
        let updated = set_primary_output_in_toml(&raw, output_name);

        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(&config_path, updated.as_bytes()) {
            warn!("Failed to write primary output to config: {}", e);
        } else {
            info!(
                "Saved primary output {:?} to {:?}",
                output_name, config_path
            );
        }
    }

    /// Set the configured mode for one output while preserving the rest of the file.
    pub fn save_output_mode(
        output_name: &str,
        width: i32,
        height: i32,
        refresh_millihz: Option<i32>,
    ) {
        let output_name = output_name.trim();
        if output_name.is_empty() || width <= 0 || height <= 0 {
            warn!(
                "Refusing to save invalid output mode: output={:?} width={} height={}",
                output_name, width, height
            );
            return;
        }

        let config_path = config_directory().join("config.toml");
        let raw = if config_path.exists() {
            fs::read_to_string(&config_path).unwrap_or_default()
        } else {
            String::new()
        };
        let updated = set_output_mode_in_toml(&raw, output_name, width, height, refresh_millihz);

        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(&config_path, updated.as_bytes()) {
            warn!("Failed to write output mode to config: {}", e);
        } else {
            info!(
                "Saved output mode {}x{}@{:?} for {:?} to {:?}",
                width, height, refresh_millihz, output_name, config_path
            );
        }
    }

    /// Set the `scale` for one output, preserving the rest of the file.
    pub fn save_output_scale(output_name: &str, scale: f64) {
        let output_name = output_name.trim();
        if output_name.is_empty() || !scale.is_finite() || scale <= 0.0 {
            warn!(
                "Refusing to save invalid output scale: output={:?} scale={}",
                output_name, scale
            );
            return;
        }
        // Trim trailing zeros so 1.0 stays "1", 1.25 stays "1.25".
        let literal = format!("{}", scale);
        write_output_key(output_name, "scale", &literal, "scale");
    }

    /// Set the `transform` (rotation) for one output. `None` removes the key
    /// (back to normal orientation). Accepts "90"/"180"/"270"/"normal"/flips.
    pub fn save_output_transform(output_name: &str, transform: Option<&str>) {
        let output_name = output_name.trim();
        if output_name.is_empty() {
            warn!("Refusing to save output transform for empty output name");
            return;
        }
        match transform {
            Some(t) => {
                let literal = format!("{:?}", t); // quoted TOML string
                write_output_key(output_name, "transform", &literal, "transform");
            }
            None => remove_output_key(output_name, "transform"),
        }
    }

    /// Scan standard wallpaper directories; group resolution variants into one entry per pack.
    pub fn scan_wallpaper_dirs() -> Vec<WallpaperEntry> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let top_dirs: &[std::path::PathBuf] = &[
            std::path::PathBuf::from("/usr/share/wallpapers"),
            std::path::PathBuf::from("/usr/share/backgrounds"),
            std::path::PathBuf::from(format!("{}/Pictures", home)),
        ];
        let mut by_dir: std::collections::BTreeMap<std::path::PathBuf, Vec<(u64, String)>> =
            std::collections::BTreeMap::new();
        for dir in top_dirs {
            if dir.exists() {
                collect_images_by_dir(dir, &mut by_dir, 5);
            }
        }
        let mut entries: Vec<WallpaperEntry> = Vec::new();
        for (dir, mut files) in by_dir {
            if files.is_empty() {
                continue;
            }
            files.sort_by_key(|(sz, _)| *sz);
            let thumbnail_path = files[0].1.clone();
            let apply_path = files.last().unwrap().1.clone();
            let display_name = if files.len() == 1 {
                wallpaper_entry_display_name(&apply_path)
            } else {
                wallpaper_dir_display_name(&dir)
            };
            entries.push(WallpaperEntry {
                display_name,
                apply_path,
                thumbnail_path,
            });
        }
        entries.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        entries
    }
}
