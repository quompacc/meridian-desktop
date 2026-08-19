//! Keeps Rust modules small enough to review and maintain safely.

use std::fs;
use std::path::{Path, PathBuf};

const MAX_RUST_LINES: usize = 600;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("meridian-tokens must live below the workspace root")
        .to_path_buf()
}

fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("cannot scan {}: {error}", directory.display()))
        .map(|entry| entry.expect("directory entry must be readable").path())
        .collect();
    entries.sort();

    for path in entries {
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

#[test]
fn rust_source_files_stay_within_line_limit() {
    let root = workspace_root();
    let mut files = Vec::new();
    collect_rust_files(&root.join("crates"), &mut files);

    let mut oversized = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let line_count = source.lines().count();
        if line_count > MAX_RUST_LINES {
            let relative = path.strip_prefix(&root).unwrap_or(&path);
            oversized.push(format!("{}: {line_count}", relative.display()));
        }
    }

    assert!(
        oversized.is_empty(),
        "Rust source files may contain at most {MAX_RUST_LINES} physical lines:\n{}",
        oversized.join("\n")
    );
}
