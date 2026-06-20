# Meridian – Regeln für Claude Code

## Pflichtregeln bei JEDER Änderung:
1. Für jede neue Funktion mindestens einen Unit-Test
2. cargo test --workspace muss grün bleiben
3. Keine Heap-Allokation im Render-Loop
4. Kein Clone() von Theme-Daten im Render-Loop
5. Keine externen Abhängigkeiten für Kernfunktionen
6. Alles was zum Desktop gehört wird eingebettet

## Design – VERBINDLICH (bei jeder UI-/Render-Änderung):
- `docs/meridian_design_manifest.md` ist die maßgebliche Design-Spezifikation.
  Bei Konflikt schlägt das Manifest jede andere Quelle.
- Eine zentrale Design-Quelle: `meridian-tokens` + `meridian-config`. KEIN
  hartverdrahteter Farb-/Alpha-/Radius-/Mix-Wert im Render-Code außerhalb davon
  (Ausnahmen nur via `// guard:allow: <grund>` / `guard:allow-file`).
- Genau 2 Themes (hell/dunkel), identisch bis auf Farben.
- Branding (Kompass) nur subtil in Login/Bootsplash, nie in Alltags-UI/Startbutton
  (Manifest §3.4/§9/§14).
- Guard grün halten: `cargo test -p meridian-tokens --test design_guard`.
- Definition of Done: `docs/GUI_CENTRALIZATION_PLAN.md` §9.

## Vor jedem Commit:
- cargo test --workspace grün
- cargo clippy -- -D warnings grün
- design_guard grün (`cargo test -p meridian-tokens --test design_guard`)
