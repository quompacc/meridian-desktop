# Multi-Monitor Audit

## Scope
Dieser Task dokumentiert den aktuellen Output-Stand und ein Zielmodell für späteren Multi-Monitor-Support.  
Keine vollständige Implementierung, keine Render-/Workspace-Verhaltensänderung.

## Ist-Zustand (Code-Audit)

### 1) Output-Registrierung
- Winit:
  - `crates/meridian-compositor/src/backend/winit/mod.rs`
  - erzeugt genau einen Output (`"winit"`), setzt Mode (`refresh: 60_000`), mapped auf aktive Workspace bei `(0,0)`, pusht in `state.outputs`.
- DRM:
  - `crates/meridian-compositor/src/backend/drm/init.rs`
  - enumeriert verbundene Connectoren, erzeugt pro Connector `Output` (`drm-{idx}`), setzt Mode/Refresh/Transform, mapped Outputs horizontal versetzt (`x_offset`), pusht in `state.outputs`.
  - zusätzlich pro Output `DrmOutput` in `DrmBackend.outputs`.

### 2) Output-Geometrie / State
- Global im State:
  - `crates/meridian-compositor/src/state/mod.rs`
  - `MeridianState.outputs: Vec<Output>`
  - `MeridianState.output_registry: OutputRegistry` (read-only Sicht auf Output-Metadaten)
- Wayland-Output-Protokollmodul:
  - `crates/meridian-compositor/src/protocols/output.rs` ist derzeit nur Placeholder.
- Workspace-Mapping:
  - `crates/meridian-compositor/src/workspace.rs`
  - `Space::map_output(...)` wird verwendet; beim Workspace-Switch werden Outputs vom alten in den neuen Workspace remapped (`remap_outputs`).

### 3) Rendering pro Output
- DRM:
  - `crates/meridian-compositor/src/backend/drm/render.rs`
  - iteriert über `drm.outputs` und rendert pro Output.
  - Layer-Daten werden pro Output geholt (`collect_layer_data(&out.output)`).
  - Cursor wird nur für den Output gerendert, dessen Geometrie den Pointer enthält.
- Winit:
  - `crates/meridian-compositor/src/backend/winit/mod.rs`
  - ein Output, ein Damage-Tracker, ein Redraw-Loop.

### 4) Layer-Shell Output-Zuordnung
- `crates/meridian-compositor/src/state/handlers/core/layer_shell.rs`
- Neue Layer-Surfaces:
  - nutzen angeforderten Output (`wl_output`) falls vorhanden,
  - sonst Registry-Policy `primary -> first`.
  - unbekannter angeforderter Output wird geloggt und fällt sicher zurück.
- Layer-Rendering bleibt output-lokal über `layer_map_for_output(output)`.

### 5) Cursor-Koordinaten
- Pointer-Motion:
  - `crates/meridian-compositor/src/input/pointer/mod.rs`
  - Absolute Motion nutzt jetzt `OutputRegistry`-Desktop-Bounds + `output_at_point(...)` mit `primary()/first()`-Fallback.
- Cursor-Render:
  - `crates/meridian-compositor/src/backend/drm/render.rs`
  - Pointer-Location wird gegen jeweilige `output_geometry` geprüft; Cursor nur auf passendem Output.

### 6) Workspace-Mapping zu Outputs
- Aktuell ein global aktiver Workspace (`WorkspaceManager.active`).
- Beim Workspace-Switch werden alle Outputs auf den neuen aktiven Workspace remapped.
- Kein per-output aktiver Workspace.
- Phase-1-Datenmodell ist vorbereitet:
  - `focused_output` + `active_workspace_by_output` werden im Compositor-State geführt.
  - bisher read-only/prepare-only; globale Workspace-Semantik bleibt aktiv.
- Phase-2-Fokuspflege ist aktiv:
  - Pointer Motion/Click aktualisieren `focused_output` nur bei eindeutiger Punktzuordnung auf einen Output.
  - Keyboard-Fokuswechsel aktualisiert `focused_output`, wenn die fokussierte Surface einem Window/Output zuordenbar ist.
  - Unauflösbare Fälle lassen `focused_output` unverändert.
- Phase-3-Read-Path ist aktiv:
  - aktueller Workspace-Read nutzt bevorzugt `active_workspace_by_output` des `focused_output`.
  - globales `WorkspaceManager.active` bleibt Kompatibilitäts-Fallback.
  - zusätzliche fokus-/window-lokale Read-Pfade laufen schrittweise über `current_workspace_index()`.
  - `focus_window_by_id` nutzt jetzt ebenfalls `current_workspace_index()` statt direktem `active_space()`.
  - keine vollständige Keybinding-/Panel-/IPC-Umstellung in dieser Phase.
  - detaillierte Restpfad-Klassifikation: `docs/WORKSPACES.md` Abschnitt `Phase-3 Read-Path Audit`.
- Phase-4-Keybinding-Semantik ist spezifiziert:
  - `Super+1..9` wirkt auf `focused_output` und aktualisiert `active_workspace_by_output`.
  - `WorkspaceManager.active` bleibt zunächst globaler Kompatibilitäts-Shadow.
  - `Super+Shift+1..9` bleibt ohne Auto-Switch und ohne impliziten Output-Wechsel.
  - Details: `docs/WORKSPACES.md` Abschnitt `Phase 4 Keybinding-Semantik`.
- Phase 4a ist als Codepfad vorbereitet:
  - `switch_workspace_for_focused_output(...)` ist implementiert.
- Phase 4b ist aktiv:
  - `Super+1..9` (inkl. Fallback) routet jetzt auf `switch_workspace_for_focused_output(...)`.
  - `Super+Shift+1..9` bleibt auf Move-Pfad.
  - Phase 4c ist aktiv: Move ist focused-output-policy-konsistent (kein Auto-Switch, kein impliziter Output-Wechsel, Source-Workspace über fokussiertes Fenster).
  - Manueller Ablauf und erwartete Logs: `docs/DEBUGGING.md`, Abschnitt `Manueller E2E-Test: Focused-Output Workspace Switch (Phase 4b)`.
  - Manueller Move-Test und Logs: `docs/DEBUGGING.md`, Abschnitt `Manueller E2E-Test: Focused-Output Move-to-Workspace (Phase 4c)`.
- Phase 4d ist spezifiziert und teilweise implementiert:
  - IPC soll auf output-aware Workspace-Kontext erweitert werden.
  - Empfehlung ist Parallelbetrieb: legacy `WorkspaceChanged` + neue output-aware Events/Snapshots während der Migration.
  - Phase 4d1 ist vorbereitet: IPC-Typen für output-aware Workspace-State sind im IPC-Crate ergänzt.
  - Phase 4d2 ist aktiv: Compositor sendet output-aware Workspace-Events/Snapshots zusätzlich zu Legacy-Events.
  - Phase 4d3 ist aktiv: Shell speichert output-aware Workspace-State und hält Legacy-Fallback parallel aktiv.
  - Phase 4e ist aktiv: Panel-Active-Marker nutzt output-aware State mit Legacy-Fallback.
  - Referenz: `docs/WORKSPACES.md` Abschnitt `Phase 4d IPC-Workspace-Kontext (Spezifikation)`.
- Phase-4 Abschluss-Audit dokumentiert:
  - Fokus-/Switch-/Move-/IPC-/Panel-Active-Pfade sind konsistent.
  - Occupancy-Produktregel ist finalisiert: active output-aware, occupied global (active hat Vorrang).
  - Offene Grenze bleibt per-output Occupancy erst nach stabiler Output/Home-Policy + Hotplug-Follow-up.
- Tiling-Rect-Auswahl nutzt `OutputRegistry` mit definierter Fallback-Policy (`primary` -> `first`), nicht mehr `outputs.first()`.
- Surface Hit-Testing in `state/layout/surface.rs` nutzt `OutputRegistry`-Auswahl über Punkt-Geometrie (`output_at_point`-äquivalent) mit Fallback `primary` -> `first`.
- Pointer-Button/Click-Output-Auswahl in `input/pointer/button.rs` nutzt `OutputRegistry`-Policy `point-match -> primary -> first` statt implizitem `outputs.first()`.

### 7) Wallpaper pro Output
- DRM:
  - `DrmOutput.wallpaper: Option<WallpaperGpuCache>` pro Output.
- Winit:
  - ein globaler Wallpaper-Cache für den einzelnen Winit-Output.

### 8) Dirty / Damage
- Winit: `OutputDamageTracker` (ein Output).
- DRM: per-Output `DrmCompositor::render_frame(...)` + `queue_frame`.
- Workspace-Refresh ist derzeit global auf `active_space`.

## Zielmodell (für spätere Implementierung)

### Output-Identität
- `OutputId` (stabil intern) + `OutputName` (menschlich, z. B. `eDP-1`, `HDMI-A-1`).
- Mapping von Wayland-Output-Resource auf `OutputId`.

### Output-Zustand
- `geometry`: `x/y/width/height`
- `scale`
- `transform`
- `refresh`
- `primary`-Policy (z. B. first-connected oder konfigurierbar)

### Workspace-Strategie
- Spezifikation ist festgelegt in `docs/WORKSPACES.md`.
- Zielentscheidung: Hybrid-Modell (globales Workspace-Set + `active_workspace_by_output` + `focused_output`).
- Invarianten/Keybinding-/Panel-Regeln dort verbindlich dokumentiert.

### Layer / Panel / Launcher
- Layer-Surfaces sauber pro Output führen.
- Panel-Placement-Regel festlegen:
  - nur Primary Output oder pro Output ein Panel.

### Wallpaper
- Wallpaper-Cache und Konfiguration pro Output (DRM bereits teilweise vorbereitet).

### Dirty-Tracking
- Dirty-Flag/Damage pro Output führen.
- Änderungen an einem Output sollen andere Outputs nicht unnötig rendern.

### Screenshot-Bezug
- `ScreenshotBridgeRequest.output` auf `OutputId/OutputName` eindeutig auflösen.
- Bei unbekanntem Output kontrollierter Fehler (`InvalidRequest`/`Unsupported`).

## Risiken
- Größtes verbleibendes Architekturthema ist die Umsetzung der spezifizierten Hybrid-Workspace-Policy (`docs/WORKSPACES.md`).
- Maximize/Fullscreen und Tiling verwenden jetzt definierte Registry-Fallbacks, aber weiterhin ohne per-output Workspace-Policy.
- Layer-Shell nutzt jetzt Registry-Fallbacks; ohne per-output Workspace-Policy bleiben Platzierungsentscheidungen dennoch global geprägt.
- Screenshot-Output-Auswahl ist im Contract vorhanden, aber noch ohne echte Output-Auflösung.
- DRM-Commit/Lifecycle bei Hotplug (add/remove/reconfigure) ist noch nicht als eigener Zustandspfad modelliert.

## Hotplug-Policies (spezifiziert)
- Verbindliche Add/Remove/Reconfigure/Recovery-Regeln sind in `docs/WORKSPACES.md` unter `Hotplug-Policy (verbindlich, vor Implementierung)` festgelegt.
- Kernregeln:
  - Add: neues Output-Mapping initial auf `WorkspaceManager.active`, Fokus bleibt stabil.
  - Remove: `focused_output` fallback `primary -> first`, Mapping-Cleanup ohne Fensterverlust.
  - Reconfigure: output-lokale Invalidation (Layout/Wallpaper/Layer-Shell), kein unnötiges globales Redraw.
  - Recovery: keine implizite Fenster-Migration, unsichtbare Workspaces bleiben erhalten.

## H1-H4 Abschluss-Audit (vor Backend-Anbindung)

### Pipeline (aktuell)
1. `OutputRegistry` ändern (`add/update/remove/reconfigure`) über zentrale State-Helper in `state/setup.rs`.
2. `WorkspaceOutputState` synchronisieren (`sync_outputs_with_workspace_state`, inkl. stale-cleanup + focused fallback).
3. Layer-Shell-Recovery ausführen (`reconcile_layer_shell_outputs_after_output_change`).
4. Output-aware Snapshot broadcasten (`broadcast_output_workspace_snapshot`).
5. Reconfigure/Dirty-seitige Wirkung:
   - aktuell Layer-Shell: `arrange()` + `send_configure()`;
   - kein vollständiges globales Dirty-/Modeset-Lifecycle in H1-H4.

### Reihenfolge-Bewertung
- Die aktuelle Reihenfolge ist konsistent mit der Policy:
  - Registry zuerst, dann Workspace-Fallback, dann Layer-Recovery, dann Snapshot.
- Keine Abweichung, daher kein Code-Change in diesem Audit.

### Backend-Hookpunkte für H5
- DRM:
  - `crates/meridian-compositor/src/backend/drm/init.rs`
  - bestehender Device-Notifier ist vorhanden (`insert_source(drm_notifier, ...)`), aktuell auf `DrmEvent::VBlank` beschränkt.
  - H5-Hook: Connector-Änderungen dort erkennen und auf `handle_output_*` routen (rescan/reconfigure/remove/add).
- Winit:
  - `crates/meridian-compositor/src/backend/winit/mod.rs`
  - `WinitEvent::Resized` ist vorhanden und aktualisiert bereits Output-Metadaten.
  - H5a-Hook: Resize/Reconfigure gezielt über `handle_output_reconfigured(...)` führen (statt generischem Update), mit stabilem OutputId-Pfad.

### H5-Plan
1. **H5a**: Winit Resize/Reconfigure explizit auf `handle_output_reconfigured(...)` routen. ✅ aktiv
2. **H5b**: DRM Connector-Rescan/Reconfigure-Hook am DRM-Notifier etablieren. ✅ aktiv
3. **H5c**: DRM remove minimal auf `handle_output_removed(...)` routen. ✅ aktiv
4. **H5c-add**: DRM add minimal auf `handle_output_added_or_updated(...)` routen. ✅ aktiv
5. **H5d**: Manueller Hotplug-Testlauf mit erwarteten Logs (Fallback/Recovery/Snapshot). ✅ dokumentiert

### H5d Zwischenergebnis (realer Lauf)
- Initialer DRM Output Add: **pass**
- Layer-Shell Recovery bei initial add: **pass**
- OutputWorkspaceSnapshot bei initial add: **pass**
- Reconfigure: **pending**
- Runtime Remove: **pending**
- Runtime Add: **pending**
- Beobachtung: `Smithay atomic restore previous state failed` mit `EINVAL`
- Stabilität: kein Crash beobachtet
- NVIDIA-VM-Lauf bestätigt:
  - `card0=NVIDIA (10de:2783)`, `card1=virtio-gpu`, `card0-HDMI-A-1 connected`
  - Renderpfad stabil (`3440x1440@60Hz`, Panel-Layer sichtbar, Pointer auf `drm-0` ok)

### Risiken (H5)
- DRM connector identity/stable matching (Name/Handle-Zuordnung).
- Primary-Auswahl bei Add/Remove-Reihenfolgen.
- stale Smithay `Output`-Objekte bei Remove-Timing.
- Layer-Surface close/reassign Timing während Output-Wechsel.
- Atomic modeset edge cases bei Reconfigure/Remove.

## First-Output Audit (nach OutputRegistry-Migration)
- Scope: `crates/meridian-compositor/src` (Codepfade, keine reine Doku-Treffer).
- Suchmuster: `outputs.first()`, `.first()` im Output-Kontext, `first output`, `primary`, `fallback`.

Klassifikation:
- Bereits migriert:
  - Pointer absolute motion (`input/pointer/mod.rs`)
  - Pointer button (`input/pointer/button.rs`)
  - Surface hit-testing (`state/layout/surface.rs`)
  - Maximize/Fullscreen (`state/handlers/xdg/requests/state.rs`)
  - Tiling (`state/layout/tiling.rs`)
  - Layer-Shell fallback (`state/handlers/core/layer_shell.rs`)
- Bewusst erlaubt:
  - `state/output_registry.rs::OutputRegistry::first()` verwendet intern `self.outputs.first()` als definierte API-Basis.
  - Selektor-Helfer in migrierten Modulen nutzen `.first()` auf `OutputInfo`-Slices als definierter letzter Fallback (`primary -> first`).
- False positives (kein Output-Auswahlrisiko):
  - `backend/drm/render.rs`: `render_order.first()` (Render-Stack-Assertion, kein Output-Routing).
  - `backend/drm/gpu.rs`: `primary_gpu(...)` (GPU-Erkennung, nicht Output-Auswahl im Workspace/Hit-Testing).
- Noch riskant:
  - Keine verbleibende produktive `state.outputs.first()`-Fallback-Stelle in den bekannten Output-Auswahlpfaden gefunden.

## Debugging-Leitfaden (Multi-Monitor)
- Output-Lifecycle:
  - erwartete Logs bei Init: Output-Name, Geometrie, Mode, Refresh.
  - Hotplug-Remove/Add sind minimal über die H5-Pipeline umgesetzt.
  - VBlank-/Scan-Trigger-Logs sind auf `trace` reduziert; Hotplug-Änderungslogs bleiben auf `debug/info` für H5d-Auswertung.
- Layer-Shell:
  - `new_layer_surface`: requested_output vs. mapped output prüfen.
  - Layer-Map pro Output prüfen.
- Render:
  - pro Frame render target output + element counts prüfen (`backend/drm/render.rs`).
- Pointer/Cursor:
  - pointer location vs. output geometry prüfen.
  - Cursor sichtbar nur auf dem enthaltenen Output erwarten.

## Nächster kleiner Implementierungsslice
1. H5d auf realer DRM-Hardware ausführen und Ergebnisprotokoll in `docs/PROJECT_STATUS.md` eintragen.
2. NVIDIA-Passthrough-Hardwarelauf mit `docs/NVIDIA_PASSTHROUGH.md` durchführen und H5d Runtime-Ergebnisse ergänzen.
