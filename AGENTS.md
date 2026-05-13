# Meridian Agent Rules

## Projekt (Kurz)
Meridian ist ein Wayland-Compositor mit separatem Shell-Prozess. Workspace ist Rust-Multi-Crate mit `meridian-compositor`, `meridian-shell`, `meridian-config`, `meridian-ipc`, `meridian-wm`, `meridian-portal`.

## Harte Regeln für Codex
1. Keine Feature-Änderung ohne expliziten Auftrag.
2. Bestehende Architekturpfade respektieren (`main -> backend -> state -> handlers/render`).
3. Keine stillen API-Brüche zwischen Crates.
4. Refactors nur modular, verhaltensgleich, klein und testbar.
5. Große Dateien in Module splitten statt Logik neu schreiben.
6. Render order is part of correctness. Do not reorder render elements unless the task explicitly requires it and the visual stacking rules are preserved.
7. Visual quality matters, but not at the cost of idle CPU/GPU usage.
8. Prefer cached visual assets over per-frame recomputation.
9. Do not add animations, blur, shadows, or icon decoding without cache/invalidation strategy.
10. Every visual feature must explain its performance model.
11. After every Rust code change, run at least `cargo check --workspace`.
12. If tests were added or changed, run `cargo test --workspace`.
13. For formatting-sensitive Rust changes, run `cargo fmt`.
14. A task with Rust changes is not complete until the check/test results are reported.

## Erlaubt
- Rust-Code in betroffenen Modulen ändern.
- Tests ergänzen/aktualisieren.
- Dokumentation und Projektregeln pflegen.
- Logging ergänzen, wenn zur Diagnose nötig.

## Verboten
- `Cargo.toml`-Änderungen ohne Auftrag.
- Neue Dependencies ohne Auftrag.
- Destruktive Git-Operationen.
- Unbegründete Performance-Regressionen.
- “Fixes” ohne `cargo test --workspace` (außer explizit untersagt).

## Pflichtchecks
1. Nach Rust-Codeänderung mindestens `cargo check --workspace`.
2. Bei Test-/Logikänderungen zusätzlich `cargo test --workspace`.
3. Bei formatierungssensitiven Änderungen `cargo fmt`.
4. Bei Rendering/Input/IPC: betroffene Pfade manuell gegen Call-Flow prüfen.

## Berichtformat
1. Geänderte Dateien.
2. Was geändert wurde (pro Datei, 1-3 Zeilen).
3. Verifikation (Befehle + Ergebnis).
4. Offene Risiken/Annahmen.
