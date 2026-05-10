# Performance Rules

Siehe ergänzend: `docs/VISUAL_PERFORMANCE.md`.

## Render-Loop Regeln
1. Keine neuen Heap-Allokationen im Hot Path (`render_outputs`, `WinitEvent::Redraw`).
2. Kein `Theme`-Clone im Render-Loop; nur Referenzen/abgeleitete primitive Werte.
3. Keine synchronen I/O-Operationen im Render-Loop.
4. Render-Reihenfolge ist Korrektheit, nicht nur Performance: Optimierungen dürfen Z-Order nicht brechen.
5. High quality visuals must be implemented with caching and dirty invalidation.
6. Idle desktop must stay idle.

## Dirty-Flag-Regel
- Recompute/Reupload nur bei echten Änderungen:
  - Größe
  - Theme/Source-Key
  - explizites `dirty`
- Dirty-Flag-Optimierung darf Layer-Z-Order nicht verändern.

## SmallVec-Regel
- Für kleine, häufige Elementlisten `SmallVec` statt `Vec` (z. B. Decoration-Elemente).
- Größere Sammellisten nur einmal pro Frame aufbauen.

## Buffer-Reuse-Regel
- SHM/Texture/SlotPool wiederverwenden.
- Keine Neuallokation pro Frame, wenn Maße unverändert sind.

## Event-driven statt Polling
- Zustandswechsel über Events/Handler.
- Polling nur für klar begrenzte Aufgaben (z. B. IPC-Timer), mit moderatem Intervall.

## RAM-Ziele (Richtwerte)
- Keine ungebremsten per-frame Wachstumsstrukturen.
- Caches müssen ersetzbar/invalidierbar sein.
- Per-Output GPU-Caches nur solange nötig halten.

## Vor Merge prüfen
1. `cargo test --workspace`
2. Renderpfad-Diff auf Allokationen/Clones prüfen
3. Keine neuen Warnungen im geänderten Pfad
4. Für Rendering-Änderungen: beide Backends (`drm`, `winit`) gedanklich/technisch mitprüfen
