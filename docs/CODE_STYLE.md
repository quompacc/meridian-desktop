# Code Style

> Updated 2026-08-19: Rust remains the system/policy language. The target UI
> adds a small TypeScript/CSS layer governed by the same modularity, security and
> design-token rules.

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

## Plattform-Code
- Linux/OpenBSD/FreeBSD-Abhängigkeiten hinter kleinen, benannten Adaptern halten.
- Gemeinsame Semantik testen; OS-spezifische Sicherheits- und Sessionmodelle
  nicht durch scheinbar einheitliche, aber falsche Abstraktionen verstecken.
- Unsupported klar melden statt still auf Linux-Verhalten zurückzufallen.

## Web-UI
- Möglichst Web Components + CSS; TypeScript nur für State/Bridge/Interaktion.
- Keine Framework-Abhängigkeit ohne expliziten Architekturauftrag.
- Bridge-Typen versionieren und an der Rust-Grenze validieren.
- Keine lokalen Produktionsfarben/-radien/-alphas; nur generierte CSS-Tokens.
- Komponenten nach Verantwortung splitten; keine monolithischen Surface-Dateien.
- Keine synchronen Bridge-Aufrufe in Input-/Paint-Pfaden.
- Fallbackpfad (ohne Feature) muss kompilieren und getestet bleiben.

