# Code Style

## Rust-Stil
- Kleine, fokussierte Module.
- Frühzeitige Returns statt tiefer Verschachtelung.
- Explizite Typen dort, wo Lesbarkeit gewinnt.

## Modulgröße
- Ziel: ~50-200 Zeilen.
- Ab ~250 Zeilen aktiv splitten nach Verantwortung.

## Fehlerbehandlung
- `Result`/`Option` korrekt propagieren.
- `unwrap()` nur bei klaren Invarianten.
- Warn-Logs mit Kontext (was, wo, warum).

## Logging-Konvention
- `info!` für relevante Lebenszyklus-/State-Wechsel.
- `warn!` für degradierte Zustände/Fallbacks.
- `debug!` für Detaildiagnose.
- Keine Log-Spam-Schleifen ohne Begrenzung.

## Naming
- Dateinamen nach Verantwortung (`render`, `layer_shell`, `commands`, `broadcast`).
- Funktionsnamen als Verb + Objekt (`handle_*`, `render_*`, `broadcast_*`).

## Dependency-Regeln
- Keine neuen Crates ohne klaren Grund und expliziten Auftrag.
- Bestehende Utility-/State-Funktionen wiederverwenden statt Duplikate.

## Feature-Gates
- Optionales Verhalten klar hinter `#[cfg(feature = ...)]`.
- Fallbackpfad (ohne Feature) muss kompilieren und getestet bleiben.

