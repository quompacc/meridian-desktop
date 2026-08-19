fn placement(
    name: &str,
    position: OutputPosition,
    primary: bool,
    enabled: bool,
) -> OutputPlacement {
    OutputPlacement {
        name: name.to_string(),
        position,
        primary,
        enabled,
    }
}

fn connected(name: &str, width: i32, height: i32) -> ConnectedOutput {
    ConnectedOutput {
        name: name.to_string(),
        width,
        height,
    }
}

fn expected_output(
    name: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    primary: bool,
    enabled: bool,
) -> ResolvedOutput {
    ResolvedOutput {
        name: name.to_string(),
        x,
        y,
        width,
        height,
        primary,
        enabled,
    }
}

fn config_entry(
    name: &str,
    position: OutputPositionConfig,
    primary: bool,
    enabled: bool,
) -> OutputEntry {
    let mut entry = OutputEntry::defaults_for(name);
    entry.position = position;
    entry.primary = primary;
    entry.enabled = enabled;
    entry
}

fn config_entry_with_mode(
    name: &str,
    mode: Option<OutputModeConfig>,
    enabled: bool,
) -> OutputEntry {
    let mut entry = OutputEntry::defaults_for(name);
    entry.mode = mode;
    entry.enabled = enabled;
    entry
}

fn mode(width: i32, height: i32, refresh_millihz: Option<i32>) -> OutputModeConfig {
    OutputModeConfig {
        width,
        height,
        refresh_millihz,
    }
}
