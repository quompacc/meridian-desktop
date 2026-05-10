# Project Status

## Aktueller Stand
- Umfangreiche Modul-Splits über Compositor, Shell, State, Render, Theme, IPC durchgeführt.
- Workspace bleibt testbar; `cargo test --workspace` zuletzt grün.
- Refactor-Integrität aktuell geprüft: `cargo fmt` + `cargo check --workspace` grün.
- Architektur ist deutlich modularer als am Start (weniger große Sammeldateien).
- Panel ist sichtbar; Layer/Render-Z-Order-Fix ist integriert.
- Workspace-/Panel-Regressionstests für reine Logik ergänzt (Switch-Guards + Occupied-Snapshot-State).
- IPC/Snapshot-Regressionstests ergänzt (WindowSnapshot roundtrip, Active/Occupied-Berechnung inkl. Out-of-Range-Werte).
- Minimale `config.toml`-Unterstützung aktiviert (`~/.config/meridian/config.toml`) mit Fallback auf Defaults bei fehlender/fehlerhafter Datei.
- `ReloadConfig` lädt jetzt zur Laufzeit auch `general.theme`, `cursor`, `wallpaper` neu (zusätzlich zu Keybinds) und wendet Overrides ohne Neustart an.
- `ReloadConfig` synchronisiert jetzt zusätzlich die `meridian-shell`: Theme/Panel-Styles werden zur Laufzeit neu geladen, Panel wird als dirty markiert und neu gezeichnet.
- E2E-Manuelltest für ReloadConfig (gültig/zweite Änderung/ungültig/fehlend) ist im Debugging-Guide dokumentiert.
- `docs/TESTING.md` ergänzt: zentrale Testübersicht für Standardchecks, Unit-Testbereiche und manuelle E2E-Referenzen.
- Keybinding-Config gehärtet: klarere Parse-Fehler, Tests für Defaults (`Super+1..9`, `Super+Shift+1..9`) und Reload-Verhalten bei ungültigen Keybinds.
- `docs/CONFIGURATION.md` ergänzt: vollständiges `config.toml`-Beispiel inkl. `[keybinds]`.
- Launcher-Baseline verbessert: robustere `.desktop`-Erkennung (XDG-App-Verzeichnisse, Ignore ungültiger/leerer Entries), stabilere Sortierung, gecachte App-Liste + bestehender Query-Filter beibehalten.
- Launcher-Parser gehärtet: `OnlyShowIn`/`NotShowIn` für Desktop-Umgebung `Meridian`, robustere `Exec`-Bereinigung (Fieldcodes + einfache Quotes), `TryExec` mit PATH/Absolutpfad und Executable-Check (Unix).
- Launcher-Startpfad auf argv-basierten Launch umgestellt: kein `sh -c` mehr im Shell-Fallback und im Compositor-Launchpfad; `LaunchApp` überträgt jetzt `program + args` (mit Legacy-Decode für altes `command`-Feld).
- Launcher-UI-Baseline poliert: klarere Suchzeile, sichtbare Auswahlmarkierung (inkl. Up/Down), Trefferzähler und robuster Empty-State ohne neue Effekte/Assets.
- Launcher-Selektion vereinheitlicht: Hover, Klick und Enter teilen denselben `selected_index`; Maus-/Tastatur-Navigation bleibt konsistent.
- Cursor-Fallback-Qualität verbessert: eingebetteter ARGB-Cursor nutzt feineres Antialiasing, präzisere Form und premultiplied Alpha; Hotspot bleibt exakt bei `(0,0)`.
- XDG-Portal-Plan dokumentiert (`docs/XDG_PORTALS.md`): Architekturgrenzen, Reihenfolge und Risiken für stufenweise Einführung.
- `meridian-portal` Minimal-Scaffold angelegt: eigenes Binary mit Health-Logging und Screenshot-Stummel (`Unsupported`), ohne D-Bus-/Portal-Featurelogik.
- `meridian-portal` D-Bus-Service-Skeleton ergänzt: Session-Bus-Connect + Name-Request + Screenshot-Interface-Platzhalter; Screenshot bleibt bewusst `Unsupported`.
- Screenshot-Bridge-Contract (Portal <-> Compositor) als reine Typen ergänzt (`ScreenshotBridgeRequest/Response/Error`) inkl. request/correlation-id, output target, include_cursor und deny-only Fehlersemantik.
- Deny-only Bridge-Transport aktiv: `meridian-portal` sendet Screenshot-Requests jetzt über den bestehenden IPC-Socket an den Compositor und erhält typisierte `ScreenshotBridgeResult`-Antworten (`PermissionDenied`/`Unsupported`/`InvalidRequest`/`CompositorUnavailable`).
- Screenshot-Policy-Layer ergänzt: Requests laufen im Compositor über `ScreenshotPolicy::evaluate(...)` mit deny-by-default (`Deny` für valide Requests, `Unsupported` für Region, `Invalid` für fehlerhafte Requests); Request-Metadaten (`origin`, `requester`, `request_marker`, `identity_trusted`) werden übertragen, aktuell aber noch nicht als vertrauenswürdige App-Identität verwendet.
- Multi-Monitor Output-Model-Audit ergänzt (`docs/MULTI_MONITOR.md`): aktueller Output-Stand, Zielmodell (OutputId/Geometry/Scale/Transform/Refresh/Primary/Dirty pro Output), bekannte Lücken (`outputs.first()`-Pfade) und nächster kleiner Implementierungsslice dokumentiert.
- Read-only `OutputRegistry` eingeführt (`OutputId`/`OutputInfo`/`OutputRegistry`): Winit und DRM registrieren Output-Metadaten zentral (id, name, geometry, scale, transform, refresh, primary), ohne bestehendes Verhalten zu ändern.
- Absolute Pointer Motion auf `OutputRegistry` umgestellt: Output-Auswahl nutzt `output_at_point(x,y)` mit `primary()/first()`-Fallback statt implizitem `outputs.first()`.
- Maximize/Fullscreen-Geometry auf `OutputRegistry` umgestellt: bevorzugt output-zuordenbares Fensterziel (`window-output`), sonst definierter Fallback `primary` -> `first`.
- Tiling-Output-Rect auf `OutputRegistry` umgestellt: definierte Auswahlregel `primary` -> `first` statt implizitem `outputs.first()`.
- Layer-Shell-Output-Fallback auf `OutputRegistry` umgestellt: bei fehlender oder unbekannter expliziter Output-Zuordnung gilt `primary` -> `first`, mit Debug-Logs für Auswahl/Fallback.
- Surface-Hit-Testing/Output-Auswahl auf `OutputRegistry` umgestellt (`state/layout/surface.rs`): Punkt-basierte Auswahl mit Fallback `primary` -> `first` statt implizitem `outputs.first()`.
- Pointer-Button/Click-Output-Auswahl auf `OutputRegistry` umgestellt (`input/pointer/button.rs`): `point-match -> primary -> first` statt implizitem `outputs.first()`.
- First-Output-Audit nach Migration durchgeführt: in den bekannten produktiven Compositor-Output-Auswahlpfaden keine verbleibende `state.outputs.first()`-Fallback-Stelle; verbleibende `first()`-Nutzung in `OutputRegistry` ist bewusst definierter API-Fallback.
- Per-Output-Workspace-Policy spezifiziert (`docs/WORKSPACES.md`): Zielmodell Hybrid (`active_workspace_by_output` + `focused_output`) mit Invarianten, Keybinding-Regeln und Phasenplan.
- Workspace Output State vorbereitet (Phase 1): `focused_output` + `active_workspace_by_output` inkl. Fallback-/Sync-Helper und Unit-Tests vorhanden; globale Workspace-Semantik bleibt unverändert aktiv.
- focused_output Pflege aktiviert (Phase 2): aktualisiert bei Pointer-Motion/Click mit eindeutiger Output-Zuordnung und bei zuordenbaren Keyboard-Fokuswechseln; globale Workspace-Switch/Move-Semantik bleibt unverändert.
- active_workspace_by_output Read-Path aktiviert (Phase 3 Übergang): zentraler Workspace-Read bevorzugt `focused_output`-Mapping und fällt auf globales `WorkspaceManager.active` zurück; keine Keybinding-/Panel-/IPC-Umstellung in diesem Schritt.
- Phase 3 fortgesetzt: weitere fokus-/window-lokale Workspace-Reads nutzen `current_workspace_index()`; Switch/Move-Semantik bleibt global-kompatibel unverändert.
- Phase-3 Workspace Read-Path Audit dokumentiert: verbleibende `active_space`/`active`-Pfadstellen sind in `docs/WORKSPACES.md` klassifiziert (bereits migriert / sichere nächste Kandidaten / bewusst später).
- `focus_window_by_id` Read-Path auf `current_workspace_index()` umgestellt: Workspace-Lookup/Raise/Configure sind focused-output-aware mit globalem Fallback; IPC-Semantik unverändert.
- Phase-4-Spezifikation dokumentiert (ohne Implementierung): Keybinding-Semantik für `Super+1..9` und `Super+Shift+1..9` ist als focused-output-Policy festgelegt, inklusive Übergangsrolle von `WorkspaceManager.active` und geplantem IPC/Panel-Folgepfad.
- Phase 4a vorbereitet: `switch_workspace_for_focused_output(target)` implementiert (separater Pfad) inkl. Shadow-Sync für `WorkspaceManager.active`.
- Phase 4b aktiv: Super+1..9 (inkl. Keyboard-Fallback) nutzt jetzt `switch_workspace_for_focused_output(...)`; IPC/Panel-Semantik bleiben unverändert.
- Manueller Phase-4b-Test für focused-output Switch ist dokumentiert (Single-Output-Regression, Pointer-Fokus, Fensterfokus, Fallback): `docs/DEBUGGING.md`.
- Phase 4c aktiv: `Super+Shift+1..9` bleibt ohne Auto-Switch und ohne impliziten Output-Wechsel; Source-Workspace wird über das fokussierte Fenster bestimmt (nicht blind global), `focused_output`/`active_workspace_by_output` bleiben bei erfolgreichem Move unverändert.
- Phase 4d spezifiziert (ohne Implementierung): output-aware IPC-Strategie für Workspace-Kontext ist dokumentiert (Parallelbetrieb legacy + neue output-aware Events/Snapshots, dann Shell/Panel-Migration).
- Phase 4d1 ergänzt: `meridian-ipc` enthält jetzt output-aware Workspace-Typen/Events (`OutputWorkspaceState`, `OutputWorkspaceSnapshot`, `OutputWorkspaceChanged`, `OutputWorkspaceSnapshot`-Event); legacy `WorkspaceChanged`/`WindowSnapshot` bleiben unverändert und aktiv.
- Phase 4d2 aktiv: Compositor broadcastet output-aware Workspace-Events zusätzlich (`OutputWorkspaceChanged` bei focused-output Switch, `OutputWorkspaceSnapshot` parallel zu Legacy-Snapshot).
- Phase 4d3 aktiv: Shell speichert/verarbeitet output-aware Workspace-Events im eigenen State (`focused_output_id`, `output_workspaces`) mit Legacy-Fallback.
- Phase 4e aktiv: Panel-Workspace-Markierung nutzt bevorzugt output-aware Active-Workspace-Auswahl (`focused_output_id` -> `focused` -> `primary` -> `first`) mit Legacy-Fallback auf `active_workspace`; Occupied-State bleibt global.
- Phase-4 Abschluss-Audit dokumentiert: focused-output Switch/Move, output-aware IPC, Shell-State und Panel-Active-Marker sind konsistent; Übergangsgrenzen sind explizit festgehalten (Occupied global, Hotplug offen, `WorkspaceManager.active` Shadow).
- Occupancy-Produktregel finalisiert: Active ist output-aware, Occupied bleibt global; Panel zeigt beide gleichzeitig, Active-Markierung hat Vorrang.
- Hotplug-Policies spezifiziert (ohne Implementierung): klare Add/Remove/Reconfigure/Recovery-Regeln inkl. Phasen H1-H5 in `docs/WORKSPACES.md`.
- Hotplug H1 vorbereitet: `OutputRegistry` unterstützt jetzt remove/reconfigure-API (id/name/remove, reconfigure, contains) mit Unit-Tests; keine Backend-Hotplug-Anbindung in diesem Schritt.
- Hotplug H2 vorbereitet: `WorkspaceOutputState` bereinigt stale Output-Mappings robust und fallbackt `focused_output` deterministisch (`primary -> first`, leer -> `None`); Reconfigure mit stabiler OutputId erhält Fokus/Mapping, Add erzeugt Mapping aus globalem Shadow-Workspace.
- Hotplug H3 vorbereitet: zentraler Compositor-State-Pfad für Output-Änderungen ergänzt (`handle_output_added_or_updated`, `handle_output_removed`, `handle_output_reconfigured`), der `WorkspaceOutputState` synchronisiert und anschließend output-aware `OutputWorkspaceSnapshot` broadcastet.
- Hotplug H4 vorbereitet: Layer-Shell-Recovery-Pfad ergänzt (`reconcile_layer_shell_outputs_after_output_change`) mit sicherem Fallback/No-output-Handling und Reconfigure-Rearrange/Configure; keine Backend-Hotplug-Anbindung.
- H1-H4 Abschluss-Audit dokumentiert: Pipeline-Reihenfolge ist konsistent (Registry -> WorkspaceState sync/fallback -> Layer-Shell recovery -> Snapshot broadcast); konkrete H5-Backend-Hooks sind benannt.
- Hotplug H5a aktiv: Winit-Resize nutzt explizit `handle_output_reconfigured(...)` (stabile OutputId), wodurch die H1-H4-Pipeline im Winit-Reconfigure-Pfad durchlaufen wird.
- Hotplug H5b aktiv: DRM-Notifier triggert einen throttled Connector-Rescan; bekannte Connector-Outputs werden über `handle_output_reconfigured(...)` aktualisiert, neue/fehlende Connectoren werden als deferred Add/Remove geloggt (ohne tatsächliches Add/Remove in H5b).
- Hotplug H5c aktiv: bekannte disconnected DRM-Outputs werden minimal entfernt (DRM-Output-State + `state.outputs`) und über `handle_output_removed(...)` durch die H1-H4-Pipeline geführt (Registry remove, WorkspaceOutputState fallback/cleanup, Layer-Shell-Recovery, OutputWorkspaceSnapshot Broadcast).
- Hotplug H5c-add aktiv: neue connected DRM-Connectoren werden minimal hinzugefügt (Mode-Auswahl `preferred -> first`, neuer `DrmOutput` + `state.outputs`) und über `handle_output_added_or_updated(...)` in Registry/WorkspaceOutputState/Snapshot-Pipeline integriert.
- Hotplug H5d dokumentiert: konsolidierter manueller E2E-Runbook für DRM Reconfigure/Remove/Add + Panel/Layer-Shell/Workspace-State in `docs/DEBUGGING.md`.
- Hotplug H5d Zwischenergebnis (realer Lauf):
  - Initialer DRM Output Add: **pass**
  - Layer-Shell Recovery bei initial add: **pass**
  - OutputWorkspaceSnapshot bei initial add: **pass**
  - Reconfigure: **pending**
  - Runtime Remove: **pending**
  - Runtime Add: **pending**
  - Beobachtung: Warnung `Smithay atomic restore previous state failed` mit `EINVAL`
  - Stabilität: **kein Crash beobachtet**
- NVIDIA Passthrough Ergebnis (VM):
  - PCI sichtbar: `07:00.0 [10de:2783]` + `08:00.0 [10de:22bc]`
  - DRM-Zuordnung: `card0=NVIDIA`, `card1=virtio-gpu`, Connector `card0-HDMI-A-1 connected`
  - Meridian: `Frame rendered` auf `3440x1440@60Hz`, Panel-Layer `3440x36` sichtbar, Layer-Map `surfaces=2`, Pointer auf `drm-0` ok
  - Status: NVIDIA passthrough **pass**, NVIDIA DRM/GBM/EGL render **pass**, Runtime-hotplug **pending**
- NVIDIA Input Zwischenergebnis:
  - Maus: **partial pass** (funktioniert, aber hackelig)
  - Tastatur: **pending/fail** im Meridian-Test
  - Monitor-Hub/KVM als wahrscheinlicher Risikofaktor markiert
  - Bis libinput-/Direct-USB-Vergleich vorliegt: weitere Hardware-Validierung nötig
- NVIDIA DRM Stutter-Diagnose vorbereitet:
  - Per-frame `Frame rendered` Log wurde von `info` auf `debug` reduziert (weniger I/O-Overhead im Normalbetrieb).
  - Neue opt-in Aggregation (`MERIDIAN_DRM_TIMING=1`) liefert 1s-Metriken:
    - `frames`
    - `interval_ms` avg/min/max
    - `render_ms` avg/min/max
    - `commit_ms` avg/min/max
    - `queue_ms` avg/min/max
    - `vblank_wait_ms` avg/min/max
    - zusätzlich: `timer_fire_ms`, `timer_lag_ms`, `tick_ms`, `output_pass_ms`, `vblank_handler_ms`, `frame_submitted_ms`, `queued_pending`, `queue_failures`
  - Repaint-Scheduler-Override für Diagnose ergänzt:
    - `MERIDIAN_DRM_FORCE_REFRESH_HZ=60`
    - `MERIDIAN_DRM_FRAME_INTERVAL_MS=16`
    - Hinweis: beide Flags beeinflussen nur das Repaint-Scheduling (Timer), nicht die KMS/Display-Mode-Auswahl.
    - `MERIDIAN_DRM_FORCE_MODE=1920x1080|2560x1440` (nur Diagnose)
  - DRM-Pfad-Diagnose ergänzt:
    - Mode-Detail-Logs (clock/sync/flags/type)
    - API-Log `drm api selected: path=atomic|legacy`
    - Session-Kontext-Logs (`libseat`, `XDG_SESSION_TYPE`, `SMITHAY_USE_LEGACY`)
  - Commit-Pacing-Schutz ergänzt:
    - pro Output `frame_in_flight`-Gating, um Render/Commit nicht mehrfach zu submitten solange kein VBlank-Submit abgeschlossen ist
    - neuer Timing-Zähler `outputs_skipped_in_flight`
  - Aktueller Befund auf NVIDIA-VM bleibt ~11 FPS (`~90ms` Cadence) trotz 60Hz-Mode-Anzeige; `queue_ms`/`frame_submitted_ms` unauffällig, `commit_ms` bleibt dominant.
  - Realer Before/After-Hardwarevergleich auf NVIDIA-VM bleibt als manueller Lauf offen.
- H6c DRM/KMS-Startup-Gate ergänzt:
  - DRM-Startup loggt Session-/Seat-/VT-/Backend-Kontext, gewählten Node und Primary-Node-Flag.
  - `acquire_master_lock` wird als Diagnose geloggt, ist aber nicht mehr der einzige Gate.
  - Funktionaler Gate ist KMS-Surface-Erzeugung + erster echter KMS-Commit:
    - Commit-Erfolg trotz Lock-Fehler => weiterlaufen mit Warn-Log.
    - Commit-Fehler => fataler Abbruch mit vollem Kontext.
  - Kein Winit-Fallback im DRM-Startup-Pfad ohne Parent-Display.
- H6d Isolation/Noise-Cleanup ergänzt:
  - Layer-surface Commit-Hotpath-Logs von `info` auf `trace/debug` für steady-state reduziert.
  - Auto-Start der Shell ist für Render-Isolation abschaltbar:
    - `MERIDIAN_DRM_DISABLE_SHELL=1` oder `MERIDIAN_NO_SHELL=1`.
  - Zusätzliche Mode-Override-Varianten für Diagnose:
    - `MERIDIAN_DRM_MODE=WxH`
    - `MERIDIAN_DRM_MODE_INDEX=N`
    - `MERIDIAN_DRM_FORCE_MODE` bleibt als Alias nutzbar.
    - Diese Mode-Flags beeinflussen die KMS/Display-Mode-Auswahl; sie sind getrennt von den Repaint-Scheduler-Overrides.
  - Shell-Frame-Callback-Redraw-Loop entfernt: Panel/Launcher committen nicht mehr automatisch pro Compositor-Frame.
  - Opt-in Shell-Repaint-Grundstatistik ergänzt (`MERIDIAN_SHELL_REPAINT_STATS=1`).
- Input-Pfad-Fix ergänzt:
  - `InputEvent::PointerMotion` (relative libinput motion) wird jetzt im DRM-Run verarbeitet.
  - Absolute (`POINTER_MOTION_ABSOLUTE`, QEMU tablet) blieb unverändert funktionsfähig.
- NVIDIA Passthrough Testplan ergänzt: `docs/NVIDIA_PASSTHROUGH.md` (VFIO-Checkliste Host/VM/Guest, DRM/GBM-Meridian-Test, Recovery und Ergebnisprotokoll).
- DRM-Event-Logging präzisiert: VBlank/Scan-Trigger sind jetzt `trace` (weniger Debug-Rauschen), Hotplug-Änderungslogs (`add/remove/reconfigure` + pipeline) bleiben klar sichtbar.
- Layer-Render-Detail-Logs reduziert: per-frame Layer-Map/Layer-Surface/Layer-Element-Zeilen laufen jetzt auf `debug` statt `info`.
- Bestätigter NVIDIA-DRM Root Cause dokumentiert:
  - Vorher lief die VM effektiv im falschen GL-/Treiberpfad (Mesa llvmpipe bzw. inkonsistenter nouveau/nvidia-Modulzustand).
  - `nvidia`/`nvidia_drm` waren initial nicht sauber verfügbar/geladen; dadurch falscher Renderpfad und schlechte Laufzeit.
  - Nach Reboot + korrektem Treiberzustand: `GL_VENDOR=NVIDIA Corporation`, `GL_RENDERER=NVIDIA GeForce RTX 4070 SUPER/PCIe/SSE2`, DRM API `atomic`, stabile Framezeiten.
- DRM-Master-Lock-Check ist jetzt klar als Diagnose markiert; funktionaler KMS-Gate (Surface + erster Commit) ist die entscheidende Erfolgsbedingung.

## Implementierte Features
- Backend-Auswahl: `drm` + `winit` Fallback.
- XDG Shell + Layer Shell + XDG Decoration + SHM + Data Device + XWayland Shell.
- Shell mit Panel/Launcher als eigener Wayland-Client.
- Layer/Render-Z-Order korrigiert (Wallpaper verdeckt Panel nicht mehr).
- IPC zwischen Shell und Compositor (Workspace, Fokus, Launch, Reload).
- Tiling/Floating-Workspace-Logik inkl. Split/Resize.
- Wallpaper-Management mit Modi (`fill/fit/center/tile`) und GPU-Cache.
- Cursor-Theme/Fallback-Pfad aktiv: Theme-Load mit Default-Theme (`default`, Größe `24`) und eingebettetem ARGB-Fallback.
- Workspace-Switching per `Super+1..Super+9` (inkl. Keyboard-Fallback, falls ein Custom-Keybind-Set keine Workspace-Bindings enthält).
- Move-to-workspace per `Super+Shift+1..Super+Shift+9` für das fokussierte Fenster, inkl. Guard-Logs und Refresh/Broadcast nach erfolgreichem Move.
- Panel-Workspace-Indikator aktiv: Anzeige `1..9`, aktiver Workspace visuell markiert; bevorzugt output-aware Active-Workspace-State mit Legacy-Fallback.
- Occupied-Workspace-Status im Panel aktiv: belegte Workspaces werden zusätzlich markiert (ohne neues IPC-Event).
- Occupied-State ist jetzt snapshot-basiert und deterministisch: Compositor sendet einen vollständigen `WindowSnapshot` inkl. Workspace-Zuordnung; Shell berechnet daraus `window_counts`/`occupied_workspaces`.
- Launcher scannt App-Einträge event-getrieben (Start/Toggle), nicht im Render-Hotpath; neue Unit-Tests decken Parse/Ignore/Sort/Filter ab.
- Launcher-Filter folgt näher der freedesktop-Semantik: Desktop-Environment-Listen werden ausgewertet, ignorierte Entries werden nur auf Debug-Level geloggt.
- Launcher startet App-Kommandos jetzt direkt argv-basiert (`Command::new(program).args(args)`); Terminal-Apps werden ohne Shell über einen Terminal-Wrapper (`-e`) gestartet.

## Offene Bugs
- Cursor-Qualität (AA/Skalierung) weiterhin verbesserungsbedürftig, Form aber korrigiert.
- Bei fehlendem Snapshot (z. B. IPC-Ausfall) fällt das Panel auf active-only zurück; Occupied-Markierung wird erst nach Snapshot konsistent.
- Shell-Commit-Rate im Idle bleibt ein offener Quality-/Performance-Punkt:
  - weiterhin hohe `layer-surface-commit`/`surface-commit` Rate beobachtet.
  - mit echtem NVIDIA-Renderer kein akuter Stutter mehr, aber unnötige Commit-Last bleibt.

## Nächste Schritte (geplant)
1. Refactor prüfen
2. Cursor-Qualität verbessern (AA/Skalierung, Theme-Feinschliff)
3. TOML-Konfiguration erweitern (über Minimalumfang hinaus)
4. IPC ausbauen
   - nächster Slice: H5d auf realer DRM-Hardware ausführen und Ergebnisprotokoll erfassen
5. XDG Portals (stufenweise):
   - zuerst Screenshot
   - dann FileChooser-Integration/Delegation
   - dann ScreenCast
6. Multi-Monitor
7. NVIDIA Passthrough (Testplan aktualisieren)
   - nächster manueller Schritt: VFIO-Passthrough-Lauf gemäß `docs/NVIDIA_PASSTHROUGH.md` durchführen und Ergebnisprotokoll ausfüllen
8. Workspace-Policy-Phasen aus `docs/WORKSPACES.md` umsetzen (Phase 1-6)
9. Shell Idle-Commit-Optimierung:
   - konkrete Commit-Reasons aus `MERIDIAN_SHELL_COMMIT_STATS=1` auswerten
   - unnötige Panel-/Launcher-Commits im Idle reduzieren

## Manueller Testhinweis (Workspace-Switching)
1. Meridian starten.
2. `Super+1` bis `Super+9` drücken.
3. Logs auf focused-output-Switch prüfen (`keybind switch workspace for focused output`, `focused-output workspace switch requested`, `old/new workspace`, `compatibility global active updated`).
4. Detaillierter Multi-Output-Ablauf (Pointer-/Fensterfokus + Fallback) in `docs/DEBUGGING.md`, Abschnitt `Manueller E2E-Test: Focused-Output Workspace Switch (Phase 4b)`.

## Manueller Testhinweis (Phase-4 Abschluss)
1. Konsolidierter Ablauf in `docs/DEBUGGING.md`, Abschnitt `Manueller E2E-Test: Phase-4 Abschluss (Switch/Move/Panel/Fallback)`.
2. Deckt Single-Output-Regression, focused-output Switch, Move ohne Auto-Switch, output-aware Panel-Marker und Legacy-Fallback ab.

## Manueller Testhinweis (Move-to-workspace)
1. Zwei Workspaces verwenden.
2. Fenster öffnen.
3. `Super+Shift+2` drücken.
4. Prüfen, dass kein automatischer Wechsel stattfindet.
5. `Super+2` drücken.
6. Prüfen, ob das Fenster dort erscheint.
7. Logs auf focused-output Move prüfen (`keybind move workspace for focused output`, `focused-output move requested`, `workspace move details`, `workspace move completed`).

## Manueller Testhinweis (Panel Workspace-Indikator)
1. Meridian starten.
2. Panel-Sichtbarkeit prüfen.
3. `Super+1`, `Super+2`, `Super+3` drücken.
4. Prüfen, ob die aktive Workspace-Markierung im Panel mitwechselt.
5. Optional: Fenster mit `Super+Shift+N` verschieben und Stabilität der Anzeige prüfen.

## Manueller Testhinweis (Occupied Workspaces)
1. Meridian starten.
2. Fenster auf Workspace 1 öffnen.
3. `Super+Shift+2` drücken (Fenster nach Workspace 2 verschieben).
4. Panel muss Workspace 2 als belegt anzeigen.
5. `Super+2` drücken.
6. Workspace 2 muss aktiv markiert sein.
7. Fenster zwischen mehreren Workspaces verschieben und Panel beobachten.

## Erledigte Kernprobleme
- Panel-Unsichtbarkeit behoben.
- Ursache war falsche Render-Reihenfolge (Wallpaper über UI-Layern).
