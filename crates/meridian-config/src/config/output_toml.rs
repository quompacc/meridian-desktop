fn strip_toml_section(raw: &str, section: &str) -> String {
    let header = format!("[{}]", section);
    let mut out = String::new();
    let mut in_target = false;
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with('[') && !t.starts_with("[[") {
            in_target = t == header;
        }
        if !in_target {
            out.push_str(line);
            out.push('\n');
        }
    }
    let trimmed = out.trim_end_matches('\n').to_string();
    if trimmed.is_empty() {
        trimmed
    } else {
        trimmed + "\n"
    }
}

/// Replace (or append) the [cursor] section with the given theme and size,
/// preserving the rest of the file. Pure string transform so it can be tested
/// without touching the real config directory.
fn set_cursor_in_toml(raw: &str, theme: &str, size: u32) -> String {
    let mut out = strip_toml_section(raw, "cursor");
    if !out.ends_with('\n') && !out.is_empty() {
        out.push('\n');
    }
    out.push_str("\n[cursor]\n");
    out.push_str(&format!("theme = \"{}\"\n", theme));
    out.push_str(&format!("size = {}\n", size));
    out
}

fn set_output_mode_in_toml(
    raw: &str,
    output_name: &str,
    width: i32,
    height: i32,
    refresh_millihz: Option<i32>,
) -> String {
    let mut out = String::new();
    let mut current_output: Option<String> = None;
    let mut current_output_has_mode = false;
    let mut found_target_output = false;

    for line in raw.lines() {
        if let Some(next_output) = output_section_name(line) {
            if current_output.as_deref() == Some(output_name) && !current_output_has_mode {
                push_output_mode_line(&mut out, width, height, refresh_millihz);
            }
            found_target_output |= next_output == output_name;
            current_output = Some(next_output);
            current_output_has_mode = false;
            out.push_str(line);
            out.push('\n');
            continue;
        }

        let trimmed = line.trim();
        if current_output.as_deref() == Some(output_name)
            && trimmed.starts_with('[')
            && !trimmed.starts_with("[[")
        {
            if !current_output_has_mode {
                push_output_mode_line(&mut out, width, height, refresh_millihz);
            }
            current_output = None;
            current_output_has_mode = false;
        }

        if current_output.as_deref() == Some(output_name) && is_mode_key_line(line) {
            push_output_mode_line(&mut out, width, height, refresh_millihz);
            current_output_has_mode = true;
            continue;
        }

        out.push_str(line);
        out.push('\n');
    }

    if current_output.as_deref() == Some(output_name) && !current_output_has_mode {
        push_output_mode_line(&mut out, width, height, refresh_millihz);
    }

    if !found_target_output {
        if !out.ends_with('\n') && !out.is_empty() {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(&format!("[outputs.{:?}]\n", output_name));
        out.push_str("enabled = true\n");
        out.push_str("position = \"auto\"\n");
        push_output_mode_line(&mut out, width, height, refresh_millihz);
    }

    out
}

fn set_primary_output_in_toml(raw: &str, output_name: &str) -> String {
    let mut out = String::new();
    let mut current_output: Option<String> = None;
    let mut current_output_has_primary = false;
    let mut found_target_output = false;

    for line in raw.lines() {
        if let Some(next_output) = output_section_name(line) {
            if let Some(previous_output) = current_output.take() {
                if !current_output_has_primary {
                    push_primary_line(&mut out, &previous_output, output_name);
                }
            }
            found_target_output |= next_output == output_name;
            current_output = Some(next_output);
            current_output_has_primary = false;
            out.push_str(line);
            out.push('\n');
            continue;
        }

        let trimmed = line.trim();
        if current_output.is_some() && trimmed.starts_with('[') && !trimmed.starts_with("[[") {
            if let Some(previous_output) = current_output.take() {
                if !current_output_has_primary {
                    push_primary_line(&mut out, &previous_output, output_name);
                }
            }
            current_output_has_primary = false;
        }

        if let Some(ref name) = current_output {
            if is_primary_key_line(line) {
                push_primary_line(&mut out, name, output_name);
                current_output_has_primary = true;
                continue;
            }
        }

        out.push_str(line);
        out.push('\n');
    }

    if let Some(previous_output) = current_output {
        if !current_output_has_primary {
            push_primary_line(&mut out, &previous_output, output_name);
        }
    }

    if !found_target_output {
        if !out.ends_with('\n') && !out.is_empty() {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(&format!("[outputs.{:?}]\n", output_name));
        out.push_str("primary = true\n");
        out.push_str("enabled = true\n");
        out.push_str("position = \"auto\"\n");
    }

    out
}

fn output_section_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.starts_with("[[") || !trimmed.starts_with("[outputs.") || !trimmed.ends_with(']') {
        return None;
    }
    let raw_name = trimmed.strip_prefix("[outputs.")?.strip_suffix(']')?.trim();
    if raw_name.is_empty() {
        return None;
    }
    if raw_name.starts_with('"') && raw_name.ends_with('"') && raw_name.len() >= 2 {
        return Some(raw_name[1..raw_name.len() - 1].replace("\\\"", "\""));
    }
    Some(raw_name.to_string())
}

fn is_mode_key_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix("mode") else {
        return false;
    };
    rest.trim_start().starts_with('=')
}

fn is_primary_key_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix("primary") else {
        return false;
    };
    rest.trim_start().starts_with('=')
}

fn push_output_mode_line(out: &mut String, width: i32, height: i32, refresh_millihz: Option<i32>) {
    out.push_str("mode = { width = ");
    out.push_str(&width.to_string());
    out.push_str(", height = ");
    out.push_str(&height.to_string());
    if let Some(refresh) = refresh_millihz {
        out.push_str(", refresh_millihz = ");
        out.push_str(&refresh.to_string());
    }
    out.push_str(" }\n");
}

fn push_primary_line(out: &mut String, current_output: &str, primary_output: &str) {
    out.push_str("primary = ");
    out.push_str(if current_output == primary_output {
        "true"
    } else {
        "false"
    });
    out.push('\n');
}

/// Read-modify-write a single `<key> = <literal>` inside `[outputs."<name>"]`,
/// shared by save_output_scale/transform.
fn write_output_key(output_name: &str, key: &str, value_literal: &str, log_what: &str) {
    let config_path = config_directory().join("config.toml");
    let raw = if config_path.exists() {
        fs::read_to_string(&config_path).unwrap_or_default()
    } else {
        String::new()
    };
    let updated = set_output_key_in_toml(&raw, output_name, key, value_literal);
    if let Some(parent) = config_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Err(e) = fs::write(&config_path, updated.as_bytes()) {
        warn!("Failed to write output {} to config: {}", log_what, e);
    } else {
        info!(
            "Saved output {} = {} for {:?} to {:?}",
            key, value_literal, output_name, config_path
        );
    }
}

fn remove_output_key(output_name: &str, key: &str) {
    let config_path = config_directory().join("config.toml");
    let Ok(raw) = fs::read_to_string(&config_path) else {
        return; // nothing to remove
    };
    let updated = remove_output_key_in_toml(&raw, output_name, key);
    if let Err(e) = fs::write(&config_path, updated.as_bytes()) {
        warn!("Failed to remove output {} from config: {}", key, e);
    } else {
        info!("Removed output {} for {:?}", key, output_name);
    }
}

/// True if the trimmed line is `<key> = …` (exact key, not a prefix).
fn is_output_key_line(line: &str, key: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix(key) else {
        return false;
    };
    rest.trim_start().starts_with('=')
}

/// Replace (or insert) `<key> = <value_literal>` inside the target output's
/// `[outputs."<name>"]` section, preserving everything else. Appends a new
/// section if the output is absent. Mirrors set_output_mode_in_toml.
fn set_output_key_in_toml(raw: &str, output_name: &str, key: &str, value_literal: &str) -> String {
    let key_line = format!("{} = {}\n", key, value_literal);
    let mut out = String::new();
    let mut current_output: Option<String> = None;
    let mut current_has_key = false;
    let mut found_target = false;

    for line in raw.lines() {
        if let Some(next_output) = output_section_name(line) {
            if current_output.as_deref() == Some(output_name) && !current_has_key {
                out.push_str(&key_line);
            }
            found_target |= next_output == output_name;
            current_output = Some(next_output);
            current_has_key = false;
            out.push_str(line);
            out.push('\n');
            continue;
        }

        let trimmed = line.trim();
        if current_output.as_deref() == Some(output_name)
            && trimmed.starts_with('[')
            && !trimmed.starts_with("[[")
        {
            if !current_has_key {
                out.push_str(&key_line);
            }
            current_output = None;
            current_has_key = false;
        }

        if current_output.as_deref() == Some(output_name) && is_output_key_line(line, key) {
            out.push_str(&key_line);
            current_has_key = true;
            continue;
        }

        out.push_str(line);
        out.push('\n');
    }

    if current_output.as_deref() == Some(output_name) && !current_has_key {
        out.push_str(&key_line);
    }

    if !found_target {
        if !out.ends_with('\n') && !out.is_empty() {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(&format!("[outputs.{:?}]\n", output_name));
        out.push_str("enabled = true\n");
        out.push_str("position = \"auto\"\n");
        out.push_str(&key_line);
    }

    out
}

/// Drop a `<key> = …` line from the target output's section. No-op if absent.
fn remove_output_key_in_toml(raw: &str, output_name: &str, key: &str) -> String {
    let mut out = String::new();
    let mut current_output: Option<String> = None;

    for line in raw.lines() {
        if let Some(next_output) = output_section_name(line) {
            current_output = Some(next_output);
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let trimmed = line.trim();
        if trimmed.starts_with('[') && !trimmed.starts_with("[[") {
            current_output = None;
        }
        if current_output.as_deref() == Some(output_name) && is_output_key_line(line, key) {
            continue; // skip the key line
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}
