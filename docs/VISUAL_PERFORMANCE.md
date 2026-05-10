# Visual Performance

## Grundprinzip
High-end visuals through cached, event-driven rendering.

## Optik-Ziele
- Saubere Kanten
- Gute Typografie
- Weiche Schatten nur wenn gecached
- Dezente Transparenz
- Konsistente Abstände
- Hochwertige Icons
- Kein unnötiger visueller Lärm

## Performance-Regeln
- Keine Animation ohne Frame-Budget.
- Keine dauerhaften Animationen im Idle.
- Keine Blur-Effekte im Hotpath.
- Keine Schatten pro Frame neu berechnen.
- Keine Textlayouts pro Frame neu berechnen.
- Keine SVG-/PNG-Decodes im Render-Loop.
- Keine Fontsuche im Render-Loop.
- Keine Theme-/Color-Parsing im Render-Loop.
- Keine neuen Buffers pro Frame.

## Erlaubte Optik-Techniken
- Cached rounded rectangles
- Cached shadow textures
- Precomputed gradients
- Glyph/text layout cache
- Icon atlas
- Dirty-region rendering
- Static wallpaper cache
- Scale-aware assets

## Animation-Regeln
- Nur event-getrieben.
- Kurze Dauer.
- Abschaltbar.
- Kein Idle-Wakeup.
- Ziel 60 FPS nur während Animation.
- Danach wieder idle.

## RAM-Regeln
- Kleine Caches mit Limits.
- LRU nur wenn nötig.
- Keine unbounded caches.
- Assets in passenden Größen laden.

## GPU/NVIDIA-Regeln
- GBM/linux-dmabuf freundlich.
- Keine exotischen Renderpfade.
- Möglichst wenig Kopien.
- Keine unnötigen CPU-GPU-Roundtrips.

## Review-Checklist
1. Was wird gecached?
2. Was invalidiert den Cache?
3. Was passiert im Idle?
4. Welche Allokationen entstehen pro Frame?
5. Wie viel RAM kostet das Feature?
6. Ist die Optik skalierungsfähig?
