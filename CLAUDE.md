# Meridian – Regeln für Claude Code

## Pflichtregeln bei JEDER Änderung:
1. Für jede neue Funktion mindestens einen Unit-Test
2. cargo test --workspace muss grün bleiben
3. Keine Heap-Allokation im Render-Loop
4. Kein Clone() von Theme-Daten im Render-Loop
5. Keine externen Abhängigkeiten für Kernfunktionen
6. Alles was zum Desktop gehört wird eingebettet

## Vor jedem Commit:
- cargo test --workspace grün
- cargo clippy -- -D warnings grün
