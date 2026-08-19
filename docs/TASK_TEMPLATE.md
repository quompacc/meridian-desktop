# Task Template (Codex)

## Ziel
Kurz und eindeutig: gewünschtes Ergebnis, betroffener Bereich, kein Scope-Creep.

## Erlaubte Änderungen
- Konkrete Dateien/Module
- Ob Tests angepasst/ergänzt werden dürfen
- Ob Logging erlaubt ist

## Nicht erlaubt
- Keine Feature-Erweiterungen
- Keine Dependency-/Cargo-Änderungen
- Keine Architekturwechsel außerhalb Scope

## Erfolgskriterium
- Funktionales Ziel erreicht
- Keine Regression in benachbarten Pfaden
- Definierte Tests/Checks grün
- Zielarchitektur und aktueller Implementierungsstand nicht vermischt

## Checks
1. `cargo test --workspace`
2. Optional: gezielte crate-spezifische Tests
3. Optional: manuelle Ablaufprüfung (nennen)
4. Bei Web/UI: Token-Guard, Bridge-/Component-Tests, Cache-/Idle-Modell
5. Bei BSD: OS-Version, Hardware und reales vs. VM-Ergebnis nennen

## Berichtformat
1. Geänderte Dateien
2. Änderung je Datei (knapp)
3. Verifikation
4. Offene Risiken/Annahmen

