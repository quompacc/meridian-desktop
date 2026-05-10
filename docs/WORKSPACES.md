# Workspace Policy (Multi-Monitor)

## Scope
Diese Datei spezifiziert die Ziel-Policy für Workspaces auf mehreren Outputs.  
Nur Spezifikation, keine Implementierung.

## Modelloptionen

### Option A: Global Active Workspace
- Ein globaler `active_workspace` für alle Outputs.
- `Super+N` schaltet alle Outputs gemeinsam um.
- Einfach zu implementieren, aber schwache Multi-Monitor-Ergonomie.

### Option B: Active Workspace per Output
- Jeder Output hat eigenen `active_workspace`.
- `Super+N` wirkt auf den fokussierten Output.
- Höchste Ergonomie, aber größerer Umbau (State, IPC, Panel, Fokuspfade).

### Option C: Hybrid (empfohlen)
- Es gibt weiterhin ein globales Workspace-Set (1..9).
- Zusätzlich pro Output eine aktive Workspace-View: `active_workspace_by_output`.
- Ein `focused_output` steuert input-getriebene Aktionen.
- Gute Ergonomie bei schrittweiser Migration aus dem aktuellen globalen Modell.

## Zielentscheidung für Meridian
Empfohlen: **Option C (Hybrid)**.

Begründung:
- Einfachheit: weniger riskant als sofort voll per-output (Option B).
- Performance: keine neue Render-Architektur nötig; bestehende OutputRegistry-Pfade bleiben nutzbar.
- Nutzererwartung: `Super+N` auf fokussiertem Monitor ist auf Multi-Monitor-Setups erwartbar.
- Ergonomie: unterschiedliche Workspaces pro Output möglich, ohne globalen Kontext zu verlieren.
- Umsetzbarkeit: passt zum aktuellen Code mit globalem WorkspaceManager und bereits migrierten Output-Auswahlpfaden.

## Invarianten
1. Ein Fenster gehört genau einem Workspace.
2. Ein Workspace darf auf mehreren Outputs gleichzeitig sichtbar sein (erlaubt).
3. Ein Output zeigt genau einen aktiven Workspace.
4. Fokus gehört zu genau einem Output und genau einem Surface/Fenster.
5. Input-getriebene Workspace-Aktionen verwenden `focused_output`.
6. Entfernen eines Outputs:
   - Zuordnung in `active_workspace_by_output` entfernen.
   - Falls entfernter Output fokussiert war: Fokus auf `primary` sonst `first`.
   - Fenster bleiben in ihren Workspaces; keine implizite Workspace-Migration.

## Keybinding-Policy
- `Super+1..9`:
  - wechselt den Workspace **nur auf dem fokussierten Output**.
  - kein globales Umschalten aller Outputs.
- `Super+Shift+1..9`:
  - verschiebt fokussiertes Fenster in Ziel-Workspace.
  - kein automatischer Output-Wechsel.
- Fenster verschieben zwischen Workspaces:
  - Workspace-Zuordnung ändert sich, Output-Zuordnung ergibt sich aus Sichtbarkeit/Mapping.
- Wenn kein fokussierter Output auflösbar:
  - Fallback `primary -> first`.

## Panel- und Layer-Shell-Policy
- Panel: pro Output ein Panel (Zielzustand), Startphase darf weiterhin primary-only sein.
- Workspace-Indikator:
  - zeigt aktiven Workspace des jeweiligen Outputs.
  - Occupied-Status bleibt global aus WindowSnapshot ableitbar.
- Layer-Shell-Placement:
  - expliziter Output bleibt vorrangig.
  - sonst `primary -> first` (bereits umgesetzt).

## Window Placement Policy
- Neue Fenster:
  - auf fokussiertem Output,
  - im aktiven Workspace dieses Outputs.
- Fullscreen/Maximize:
  - auf dem Output des Fensters; sonst `primary -> first`.
- Tiling:
  - pro Output-Rect des jeweiligen aktiven Workspaces,
  - kein globales `first-output`-Rect.

## Umsetzungsphasen
1. Datenmodell vorbereiten
   - `focused_output`
   - `active_workspace_by_output`
   - klare Fallback-Helfer (`primary -> first`)
2. `focused_output` einführen
   - bei Pointer/Keyboard-Fokus stabil pflegen
3. `active_workspace_by_output`
   - lesen/schreiben mit sicheren Fallbacks
4. Keybindings umstellen
   - `Super+1..9` auf fokussierten Output
5. Panel-Anzeige angleichen
   - pro Output aktiven Workspace anzeigen
6. Hotplug-Verhalten
   - remove/add, Fokus-Fallback, Mapping-Recovery

## Aktueller Fortschritt
- Phase 1 ist vorbereitet:
  - `WorkspaceOutputState` vorhanden.
  - `focused_output` und `active_workspace_by_output` werden synchronisiert.
  - Helper sind vorhanden (`focused_output`, `set_focused_output`, `active_workspace_for_output`, `set_active_workspace_for_output`, `sync_outputs_with_workspace_state`).
- Phase 2 ist teilweise aktiv:
  - `focused_output` wird aus Pointer-Position gepflegt, wenn ein Output eindeutig per Punkt-Geometrie bestimmbar ist.
  - `focused_output` wird bei Keyboard-Fokuswechsel auf ein zuordenbares Toplevel aktualisiert (z. B. neuer Toplevel, IPC focus-window).
  - Bei nicht auflösbarer Position/Surface bleibt `focused_output` unverändert.
- Phase 3 ist als Übergang aktiviert (Read-Path):
  - zentraler Read-Helper `current_workspace_index()` / `current_workspace_index_for_focused_output()` nutzt `focused_output + active_workspace_by_output`.
  - fehlendes focused_output oder fehlendes Mapping fällt auf `WorkspaceManager.active` zurück.
  - `switch_workspace` hält das Mapping für den fokussierten Output mit synchron.
  - weitere fokus-/window-lokale Read-Pfade werden schrittweise auf `current_workspace_index()` umgestellt.
- Phase 4a ist vorbereitet:
  - `switch_workspace_for_focused_output(target)` existiert als separater Pfad.
  - setzt `active_workspace_by_output` für `focused_output`.
  - führt `WorkspaceManager.active` als Kompatibilitäts-Shadow mit.
- Phase 4b ist aktiv:
  - `Super+1..9` (inkl. Fallback) ist auf `switch_workspace_for_focused_output(...)` geroutet.
  - `Super+Shift+1..9` bleibt unverändert auf dem bestehenden Move-Pfad.
  - IPC/Panel bleiben im Übergang global/shadow-basiert.

## Migrationsrisiken
- Bestehende globale Workspace-Tests müssen in output-spezifische Fälle erweitert werden.
- IPC `WorkspaceChanged` ist aktuell global gedacht; benötigt Output-Kontext.
- `WindowSnapshot`-Verarbeitung im Panel bleibt global occupied, braucht ggf. output-spezifischen active-Teil.
- Panel occupied-state darf nicht regressieren.
- Tiling muss output-spezifische Workspace-Sicht korrekt verwenden.
- Focus-Handling (Keyboard/Pointer) muss deterministisch `focused_output` liefern.

## Offene Entscheidungen
1. Panel-Rollout: sofort pro Output oder Übergang primary-only -> per-output.
2. IPC-Detailform für output-aware Snapshot/Event-Payload:
   - Strategie ist festgelegt auf Übergang (Option C: legacy + neue output-aware Events parallel),
   - offen bleibt die genaue Feldstruktur/Versionierung der neuen Eventtypen.
3. Policy bei Output-Hotplug-Add:
   - Standard-Workspace des neuen Outputs (z. B. Workspace 1 vs. letzter globaler Workspace).

## Phase-3 Read-Path Audit (nur Klassifikation)

### Bereits umgestellt
- `state/layout/focus.rs::focused_window`
  - Nutzung: liest Workspace über `current_workspace_index()`.
  - Risiko: niedrig.
  - Empfehlung: beibehalten.
- `state/layout/workspace.rs::update_focused_output_from_surface`
  - Nutzung: Window-Lookup über `current_workspace_index()`.
  - Risiko: niedrig.
  - Empfehlung: beibehalten.
- `state/layout/workspace.rs::current_workspace_index*`
  - Nutzung: zentraler Read-Helper mit Fallback auf `WorkspaceManager.active`.
  - Risiko: niedrig.
  - Empfehlung: als einzige Read-Quelle weiterverwenden.

### Sichere Read-Pfade (nächste Kandidaten)
- `state/ipc/commands.rs::focus_window_by_id` ✅ migriert
  - Nutzung: Lookup/Raise/Configure laufen über `space_at(current_workspace_index())`.
  - Risiko: niedrig (reiner Read-Path + Workspace-lokale Operationen im selben Index).
  - Empfehlung: beibehalten; IPC-Contract bleibt unverändert global.
- `state/layout/focus.rs::move_focused_window_to_workspace` (nur Lookup-Teil)
  - Aktuell: Window-Lookup in `active_space()`.
  - Risiko: mittel (Move-Operation im selben Pfad).
  - Empfehlung: nur in eigenem Task mit klarer Guard-Semantik anfassen; nicht zusammen mit Switch/Move-Verhalten mischen.

### Switch/Move-Semantik (später)
- `state/layout/workspace.rs::switch_workspace`
- `state/layout/workspace.rs::move_focused_window_to_workspace_consistent`
- `input/keyboard.rs` (`workspaces.active` für Aktionen wie `ToggleTiling`, `ResizeTile`)
  - Empfehlung: erst nach stabiler Read-Path-Migration und mit expliziter Policy-Entscheidung pro Action umstellen.

### Panel/IPC-Semantik (später)
- `state/ipc/broadcast.rs` (`WorkspaceChanged`, `WindowSnapshot.active_workspace`)
  - Aktuell bewusst global.
  - Empfehlung: erst mit IPC-Contract-Änderung pro Output migrieren.

### Tiling/Layout-Semantik (vorsichtig später)
- `state/layout/tiling.rs` (`workspaces.active`, `active_space()`)
- `state/layout/surface.rs` (`active_space()` für Hit-Testing/Layer-Lookup)
- `input/pointer/button.rs` (`active_space()` für Deko-/Window-Operationen)
  - Empfehlung: separat pro Modul migrieren, weil Read-/Mutation hier eng gekoppelt sind.

### Bewusst global (aktuell korrekt)
- `backend/drm/render.rs`, `backend/winit/scene.rs`, `backend/winit/mod.rs`
  - Nutzung von `active_space()` im Renderpfad bleibt global kompatibel.
- `state/handlers/xdg/lifecycle.rs`, `state/handlers/core/compositor.rs`, `protocols/xwayland.rs`
  - aktuell auf globalen Workspace-Lifecycle ausgelegt.

### Audit-Fazit
- Phase 3 ist **fortgesetzt**, aber nicht abgeschlossen.
- `focus_window_by_id`-Lookup auf `current_workspace_index()` ist abgeschlossen.
- Nächster Schwerpunkt liegt auf Phase 4e (Panel per-output Workspace-State), nachdem 4d1/4d2/4d3 aktiviert sind.

## Phase-4 Abschluss-Audit (Status)
- focused_output Pflege: aktiv (Pointer/Click/Keyboard-Fokus).
- active_workspace_by_output: aktiv (Read/Write im Hybrid-Modell).
- `Super+1..9`: focused-output-semantisch aktiv.
- `Super+Shift+1..9`: Move ohne Auto-Switch, ohne impliziten Output-Wechsel.
- `WorkspaceManager.active`: weiterhin Kompatibilitäts-Shadow.
- output-aware IPC:
  - 4d1 Typen vorhanden,
  - 4d2 Broadcast aktiv,
  - 4d3 Shell-State-Verarbeitung aktiv.
- Panel active workspace: output-aware aktiv, Legacy-Fallback aktiv.
- Legacy-Fallbacks: aktiv und beabsichtigt.

### Bekannte Übergangsgrenzen
- Occupied Workspaces bleiben global (WindowSnapshot-basiert), nicht per-output.
- Hotplug ist nur vorbereitet; kein vollständiger per-output Workspace-Lifecycle.
- output-aware Snapshot hängt an gültiger `OutputRegistry`-Datenlage.
- `WorkspaceManager.active` ist weiterhin Shadow und noch nicht vollständig obsolet.

## Phase 4 Keybinding-Semantik (Spezifikation vor Implementierung)

### 1) `Super+1..9`
- Wirkt auf `focused_output`.
- Setzt `active_workspace_by_output[focused_output] = target_workspace`.
- `WorkspaceManager.active` wird in Phase 4 als **Kompatibilitäts-Shadow** weitergeführt:
  - bei erfolgreichem Switch auf focused output wird `WorkspaceManager.active` ebenfalls auf das Ziel gesetzt.
  - Begründung: bestehende globale Pfade (IPC/Panel/Render-Teilpfade) bleiben bis Phase 4d/4e funktional.

### 2) `Super+Shift+1..9`
- Verschiebt fokussiertes Fenster in `target_workspace` (Workspace-Zuordnung).
- Kein impliziter Output-Wechsel.
- Kein automatischer Workspace-Switch nach Move.
- Fokusbehandlung:
  - Fokus auf Quell-Output wird geleert bzw. auf keinen toplevel gesetzt, konsistent mit bestehender Move-Semantik.
  - Kein automatischer Fokus auf Ziel-Workspace in Phase 4.

### 3) Panel-Verhalten (Übergang)
- In 4a-4c blieb das Panel global.
- Seit Phase 4e nutzt der Active-Marker output-aware State mit Legacy-Fallback.
- Occupied-Workspaces bleiben im Übergang weiterhin global.

### 4) IPC-Verhalten (Übergang)
- `WorkspaceChanged` bleibt als Legacy-Event ohne `output_id` aktiv.
- Zusätzlich sind output-aware Events/Snapshots in Phase 4d aktiv.
- Übergangsinterpretation:
  - Legacy-Event repräsentiert den globalen Kompatibilitätswert (`WorkspaceManager.active`).
  - Output-aware Events liefern den bevorzugten Kontext für Multi-Output-State.

## Phase 4d IPC-Workspace-Kontext (Spezifikation)

### 1) Bestehende relevante IPC-Events
- `WorkspaceChanged { workspace }` (legacy, global/shadow).
- `WindowSnapshot { active_workspace, windows }` mit `WindowSnapshotEntry { workspace, id, title }`.
- Weitere workspace/window-nahe Events:
  - `WindowOpened { id, title }`
  - `WindowClosed { id }`
  - `WindowFocused { id }`

### 2) Ziel-IPC-Modelle (Optionen)
- Option A: bestehende Events erweitern (z. B. `output_id`, `output_name`, per-output map).
- Option B: neue output-aware Events einführen (z. B. `OutputWorkspaceChanged`, `OutputWorkspaceSnapshot`).
- Option C: Übergang mit beidem (legacy + neue Events parallel).

### 3) Empfehlung für Meridian
Empfohlen: **Option C**.
- Grund:
  - Rückwärtskompatibel zu bestehender Shell/Panel-Verarbeitung.
  - Schrittweise Migration ohne Big-Bang-Änderung.
  - Klare Testbarkeit: neue Events separat prüfen, legacy-Fallback bleibt stabil.
- Ziel nach Migration:
  - Panel nutzt output-aware Snapshot als Quelle der Wahrheit.
  - Legacy `WorkspaceChanged` wird nur noch als Kompatibilitätspfad benötigt.

### 4) Benötigte Daten im output-aware Pfad
- `output_id` (stabile interne Kennung, serialisierbar).
- `output_name` (menschenlesbarer Output-Name, optional).
- `active_workspace_by_output` (Map output -> workspace).
- `focused_output`.
- `occupied_workspaces` global ableitbar aus `windows`.
- `windows` inkl. `workspace` (bereits vorhanden).
- Optional: explizites output->workspace snapshot payload für klare Shell-State-Aktualisierung.

### 5) Übergangsregeln
- `WorkspaceChanged { workspace }` bleibt in Phase 4d als legacy/global-shadow Event erhalten.
- Phase 4e-Panel soll primär output-aware Snapshot/Events verwenden und bei Fehlen auf legacy zurückfallen.
- Single-Output:
  - output-aware und legacy Werte müssen äquivalent sein.
- Unbekanntes `output_id` in Shell:
  - Event ignorieren oder auf legacy fallbacken, kein Crash.

### 6) Event-Semantik
- Bei `Super+1` auf Output A:
  - legacy: `WorkspaceChanged { workspace = shadow_active }`
  - neu: output-aware Event mit `output_id=A`, `active_workspace_by_output[A]=target`, plus vollständigem Snapshot-Kontext.
- Bei `Super+Shift+2` Move:
  - kein Auto-Switch; aktive Workspaces bleiben unverändert.
  - output-aware Snapshot/Update zeigt veränderte Window-Zuordnung (`windows[*].workspace`), nicht zwingend Workspace-Change auf Output-Ebene.
- Bei `focused_output`-Wechsel:
  - eigenes IPC-Event ist optional; bevorzugt im Snapshot-Kontext mitsenden statt Event-Flut.
- Bei Output add/remove:
  - eigener Folgepfad (Hotplug-Phase), nicht Teil von 4d-Implementierung; in 4d nur tolerant gegenüber unbekannten/fehlenden Outputs.

### 7) Implementierungsphasen (vorgeschlagen)
1. **4d1**: IPC-Typen für output-aware Workspace-Status ergänzen, legacy-Events unverändert behalten. ✅ spezifiziert/ergänzt
2. **4d2**: Compositor sendet output-aware Events/Snapshots zusätzlich zu legacy. ✅ aktiv
3. **4d3**: Shell verarbeitet output-aware Events, fallback auf legacy bleibt aktiv. ✅ aktiv
4. **4e**: Panel rendert per-output Workspace-State; legacy nur noch Backup. ✅ aktiv

### 8) Geplante Tests für 4d/4e
- IPC encode/decode für neue output-aware Events.
- Shell-State-Verarbeitung eines output-aware Workspace-Snapshots.
- Fallback auf legacy `WorkspaceChanged`.
- Single-Output-Regression (output-aware == legacy sichtbar gleich).

## Occupancy-Produktregel (final)
- **Verbindlich jetzt**:
  - Active Workspace ist output-aware.
  - Occupied Workspaces bleiben global (`WindowSnapshot`-basiert).
  - Panel darf active + occupied gleichzeitig darstellen; active hat visuell Vorrang.
- **Produktentscheidung**: Option C gestuft
  - jetzt: active per-output + occupied global
  - später: optional duale Darstellung (global/local occupied)

### Warum global occupied jetzt korrekt ist
- Snapshot-Daten liefern aktuell stabile Workspace-Belegung global.
- Das Verhalten bleibt über Single-/Multi-Output konsistent und deterministisch.
- Es vermeidet falsche lokale Occupancy-Aussagen bei unklarer Fenster->Output-Herkunft.

### Warum per-output occupied jetzt zu früh ist
- Fenster haben noch keine stabile Home-Output-Policy als Quelle der Wahrheit.
- Hotplug-Regeln für Reassignment/Fallback sind noch nicht finalisiert.
- Move-Pfade zwischen Outputs sind für Occupancy-Semantik noch nicht vollständig spezifiziert.
- Panel wird noch nicht pro Output als unabhängige Instanz geführt.

### Voraussetzungen für spätere per-output Occupancy
1. Stabile Fenster->Output-Zuordnung (inkl. Home-Output-Regel).
2. Verbindliche Hotplug-Policies (Output add/remove, Mapping-Recovery).
3. Definierter Move-window-between-outputs-Pfad.
4. Klare Panel-Policy pro Output (single primary vs. per-output panel).

### Manual-Test-Erwartung (verbindlich)
1. Ein Workspace mit Fenster wird als occupied markiert, auch wenn er auf dem focused output nicht aktiv ist.
2. Wenn ein Workspace zugleich active und occupied ist, hat active-Markierung Vorrang.

## Hotplug-Policy (verbindlich, vor Implementierung)

### 1) Output Add
- `OutputId` wird eindeutig und monoton vergeben; bestehende IDs werden nicht wiederverwendet.
- Neuer Output erhält initial `active_workspace_by_output[new_output] = WorkspaceManager.active` (Kompatibilitäts-Shadow).
- `focused_output` bleibt unverändert, außer es gibt noch keinen gültigen Fokus; dann Fallback `primary -> first`.
- Primary-Policy:
  - erster vorhandener Output ist primary,
  - bei späterem Add bleibt bestehender primary stabil (kein Auto-Wechsel),
  - nur wenn kein primary auflösbar ist, wird der erste verfügbare Output primary.
- Panel/Layer-Shell:
  - explizit gebundene Layer-Surfaces bleiben explizit,
  - nicht explizite Fallback-Surfaces nutzen `primary -> first` und werden neu konfiguriert.

### 2) Output Remove
- Wenn `focused_output` entfernt wird: sofortiger Fallback `primary -> first`.
- `active_workspace_by_output[removed_output]` wird entfernt (cleanup, kein stale Mapping).
- Workspaces/Fenster bleiben erhalten; keine implizite Fenster- oder Workspace-Migration.
- Fullscreen/Maximized auf entferntem Output:
  - Fenster bleibt erhalten,
  - Geometrie wird auf Fallback-Output neu aufgelöst und reconfigured,
  - falls nicht sauber auflösbar, kontrollierter Fallback auf nicht-fullscreen/maximized Zustand.
- Layer-Shell auf entferntem Output:
  - explizite Bindung auf ungültigen Output wird als verloren behandelt,
  - Fallback-Zuordnung `primary -> first`, danach reconfigure.

### 3) Output Reconfigure
- Änderungen an `geometry/scale/transform/refresh` aktualisieren `OutputRegistry` atomar für den betroffenen Output.
- Danach verpflichtend:
  - Dirty für betroffenen Output setzen,
  - Layout-/Surface-Geometrie für den Output invalidieren,
  - Wallpaper-Cache für den Output neu validieren/invalidieren,
  - Layer-Shell-Rearrange/Reconfigure für den Output auslösen.
- Kein globales Redraw aller Outputs, außer wenn Geometrie-Überlappungen oder globale Bounds betroffen sind.

### 4) Workspace Recovery
- Wenn `focused_output` verschwindet: Fallback `primary -> first`.
- Wenn ein Workspace nach Hotplug auf keinem Output sichtbar ist:
  - Workspace bleibt im globalen Set erhalten,
  - bleibt unsichtbar bis erneut einem Output aktiv zugeordnet.
- Kein Fensterverlust.
- Kein automatisches Verschieben von Fenstern ohne explizite Policy.

### 5) IPC-/Shell-Folgeplan (Spezifikation)
- Später relevante Events:
  - `OutputAdded`
  - `OutputRemoved`
  - `OutputChanged`
  - `OutputWorkspaceSnapshot`
- Übergang:
  - `OutputWorkspaceSnapshot` reicht zunächst als robuste Quelle der Wahrheit,
  - dedizierte Add/Remove/Changed-Events sind Optimierung/Präzisierung.

### 6) Implementierungsphasen (Hotplug)
1. **H1**: `OutputRegistry` remove/reconfigure API + Unit-Tests. ✅ vorbereitet
2. **H2**: `WorkspaceOutputState` cleanup/fallback Tests (`focused_output`, mapping cleanup). ✅ vorbereitet
3. **H3**: Compositor broadcastet output-aware Snapshot nach Hotplug-Änderungen. ✅ vorbereitet
4. **H4**: Layer-Shell/Panel recovery bei remove/reconfigure absichern. ✅ vorbereitet
5. **H5**: Echte Hotplug-Anbindung in DRM/Winit-Lifecycle.

### H5-Aufteilung (konkret)
- **H5a (kleinster Slice)**: ✅ aktiv
  - Winit `Resized`-Pfad gezielt als Output-Reconfigure behandeln (`handle_output_reconfigured`).
- **H5b**:
  - DRM Connector-Rescan/Reconfigure-Hook am DRM-Notifier.
- **H5c**:
  - DRM remove/add minimal auf die zentralen `handle_output_*`-Pfade routen.
- **H5d**:
  - Manueller End-to-End-Hotplug-Test mit Log-Validierung.

### 5) Verbindliche Invarianten für Phase 4
1. Ein Output zeigt genau einen aktiven Workspace (`active_workspace_by_output`).
2. `focused_output` bestimmt input-getriebene Workspace-Aktionen.
3. `WorkspaceManager.active` ist in Phase 4 ein Kompatibilitätsmechanismus, nicht langfristige Quelle der Wahrheit.

### 6) Umsetzungs-Slices
1. **Phase 4a**: `switch_workspace_for_focused_output(...)` einführen, globalen `switch_workspace(...)` unangetastet lassen.
2. **Phase 4b**: `Super+1..9` auf neue Methode routen. ✅ aktiv
3. **Phase 4c**: `Super+Shift+1..9` gegen focused-output-Policy prüfen/angleichen (ohne Auto-Switch). ✅ aktiv
4. **Phase 4d**: IPC-Plan für `WorkspaceChanged` mit Output-Kontext spezifizieren/umsetzen. ✅ 4d1/4d2/4d3 aktiv
5. **Phase 4e**: Panel auf focused/per-output Workspace-State angleichen. ✅ aktiv

### Phase 4b Status
- Aktiv:
  - `Action::SwitchWorkspace` nutzt `switch_workspace_for_focused_output(...)`.
  - Super+1..9 Fallback nutzt ebenfalls `switch_workspace_for_focused_output(...)`.
- Phase 4c aktiv:
  - `Super+Shift+1..9` bleibt auf Move-Pfad, ist aber focused-output-policy-konsistent:
    - kein Auto-Switch,
    - kein impliziter Output-Wechsel,
    - Source-Workspace wird vom fokussierten Fenster ermittelt.
- Phase 4e aktiv:
  - Panel-Active-Marker nutzt bevorzugt output-aware Workspace-State.
  - Fallback-Reihenfolge: `focused_output_id` -> `focused` -> `primary` -> `first` -> legacy `active_workspace`.
  - Occupied-Workspaces bleiben global aus `WindowSnapshot`.
- Manueller Test:
  - Siehe `docs/DEBUGGING.md`, Abschnitt `Manueller E2E-Test: Focused-Output Workspace Switch (Phase 4b)`.
  - Siehe `docs/DEBUGGING.md`, Abschnitt `Manueller E2E-Test: Focused-Output Move-to-Workspace (Phase 4c)`.
  - Siehe `docs/DEBUGGING.md`, Abschnitt `Manueller E2E-Test: Panel Output-aware Active Workspace (Phase 4e)`.
