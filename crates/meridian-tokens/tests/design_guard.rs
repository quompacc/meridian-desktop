//! Design-Zentralitäts-Guard (GUI_CENTRALIZATION_PLAN.md, Phase 4 + §9 DoD).
//!
//! Dieser Test scannt den gesamten Render-Code des Workspace und schlägt fehl,
//! sobald ein hartverdrahteter Farb-/Alpha-/Mix-Wert auftaucht, der NICHT aus
//! der zentralen Design-Quelle (`meridian-tokens` / `meridian-config`) stammt.
//!
//! Ziel: „erledigt" ist kein Wort mehr, sondern ein Testergebnis. Solange dieser
//! Test grün ist, kann kein neuer Hardcode (egal von welchem Bearbeiter)
//! einsickern, ohne dass `cargo test --workspace` rot wird.
//!
//! PRÄZISION ist Pflicht: der Guard darf NICHT bei legitimem Code Alarm schlagen
//! (Theme-Kanäle wie `color.r/g/b`, reine Variablen, transparente Clears,
//! token-abgeleitete Konstanten). Nur echte Hardcodes sind Funde.
//!
//! Klassifizierung der Funde:
//! - `literal color`: alle RGB-Kanäle sind Zahlen-Literale (falsche, nicht
//!   theme-fähige Farbe; auch Schwarz/Weiß-Füllungen).
//! - `literal alpha`: Kanäle aus Theme/Variablen, aber Alpha ist ein
//!   Zahlen-Literal > 0 (hartverdrahtete Deckkraft).
//! - `raw .lerp() factor`: `x.lerp(y, 0.16)` mit Literal-Faktor statt Token.
//! - `raw *ALPHA* const`: `const …ALPHA… = <Zahl>` ohne Token-Bezug.
//!
//! Zwei Ausnahme-Mechanismen — bewusst, sichtbar, begründet:
//! - `guard:allow-file: <grund>` irgendwo in der Datei → ganze Datei erlaubt
//!   (nur für die KANONISCHEN Definitionsdateien = die Single-Source selbst).
//! - `guard:allow: <grund>` auf der Fund-Zeile oder der Zeile davor.
//!
//! Test-Code (`#[cfg(test)]` / `#[test]`-Blöcke) wird automatisch ignoriert.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct Violation {
    file: String,
    line: usize,
    kind: &'static str,
    snippet: String,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn collect_rs_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(root.join("crates")) {
        for e in entries.flatten() {
            let src = e.path().join("src");
            if src.is_dir() {
                walk(&src, &mut out);
            }
        }
    }
    out.sort();
    out
}

fn collect_web_asset_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let assets = root.join("crates/meridian-ui-runtime/assets");
    if assets.is_dir() {
        walk_web_assets(&assets, &mut files);
    }
    files.sort();
    files
}

fn walk_web_assets(directory: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_web_assets(&path, files);
                continue;
            }
            let extension = path.extension().and_then(|value| value.to_str());
            if matches!(extension, Some("css" | "html" | "js" | "ts")) {
                files.push(path);
            }
        }
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().map(|x| x == "rs").unwrap_or(false) {
                out.push(p);
            }
        }
    }
}

/// Byte-Bereiche der `#[cfg(test)]`-/`#[test]`-Blöcke (werden ignoriert).
fn test_byte_ranges(src: &str) -> Vec<(usize, usize)> {
    let bytes = src.as_bytes();
    let mut ranges = Vec::new();
    for marker in ["#[cfg(test)]", "#[test]"] {
        let mut from = 0;
        while let Some(rel) = src[from..].find(marker) {
            let at = from + rel;
            if let Some(open_rel) = src[at..].find('{') {
                let open = at + open_rel;
                let mut depth = 0i32;
                let mut close = src.len();
                for (i, &b) in bytes[open..].iter().enumerate() {
                    match b {
                        b'{' => depth += 1,
                        b'}' => {
                            depth -= 1;
                            if depth == 0 {
                                close = open + i;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                ranges.push((at, close));
            }
            from = at + marker.len();
        }
    }
    ranges
}

fn in_ranges(off: usize, ranges: &[(usize, usize)]) -> bool {
    ranges.iter().any(|&(a, b)| off >= a && off <= b)
}

fn line_of(src: &str, off: usize) -> usize {
    src[..off].bytes().filter(|&b| b == b'\n').count() + 1
}

/// Ist `s` ein reines Zahlen-Literal (dezimal, hex, float, mit optionalem
/// `as`-Cast oder Typ-Suffix)? `pal.text.r`, `r`, `alpha` → false.
fn is_num_literal(s: &str) -> bool {
    let mut s = s.trim();
    if let Some(i) = s.find(" as ") {
        s = s[..i].trim();
    }
    // Bekanntes Typ-Suffix abschneiden (NICHT beliebige Buchstaben — sonst
    // fressen wir die Hex-Ziffern A–F von z. B. `0xFF`).
    for suf in [
        "usize", "isize", "u128", "i128", "u64", "i64", "u32", "i32", "u16", "i16", "u8", "i8",
        "f64", "f32",
    ] {
        if let Some(t) = s.strip_suffix(suf) {
            s = t.trim();
            break;
        }
    }
    if s.is_empty() {
        return false;
    }
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit() || b == b'_');
    }
    s.parse::<f64>().is_ok()
}

/// Argumente eines Aufrufs ab der öffnenden Klammer (balanciert, mehrzeilig).
fn call_args(src: &str, open_paren: usize) -> Vec<String> {
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut cur = String::new();
    let mut args = Vec::new();
    for &b in &bytes[open_paren..] {
        let c = b as char;
        match c {
            '(' => {
                depth += 1;
                if depth > 1 {
                    cur.push(c);
                }
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    if !cur.trim().is_empty() {
                        args.push(cur.trim().to_string());
                    }
                    return args;
                }
                cur.push(c);
            }
            ',' if depth == 1 => {
                args.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    args
}

/// Sucht alle Vorkommen eines Patterns und ruft `f(offset_der_klammer)`.
fn for_each_call(src: &str, pat: &str, mut f: impl FnMut(usize)) {
    let mut from = 0;
    while let Some(rel) = src[from..].find(pat) {
        let at = from + rel;
        let paren = at + pat.len() - 1; // pat endet auf '('
        f(paren);
        from = at + pat.len();
    }
}

fn contains_hex_color(line: &str) -> bool {
    let bytes = line.as_bytes();
    for start in 0..bytes.len() {
        if bytes[start] != b'#' {
            continue;
        }
        let digits = bytes[start + 1..]
            .iter()
            .take_while(|byte| byte.is_ascii_hexdigit())
            .count();
        if matches!(digits, 3 | 4 | 6 | 8) {
            return true;
        }
    }
    false
}

fn contains_literal_dimension(line: &str) -> bool {
    let bytes = line.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if !byte.is_ascii_digit() {
            continue;
        }
        if index > 0 && (bytes[index - 1].is_ascii_alphanumeric() || bytes[index - 1] == b'-') {
            continue;
        }
        let number_end = bytes[index..]
            .iter()
            .position(|byte| !byte.is_ascii_digit() && *byte != b'.')
            .map_or(bytes.len(), |offset| index + offset);
        let unit = &line[number_end..];
        if unit.starts_with("px") || unit.starts_with("rem") || unit.starts_with("em") {
            return true;
        }
    }
    false
}

#[test]
fn no_hardcoded_design_values_outside_central_source() {
    let root = workspace_root();
    let files = collect_rs_files(&root);
    assert!(!files.is_empty(), "no source files under crates/*/src");

    let mut violations: Vec<Violation> = Vec::new();

    for path in &files {
        let Ok(src) = fs::read_to_string(path) else {
            continue;
        };
        if src.contains("guard:allow-file") {
            continue;
        }
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let test_ranges = test_byte_ranges(&src);
        let line_starts: Vec<usize> = {
            let mut v = vec![0usize];
            for (i, b) in src.bytes().enumerate() {
                if b == b'\n' {
                    v.push(i + 1);
                }
            }
            v
        };
        let line_text = |ln: usize| -> &str {
            let start = line_starts[ln - 1];
            let end = src[start..]
                .find('\n')
                .map(|e| start + e)
                .unwrap_or(src.len());
            &src[start..end]
        };

        let push = |off: usize, kind: &'static str, viol: &mut Vec<Violation>| {
            if in_ranges(off, &test_ranges) {
                return;
            }
            let ln = line_of(&src, off);
            let this = line_text(ln);
            if this.contains("guard:allow") {
                return;
            }
            // Scan the contiguous comment block directly above for a guard:allow.
            let mut k = ln;
            while k > 1 {
                let above = line_text(k - 1);
                if above.trim_start().starts_with("//") {
                    if above.contains("guard:allow") {
                        return;
                    }
                    k -= 1;
                } else {
                    break;
                }
            }
            // Kommentarzeile? (Fund liegt hinter `//`)
            let col = off - line_starts[ln - 1];
            if let Some(cidx) = this.find("//") {
                if col > cidx {
                    return;
                }
            }
            viol.push(Violation {
                file: rel.clone(),
                line: ln,
                kind,
                snippet: this.trim().chars().take(110).collect(),
            });
        };

        // --- Farb-Konstruktoren -------------------------------------------
        for pat in ["paint_rgba(", "from_rgba8(", "::rgba(", "::rgb("] {
            for_each_call(&src, pat, |paren| {
                let args = call_args(&src, paren);
                if args.len() < 3 {
                    return;
                }
                let has_alpha = args.len() >= 4;
                let channels = &args[..3];
                let channels_literal = channels.iter().all(|a| is_num_literal(a));
                let alpha_literal = has_alpha && is_num_literal(&args[3]);
                let alpha_zero = has_alpha
                    && args[3]
                        .trim()
                        .trim_end_matches(|c: char| c.is_ascii_alphabetic())
                        == "0";

                if alpha_zero {
                    return; // vollständig transparent → designneutral
                }
                if channels_literal {
                    push(paren, "literal color", &mut violations);
                } else if alpha_literal {
                    push(paren, "literal alpha", &mut violations);
                }
            });
        }

        // --- rohe .lerp(x, FAKTOR) ----------------------------------------
        for_each_call(&src, ".lerp(", |paren| {
            let args = call_args(&src, paren);
            if let Some(last) = args.last() {
                if is_num_literal(last) {
                    push(paren, "raw .lerp() factor", &mut violations);
                }
            }
        });

        // --- rohe *ALPHA*-Konstanten --------------------------------------
        for (i, start) in line_starts.iter().enumerate() {
            let ln = i + 1;
            let text = line_text(ln);
            let t = text.trim_start();
            let is_const = t.starts_with("const ")
                || t.starts_with("pub const ")
                || t.starts_with("pub(crate) const ");
            if is_const && text.contains("ALPHA") {
                if let Some(eq) = text.find('=') {
                    let rhs = text[eq + 1..].trim().trim_end_matches(';').trim();
                    // Token-abgeleitet (Elevation::…, Interaction::…, ::…)? → ok
                    if !rhs.contains("::") && is_num_literal(rhs) {
                        push(*start, "raw *ALPHA* const", &mut violations);
                    }
                }
            }
        }
    }

    if violations.is_empty() {
        return;
    }

    // Bericht: nach Art, dann Datei.
    let mut by_kind: BTreeMap<&str, Vec<&Violation>> = BTreeMap::new();
    for v in &violations {
        by_kind.entry(v.kind).or_default().push(v);
    }
    let mut report = format!(
        "\n\n=== DESIGN-GUARD: {} echte Hardcodes ===\n",
        violations.len()
    );
    for (kind, vs) in &by_kind {
        report.push_str(&format!("\n## {} ({})\n", kind, vs.len()));
        let mut by_file: BTreeMap<&str, Vec<&&Violation>> = BTreeMap::new();
        for v in vs {
            by_file.entry(v.file.as_str()).or_default().push(v);
        }
        for (file, fvs) in &by_file {
            report.push_str(&format!("  {}\n", file));
            for v in fvs {
                report.push_str(&format!("    L{:<5} {}\n", v.line, v.snippet));
            }
        }
    }
    report.push_str(
        "\nJeder Fund: zentralisieren (Theme/Interaction/Elevation/Decorations) \
         ODER `// guard:allow: <grund>` mit Begründung.\n",
    );
    panic!("{report}");
}

#[test]
fn web_assets_use_generated_design_tokens_only() {
    let root = workspace_root();
    let files = collect_web_asset_files(&root);
    assert!(!files.is_empty(), "no Web UI assets found to guard");

    let mut violations = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        if source.contains("guard:allow-file") {
            continue;
        }
        for (index, line) in source.lines().enumerate() {
            if line.contains("guard:allow") {
                continue;
            }
            let trimmed = line.trim();
            let reason = if contains_hex_color(trimmed) {
                Some("literal color")
            } else if ["rgb(", "rgba(", "hsl(", "hsla(", "color-mix("]
                .iter()
                .any(|pattern| trimmed.contains(pattern))
            {
                Some("local color function")
            } else if contains_literal_dimension(trimmed) {
                Some("literal geometry")
            } else if trimmed.starts_with("opacity:")
                && trimmed
                    .trim_start_matches("opacity:")
                    .trim()
                    .trim_end_matches(';')
                    .parse::<f32>()
                    .is_ok()
            {
                Some("literal opacity")
            } else if trimmed.contains(" style=") {
                Some("inline style")
            } else {
                None
            };
            if let Some(reason) = reason {
                let relative = path.strip_prefix(&root).unwrap_or(&path);
                violations.push(format!(
                    "{}:{}: {reason}: {}",
                    relative.display(),
                    index + 1,
                    trimmed
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Web UI design values must come from generated tokens:\n{}",
        violations.join("\n")
    );
}
