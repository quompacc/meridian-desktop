//! Phase 4 guard against centralization regressions
//! (see `docs/GUI_CENTRALIZATION_PLAN.md` §6).
//!
//! Hover/pressed treatments must route through `meridian_tokens::Interaction`
//! (`Interaction::DEFAULT.hover()/.pressed()`), never a hand-rolled
//! `base.lerp(white/black, x)` inside a `WidgetState::Hovered`/`Pressed` match
//! arm. This scan walks the shell + ui render sources and fails if such a
//! raw lerp reappears in a hover/pressed arm — single-line *or* the multi-line
//! `=> theme\n .palette\n .surface\n .lerp(..)` form.
//!
//! Legitimate `.lerp(...)` stays allowed everywhere else: idle/selected,
//! focused/minimized and accent/error tints are state colours, not interaction
//! states, and icon material art is exempt by the plan's Definition of Done.

use std::fs;
use std::path::{Path, PathBuf};

/// Collect the source text of a `WidgetState::Hovered|Pressed => ...` arm that
/// begins on `lines[start]`, appending continuation lines until the arm's
/// terminating comma (capped, so a malformed file cannot run away).
fn arm_text(lines: &[&str], start: usize) -> String {
    let mut buf = String::new();
    for line in lines.iter().skip(start).take(8) {
        buf.push_str(line);
        buf.push('\n');
        // The arm ends at the first line carrying its trailing comma.
        if line.trim_end().ends_with(',') {
            break;
        }
    }
    buf
}

fn scan_file(path: &Path, hits: &mut Vec<String>) {
    let Ok(src) = fs::read_to_string(path) else {
        return;
    };
    let lines: Vec<&str> = src.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let is_state_arm = trimmed.starts_with("WidgetState::Hovered")
            || trimmed.starts_with("WidgetState::Pressed");
        if is_state_arm && line.contains("=>") && arm_text(&lines, i).contains(".lerp(") {
            hits.push(format!("{}:{}: {}", path.display(), i + 1, trimmed.trim_end()));
        }
    }
}

fn scan_dir(dir: &Path, hits: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, hits);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            scan_file(&path, hits);
        }
    }
}

#[test]
fn hover_pressed_states_use_interaction_not_raw_lerp() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let roots = [
        manifest.join("src"),
        manifest.join("..").join("meridian-ui").join("src"),
    ];

    let mut hits = Vec::new();
    for root in &roots {
        scan_dir(root, &mut hits);
    }

    assert!(
        hits.is_empty(),
        "Hover/Pressed must use Interaction::DEFAULT.hover()/.pressed(), not a raw \
         lerp. Offending arms:\n{}",
        hits.join("\n")
    );
}
