# Debugging Guide

## Standard-Logging
- Standard: `RUST_LOG=info`
- Detailliert: `RUST_LOG=debug`
- DRM Timing-Aggregation (opt-in): `MERIDIAN_DRM_TIMING=1` (1s-Summary auf `info`)
- Beide Binaries nutzen `tracing_subscriber::fmt::init()`.

## Startbefehle
- Compositor (Root): `cargo run`
- Nur Shell: `cargo run -p meridian-shell`
- Tests: `cargo test --workspace`
- Render-Isolation (ohne Auto-Shell): `MERIDIAN_DRM_DISABLE_SHELL=1 cargo run`
  - Alias: `MERIDIAN_NO_SHELL=1`

## DRM Master / Session Diagnose
- Beim DRM-Start werden jetzt Session-/Seat-Parameter geloggt:
  - `XDG_SESSION_ID`, `XDG_SEAT`, `XDG_VTNR`, `XDG_SESSION_TYPE`, `LIBSEAT_BACKEND`
  - gewählter KMS-Node inkl. Primär-Node-Flag (`selected drm node: path=... primary_node=...`)
  - session-opened FD-Pfad (`drm session-opened fd path: ...`)
- `acquire_master_lock` ist Diagnose, nicht alleiniger Gate:
  - Erfolg: `drm master acquired: ...`
  - Fehler: `diagnostic drm master lock check failed ... functional KMS gate decides startup success`
- Funktionaler Gate:
  - Initiale KMS-Surface-Erzeugung muss gelingen (`drm kms surface created ...`)
  - Erster echter KMS-Commit muss gelingen:
    - Bei Erfolg trotz Master-Lock-Fehler: `diagnostic drm master lock check failed earlier, but functional KMS gate succeeded (first commit ok); continuing`
    - Bei Fehler: fataler Prozessabbruch mit Kontext (kein Winit-Fallback im DRM-Startup).
- Schlussfolgerung:
  - `acquire_master_lock` ist unter session-managed libseat/logind FDs nicht allein zuverlässig als Start-Gate.
  - Der funktionale KMS-Commit ist der maßgebliche Gate.

## Regel: Surface existiert, aber unsichtbar
Immer in dieser Reihenfolge prüfen:
1. Z-Order / Render-Reihenfolge
2. Geometrie
3. Alpha/Opacity
4. Damage/Dirty Flag
5. Output-Zuordnung

## Wayland-Socket prüfen
- `echo $WAYLAND_DISPLAY`
- `echo $XDG_RUNTIME_DIR`
- `ls -l "$XDG_RUNTIME_DIR"/"$WAYLAND_DISPLAY"`

## Multi-Monitor Debugging
- Audit/Modell: `docs/MULTI_MONITOR.md`
- Hotplug-Produktregeln: `docs/WORKSPACES.md` Abschnitt `Hotplug-Policy (verbindlich, vor Implementierung)`
- NVIDIA VFIO-Runbook: `docs/NVIDIA_PASSTHROUGH.md`
- Output-Lifecycle prüfen:
  - DRM init logs für Output-Mode/Refresh lesen (`backend/drm/init.rs`).
  - Render-Logs pro Output lesen (`backend/drm/render.rs`).
- Layer-Shell Output-Zuordnung:
  - `requested_output` vs. tatsächliches `selected output id/name` prüfen (`state/handlers/core/layer_shell.rs`).
  - `fallback_reason` prüfen (`explicit-output`, `fallback-primary`, `fallback-first`, `*-unknown-requested`).
- Pointer absolute motion:
  - Logs `pointer absolute motion` auf `selected_output_id/name` und `fallback` prüfen.
  - Bei Punkten außerhalb aller Output-Geometrien muss Fallback auf `primary()/first()` geloggt werden.
- Pointer button/click:
  - Logs `pointer button output selection requested` auf `selected_output_id/name` und `fallback_reason` prüfen (`point-match`, `fallback-primary`, `fallback-first`, `empty-registry`).
  - Bei Registry/Output-Desync wird `registry output ... not present in active output list` geloggt.
- Surface hit-testing:
  - Logs `surface output selection requested` auf `selected_output_id/name` und `fallback_reason` prüfen (`point-match`, `fallback-primary`, `fallback-first`, `empty-registry`).
  - Bei Registry/Output-Desync wird `registry output ... not present in active output list` geloggt.
- Maximize/Fullscreen:
  - Logs `maximize geometry requested` / `fullscreen geometry requested` prüfen.
  - `selected output ... id/name` und `fallback_reason` prüfen (`window-output`, `fallback-primary`, `fallback-first`).
- Tiling:
  - Log `tiling output geometry requested` prüfen.
  - `tiling selected output: id/name fallback_reason` prüfen (`primary`, `first-fallback`).
- Hotplug:
  - Remove und Add sind minimal über die Hotplug-Pipeline aktiv.
  - Erwartete Zielsemantik bei Implementierung:
    - remove des focused output -> fallback `primary -> first`
    - cleanup von `active_workspace_by_output` für entfernte Outputs
    - kein Fensterverlust, keine implizite Fenster-Migration
- Output-aware Workspace-IPC (Phase 4d2):
  - Logs `output workspace changed broadcasted` und `output workspace snapshot broadcasted` prüfen.
  - Erwartung: Legacy `WorkspaceChanged`/`WindowSnapshot` bleibt parallel aktiv.
- Hotplug-State-Änderungen (H3 vorbereitet):
  - Logs `output hotplug state changed` prüfen (`output-added`, `output-updated`, `output-removed`, `output-reconfigured`).
  - Danach muss `output workspace snapshot broadcasted after output change` folgen.
- Layer-Shell-Recovery (H4 vorbereitet):
  - Bei Remove: `layer-shell output lost` und danach `layer-shell reassigned to fallback output` prüfen.
  - Bei Reconfigure: `layer-shell output reconfigured` prüfen.
  - Wenn kein Output verfügbar: `no output available for layer-shell surface` prüfen.
- Winit-Reconfigure (H5a aktiv):
  - Bei Größenänderung im Winit-Backend muss `winit output resized` geloggt werden.
  - Danach `output reconfigured` und `output workspace snapshot broadcasted after output change`.
  - Erwartung: `layer-shell output reconfigured` folgt für betroffene Layer-Surfaces.
- DRM-Reconfigure (H5b aktiv):
  - VBlank-/Scan-Rauschen läuft auf `trace`; bei `debug/info` sollen primär echte Änderungen sichtbar sein.
  - Bei bekannter Connector-/Mode-Änderung: `drm connector reconfigure detected` und danach `drm output reconfigured via hotplug pipeline`.
  - Bei neuem connected Connector (H5c-add aktiv):
    - `drm output add detected`
    - `drm output add selected mode`
    - `drm output added via hotplug pipeline`
  - Bei fehlendem/disconnected bekanntem Connector (H5c aktiv):
    - `drm output remove detected`
    - `drm output removed via hotplug pipeline`
 - Erwartete Reihenfolge bei Output-Änderung:
   1. `output ...` (added/removed/reconfigured)
   2. `focused output ...` Fallback/Cleanup (falls zutreffend)
   3. `layer-shell ...` Recovery-Logs
   4. `output workspace snapshot broadcasted after output change`
- Shell-Verarbeitung (Phase 4d3):
  - Logs `output workspace snapshot received` und `output workspace changed received` prüfen.
  - Logs `output workspace state available` prüfen.
  - Falls noch kein output-aware State vorhanden: `legacy workspace fallback used` prüfen.

## NVIDIA Input Smoke-Test
- Referenzplan: `docs/NVIDIA_PASSTHROUGH.md` Abschnitt `NVIDIA Input Smoke-Test (vor Runtime-Hotplug)`.
- Nach Monitor-Hub/KVM-Umschaltung zuerst:
  - `lsusb`
  - `sudo libinput list-devices`
  - `journalctl -k -f`
- Live-Eingaben prüfen:
  - `sudo libinput debug-events`
- Entscheidungsregel:
  - Keine Keyboard-Events in libinput => kein Meridian-Keybinding-Problem.
  - Keyboard-Events in libinput, aber keine Meridian-Reaktion => Meridian-Inputpfad prüfen.
- Pointer-Spezifik:
  - `POINTER_MOTION_ABSOLUTE` (QEMU tablet) und `POINTER_MOTION` (USB-Maus) getrennt prüfen.
  - Erwarteter Debug-Log bei relativer Mausbewegung:
    - `pointer relative motion: dx=... dy=... old_x=... old_y=... new_x=... new_y=... selected_output_id=... fallback=...`

## DRM Render/Input Stutter (NVIDIA VM)
Ziel: Render-/Event-Loop-Lag gegen Input-Lag trennen.

### Messlauf 1 (Baseline, wenig Log-Overhead)
- `RUST_LOG=warn cargo run`
- Erwartung: keine per-frame Render-Logs.

### Messlauf 1b (Compositor-Isolation ohne Shell)
- `RUST_LOG=info MERIDIAN_DRM_TIMING=1 MERIDIAN_DRM_DISABLE_SHELL=1 cargo run`
- Alias: `MERIDIAN_NO_SHELL=1`
- Zweck: commit/render-Kosten von Shell-/Layer-Commit-Zyklen isolieren.

### Messlauf 2 (aggregierte Timing-Metriken, low-noise)
- `RUST_LOG=info MERIDIAN_DRM_TIMING=1 cargo run`
- Erwartete 1s-Logzeile:
  - `drm timing summary: ticks=... frames=... interval_ms(avg/min/max)=... render_ms(... ) commit_ms(... ) queue_ms(... ) vblank_wait_ms(... ) ...`

### Messlauf 3 (kurz, detailliert)
- `RUST_LOG=debug MERIDIAN_DRM_TIMING=1 cargo run`
- Nur kurz laufen lassen, um Diagnose-Details zu sehen.

### Auswertung
1. Frame-Intervall:
   - `interval_ms(avg)` nahe `16-17ms` ist gesund bei 60Hz.
   - `~90ms` deutet auf starken Loop-/I/O-Overhead oder Blockierung.
2. Render/Commit:
   - Hohe `render_ms/commit_ms` zeigen GPU/commit-Seite.
3. Event-Wait:
   - Hohe `vblank_wait_ms` bei niedrigen `ticks/frames` zeigt Scheduling-/Wait-Verhalten.
4. Input-Lag:
   - `libinput: client bug: event processing lagging behind` gegen Timing-Summary korrelieren.
5. Shell-Einfluss:
   - Lauf mit/ohne Shell vergleichen.
  - Ohne Shell sollten idle ticks überwiegend leer sein (`frames ~0`, `commit_ms ~0`).

### Bestätigter NVIDIA-Fixlauf
- Vorheriger Engpass war nicht primär der Master-Lock-Check, sondern ein inkonsistenter Treiberpfad (llvmpipe/nouveau/nvidia Zustand).
- Nach Reboot + sauber geladenem NVIDIA-Pfad:
  - `GL Vendor: NVIDIA Corporation`
  - `GL Renderer: NVIDIA GeForce RTX 4070 SUPER/PCIe/SSE2`
  - `drm api selected: path=atomic`
  - `render_ms ~0.6–1.2`, `commit_ms ~0.2–0.4`, `vblank_wait_ms ~16–17`
  - Maus flüssig, Panel sichtbar, kein akuter Stutter.

## Shell Commit Diagnose
- Aktivieren: `MERIDIAN_SHELL_COMMIT_STATS=1`
- In den ersten 5s nach Shell-Start wird jeder Commit auf `info` geloggt:
  - `shell commit: surface=panel|launcher reason=...`
- Zusätzlich 1s-Aggregat:
  - `shell commit summary: total=... panel(...) launcher(...)`
- Offener Punkt: hohe steady-state `layer-surface-commit`/`surface-commit` Rate im Idle weiter analysieren und reduzieren.

## Shell Idle Regression Check (DRM)
Empfohlener Lauf:

```bash
scripts/smoke-drm.sh
```

- Default ist Regression/Smoke mit Timeout (`MERIDIAN_SMOKE_TIMEOUT`, Default 20s).
- Für manuelle Launcher-/UX-Tests ohne Timeout:
  - `scripts/smoke-drm.sh run`
  - oder: `MERIDIAN_SMOKE_MODE=run scripts/smoke-drm.sh`
- Logpfad steuerbar über `MERIDIAN_SMOKE_LOG` (Default `/tmp/meridian-smoke-drm.log`).

### Guter Zustand
- Nach Setup im steady-state:
  - `frames=0`
  - `outputs_skipped_clean≈62/63` (oder ähnlich hoch)
  - `commit_ms=0`
  - `dirty reasons=<none>`
- Clock-Update:
  - einmaliger Frame (`frames=1`, einmal `layer-surface-commit`/`surface-commit`)
  - danach wieder clean idle.

### Schlechter Zustand (Regression)
- Viele `layer-surface-commit`/`surface-commit` pro Sekunde im Idle.
- Dauerhafte `Creating wl_shm buffer...`-Zeilen ohne sichtbare State-Änderung.

## Registry / Layer-Shell Debugging
- Client Bind/Init: `crates/meridian-shell/src/wayland/init.rs`
- Server Layer Mapping: `crates/meridian-compositor/src/state/handlers/core/layer_shell.rs`
- Commit/Arrange/Configure: `crates/meridian-compositor/src/state/handlers/core/compositor.rs`

## Nützliche grep/rg-Befehle
- `rg -n "Panel|Launcher|layer|configure|draw_panel|draw_launcher" crates/meridian-shell/src`
- `rg -n "layer_surface|layer_map|arrange|send_configure" crates/meridian-compositor/src`
- `rg -n "poll_ipc|broadcast_|ShellCommand|ShellEvent" crates`
- `rg -n "keybind|parse_keybind|parse_action|invalid keybind|invalid action" crates/meridian-config/src crates/meridian-compositor/src`
- `rg -n "drm output add detected|drm output added via hotplug pipeline" logs/*.log`
- `rg -n "drm output remove detected|drm output removed via hotplug pipeline" logs/*.log`
- `rg -n "drm connector reconfigure detected|drm output reconfigured via hotplug pipeline" logs/*.log`
- `rg -n "output workspace snapshot broadcasted" logs/*.log`
- `rg -n "layer-shell output lost|layer-shell reassigned|layer-shell output reconfigured|no output available for layer-shell surface" logs/*.log`
- `rg -n "drm timing summary|libinput.*lagging behind|system is too slow" logs/*.log`
- `rg -n "drm startup session context|selected drm node|drm session-opened fd path|drm master acquired|drm master lock check failed|drm kms surface created|initial KMS commit succeeded|fatal drm startup failure" logs/*.log`

## First Checks bei jedem Bug
1. Reproduktion + betroffener Pfad (drm/winit, compositor/shell, input/render/ipc).
2. Logs auf Ereignisgrenze prüfen (kommt Event an?).
3. State-Transitions prüfen (Flags/Focus/Mappings).
4. Erst danach Render-/Buffer-/Protokolldetails.

## Portal-Vorbereitung (später)
Bei zukünftigen XDG-Portal-Bugs zuerst trennen:
1. D-Bus/Portal-Lifecycle (Name, Activation, Request/Response)
2. Policy/Prompt-Entscheidung (allow/deny/cancel)
3. Meridian-Datenpfad (Screenshot-/Output-Quelle)
4. Sandboxed-App-Kontext (Flatpak/Snap)

Referenzplan: `docs/XDG_PORTALS.md`

## Portal FileChooser
Aktueller Zustand:
- `meridian-portal` stellt `org.freedesktop.impl.portal.desktop.meridian`
  auf `/org/freedesktop/portal/desktop` bereit.
- Implementiert ist `org.freedesktop.impl.portal.FileChooser`.
- `OpenFile`, `SaveFile` und `SaveFiles` delegieren an
  `MERIDIAN_FILE_PICKER` oder `/usr/local/bin/meridian-file-picker`.
- Screenshot-Portal ist derzeit nicht ueber `meridian-portal` exponiert;
  die Screenshot-Bridge-Typen existieren nur als deny-only Compositor-Pfad.

### Manueller FileChooser-Smoke
1. Portal starten:
   `RUST_LOG=debug cargo run -p meridian-portal`
2. Sicherstellen, dass `MERIDIAN_FILE_PICKER` gesetzt ist oder
   `/usr/local/bin/meridian-file-picker` existiert.
3. Einen echten `xdg-desktop-portal`-Client gegen den Meridian-Backend-Namen
   testen.
4. Erwartung:
   - Portal loggt `portal service ready`.
   - Picker wird mit Wayland-/Runtime-Umgebung gestartet.
   - Bei Auswahl liefert `OpenFile` `uris`, `SaveFile` `uri`,
     `SaveFiles` `destination`.
   - Cancel liefert Response-Code `1`, Picker-Fehler `2`.

### Screenshot-Bridge (intern, deny-only)
- Compositor kann `ScreenshotBridgeMessage::ScreenshotRequest` ueber den
  bestehenden IPC-Socket parsen und antwortet mit
  `ScreenshotBridgeMessage::ScreenshotResponse`.
- Policy bleibt deny-only (`PermissionDenied` fuer valide Requests,
  `Unsupported` fuer Region, `InvalidRequest` fuer ungueltige Requests).
- Es gibt aktuell keinen passenden `busctl`-Smoke ueber `meridian-portal`.

## Manueller E2E-Test: ReloadConfig
Vorbereitung:
1. `RUST_LOG=debug cargo run` (Compositor starten).
2. Sicherstellen, dass `~/.config/meridian/config.toml` existiert und vom laufenden Prozess lesbar ist.
3. Reload auslösen (über bestehenden Shell-Reload-Trigger).

### Testfall A: Gültige Config mit Theme-/Cursor-/Wallpaper-Änderung
`config.toml` auf gültige Werte setzen, z. B.:
```toml
[general]
theme = "default"

[cursor]
theme = "default"
size = 24

[wallpaper]
path = "/absoluter/pfad/zum/wallpaper.png"
mode = "fill"
```
Erwartete Logs:
- `config reload requested`
- `config reload succeeded`
- `ConfigReloaded { success: true }`
- `shell config reload requested`
- `shell config reload succeeded`
- `panel marked dirty after reload`
- optional je nach Änderung: `theme override changed`, `cursor override changed`, `wallpaper override changed`
Erwartetes Verhalten:
- Panel-Theme aktualisiert ohne Neustart.
- Wallpaper aktualisiert, wenn Pfad gültig ist.
- Cursor wird im DRM-Pfad neu geladen.

### Testfall B: Zweite gültige Änderung direkt danach
Direkt danach `config.toml` erneut auf andere gültige Werte ändern und Reload erneut auslösen.
Erwartete Logs:
- gleiche Erfolgssequenz wie in Testfall A.
Erwartetes Verhalten:
- Zweite visuelle Änderung wird ebenfalls ohne Neustart übernommen.
- Kein Hängenbleiben auf dem ersten Reload-Stand.

### Testfall C: Ungültige Config
`config.toml` absichtlich ungültig machen (Syntaxfehler) und Reload auslösen.
Erwartete Logs:
- `config reload requested`
- `config reload failed; keeping previous config`
- `ConfigReloaded { success: false }`
- `shell config reload requested`
- `shell config reload failed; keeping previous config`
Erwartetes Verhalten:
- Kein Crash.
- Alter visueller Zustand (Theme/Panel) bleibt aktiv.

### Testfall D: Fehlende Config
`~/.config/meridian/config.toml` temporär entfernen und Reload auslösen.
Erwartete Logs:
- `config reload requested`
- Hinweis auf Defaults (fehlende Datei)
- `config reload succeeded`
- `ConfigReloaded { success: true }`
- `shell config reload succeeded`
Erwartetes Verhalten:
- Fallback auf Defaults konsistent zu Compositor-Semantik.
- Panel wird neu gezeichnet und bleibt funktionsfähig.

## Manueller E2E-Test: Focused-Output Workspace Switch (Phase 4b)
Voraussetzung:
1. `RUST_LOG=debug cargo run` (Compositor mit Debug-Logs).
2. Für Multi-Output-Fälle zwei aktive Outputs (z. B. DRM mit zwei Monitoren).

### Testfall A: Single-Output Regression
1. Meridian starten.
2. `Super+1` bis `Super+9` drücken.
3. Erwartung: Verhalten wie vor Phase 4b (sichtbarer Workspace-Wechsel ohne Regression).
4. Erwartete Logs:
   - `keybind switch workspace for focused output`
   - `focused-output workspace switch requested`
   - `focused output id/name`
   - `old workspace`
   - `new workspace`
   - `compatibility global active updated`

### Testfall B: focused_output durch Pointer/Klick setzen
1. Pointer/Klick klar auf Output A setzen.
2. `Super+2` drücken.
3. Erwartung: Switch wirkt für focused output A.
4. Logs müssen `focused output ... A` und `target ... 2` zeigen.

### Testfall C: focused_output durch Fensterfokus setzen
1. Fenster auf Output B fokussieren (falls Multi-Output aktiv).
2. `Super+3` drücken.
3. Erwartung: Switch wirkt für focused output B.
4. Logs müssen `focused output ... B` und `target ... 3` zeigen.

### Testfall D: Fallback bei fehlendem focused_output
1. Zustand herstellen, in dem `focused_output` nicht eindeutig auflösbar ist (z. B. nach Output-Änderung/stale state).
2. `Super+N` drücken.
3. Erwartung: definierter Fallback `primary -> first`.
4. Logs müssen Fallback/Fokusersatz nachvollziehbar zeigen.

### Übergangsgrenzen (bewusst unverändert)
- Panel-Active-Marker nutzt jetzt output-aware State mit Legacy-Fallback.
- Occupied-Workspaces bleiben weiterhin global/snapshot-basiert.
- `WorkspaceChanged`/`WindowSnapshot` enthalten weiterhin keinen `output_id`-Kontext.
- `Super+Shift+1..9` (Move-to-workspace) bleibt im bisherigen Pfad.

## Manueller E2E-Test: Focused-Output Move-to-Workspace (Phase 4c)
Voraussetzung:
1. `RUST_LOG=debug cargo run` (Compositor mit Debug-Logs).
2. Mindestens ein fokussierbares Fenster.

### Testfall A: Move ohne Auto-Switch
1. Fenster auf Workspace 1 fokussieren.
2. `Super+Shift+2` drücken.
3. Erwartung:
   - Fenster wird auf Workspace 2 verschoben.
   - Kein automatischer Workspace-Wechsel auf 2.
4. Danach `Super+2` drücken und prüfen, dass das verschobene Fenster dort sichtbar ist.

### Erwartete Logs
- `keybind move workspace for focused output`
- `focused-output move requested`
- `focused output id/name`
- `workspace move details` (inkl. `window_id`, `title`, `source`, `target`)
- `workspace move completed`

### Guard-Logs
- Kein Fokusfenster:
  - `workspace move ignored, no focused window`
- Ungültiger Ziel-Workspace:
  - `workspace move ignored, invalid workspace`
- Ziel == Quell-Workspace des fokussierten Fensters:
  - `workspace move ignored, already on workspace`

## Manueller E2E-Test: Panel Output-aware Active Workspace (Phase 4e)
Voraussetzung:
1. `RUST_LOG=debug cargo run`
2. Multi-Output bevorzugt; Single-Output für Legacy-Fallback-Regression ebenfalls prüfen.

### Testfall A: Focused Output A bestimmt Panel-Markierung
1. Fokus auf Output A setzen (Pointer/Klick oder fokussiertes Fenster).
2. `Super+2` drücken.
3. Erwartung: Panel markiert Workspace 2 als aktiv (output-aware Selection für focused output A).

### Testfall B: Focused Output B bestimmt Panel-Markierung
1. Fokus auf Output B setzen.
2. `Super+3` drücken.
3. Erwartung: Panel markiert Workspace 3 aktiv entsprechend output-aware State von Output B.

### Testfall C: Legacy-Fallback bei fehlendem output-aware State
1. Single-Output starten oder kurz nach Start auf ersten Legacy-Event achten.
2. Erwartung: Panel-Markierung bleibt funktionsfähig über legacy `active_workspace`.
3. Logs: `legacy workspace fallback used` nur auf Debug-Level.

### Erwartete Logs
- `output workspace snapshot received`
- `output workspace changed received`
- `output workspace state available`
- `panel workspace indicator updated: active_workspace=... legacy_active_workspace=...`

## Manueller E2E-Test: Phase-4 Abschluss (Switch/Move/Panel/Fallback)
Voraussetzung:
1. `RUST_LOG=debug cargo run`
2. Für Multi-Output-Fälle zwei aktive Outputs.

### Schritt 1: Single-Output Regression
1. Meridian mit einem Output starten.
2. `Super+1..9` testen.
3. Erwartung: keine Regression gegenüber Legacy-Verhalten.

### Schritt 2: Focused-output Switch
1. Fokus auf Output A setzen.
2. `Super+2` drücken.
3. Fokus auf Output B setzen.
4. `Super+3` drücken.
5. Erwartung: Workspace-Wechsel folgt jeweils dem fokussierten Output-Kontext.

### Schritt 3: Move ohne Auto-Switch
1. Fenster fokussieren.
2. `Super+Shift+2` drücken.
3. Erwartung: Fenster verschoben, kein automatischer Wechsel.
4. `Super+2` drücken und Sichtbarkeit prüfen.

### Schritt 4: Panel-Active-Marker folgt output-aware State
1. Fokus zwischen Outputs wechseln.
2. `Super+N` drücken.
3. Erwartung: aktive Panel-Markierung folgt dem output-aware Active-Workspace-Selektor.
4. Erwartung Occupancy-Regel:
   - Workspaces mit Fenstern bleiben als occupied markiert (global), auch wenn sie auf dem focused output nicht aktiv sind.
   - Wenn ein Workspace gleichzeitig active + occupied ist, hat active-Markierung visuell Vorrang.

### Schritt 5: Legacy-Fallback
1. Start-/Reconnect-Fall prüfen, bevor output-aware Snapshot ankommt.
2. Erwartung: Panel bleibt über Legacy `active_workspace` nutzbar.
3. Erwarteter Log-Hinweis: `legacy workspace fallback used` (Debug-Level).

## Manueller Test (vor Hotplug-Implementierung): Policy-Validierung
1. Erwartung bei zukünftigem Output-Add:
   - neuer Output startet mit Workspace aus `WorkspaceManager.active`,
   - bestehender `focused_output` bleibt stabil.
2. Erwartung bei zukünftigem Output-Remove:
   - wenn focused output entfernt: Fallback `primary -> first`,
   - `active_workspace_by_output`-Eintrag des entfernten Outputs wird entfernt.
3. Erwartung bei zukünftigem Reconfigure:
   - betroffener Output wird dirty/reconfigured,
   - kein unnötiges globales Redraw aller Outputs.

## Manueller Test: H5a Winit Resize/Reconfigure
1. Meridian im Winit/nested Backend starten (`cargo run` ohne DRM-Session).
2. Fenstergröße des Winit-Fensters ändern.
3. Erwartete Logs:
   - `winit output resized`
   - `output reconfigured`
   - `layer-shell output reconfigured`
   - `output workspace snapshot broadcasted after output change`
4. Erwartung:
   - Panel bleibt sichtbar.
   - Kein Crash/Freeze während Resize.

## Manueller Test: H5b DRM Connector Reconfigure
1. Meridian im DRM-Backend starten (`RUST_LOG=debug cargo run` in VT/DRM-Session).
2. Falls möglich Monitor-Mode wechseln oder Connector-Event auslösen (z. B. Replug ohne dauerhaften Remove-Support erwarten).
3. Erwartete Logs:
   - `drm connector scan triggered` (nur bei `trace` sichtbar)
   - `drm connector reconfigure detected` (bei bekannten Outputs)
   - `drm output reconfigured via hotplug pipeline`
   - `output workspace snapshot broadcasted after output change`
4. Erwartung:
   - Kein Crash.
   - Panel bleibt sichtbar.

## Manueller Test: H5c DRM Output Remove (minimal)
1. Meridian im DRM-Backend starten (`RUST_LOG=debug cargo run` in VT/DRM-Session).
2. Einen bereits bekannten Monitor trennen/disconnecten.
3. Erwartete Logs:
   - `drm output remove detected`
   - `drm output removed via hotplug pipeline`
   - `focused output fallback ...` (falls der entfernte Output fokussiert war)
   - `layer-shell ...` Recovery-Logs
   - `output workspace snapshot broadcasted after output change`
4. Erwartung:
   - Kein Crash, auch wenn nur ein Output übrig bleibt.
   - Bei letztem entfernten Output bleibt der Zustand kontrolliert (`focused_output` wird geleert).
5. Hinweis:
   - Output-Add wird separat in H5c-add behandelt.

## Manueller Test: H5c-add DRM Output Add (minimal)
1. Meridian im DRM-Backend starten (`RUST_LOG=debug cargo run` in VT/DRM-Session).
2. Einen neuen Monitor verbinden/connecten.
3. Erwartete Logs:
   - `drm output add detected`
   - `drm output add selected mode`
   - `output registered` (aus `handle_output_added_or_updated`)
   - `drm output added via hotplug pipeline`
   - `output workspace snapshot broadcasted after output change`
4. Erwartung:
   - Kein Crash.
   - Panel/Layer-Shell bleiben stabil.

## Manueller Test: H5d DRM Hotplug E2E (Reconfigure/Remove/Add)
Voraussetzungen:
1. Reale DRM-Session (VT), nicht nested.
2. `RUST_LOG=debug`.
3. Mindestens ein aktiver Output; für Add/Remove idealerweise zweiter Monitor/Hotplug-fähiger Anschluss.

### Testfall A: Reconfigure
1. Auf bekanntem Output Mode/Resolution/Refresh ändern.
2. Erwartete Logs:
   - `drm connector scan triggered` (nur bei `trace` sichtbar)
   - `drm connector reconfigure detected`
   - `output reconfigured`
   - `layer-shell output reconfigured`
   - `output workspace snapshot broadcasted after output change`

### Testfall B: Remove
1. Bekannten Output disconnecten.
2. Erwartete Logs:
   - `drm output remove detected`
   - `drm output removed via hotplug pipeline`
   - `stale output workspace mapping removed` (falls Mapping vorhanden)
   - `focused output fallback ...` oder `focused output cleared ...`
   - `layer-shell ...` Recovery-Logs
   - `output workspace snapshot broadcasted after output change`

### Testfall C: Add
1. Output wieder verbinden oder neuen Output verbinden.
2. Erwartete Logs:
   - `drm output add detected`
   - `drm output add selected mode`
   - `output registered`
   - `drm output added via hotplug pipeline`
   - `output workspace snapshot broadcasted after output change`

### Testfall D: Panel/Layer-Shell Stabilität
1. Während A-C Panel beobachten.
2. Erwartung:
   - Panel bleibt sichtbar oder fällt sauber auf verfügbaren Output.
   - Kein Panic.
   - Keine Z-Order-Regression (Wallpaper bleibt unter UI).

### Testfall E: Workspace-State nach Hotplug
1. Nach Add/Remove `Super+1..9` testen.
2. Panel Active Marker prüfen.
3. Erwartung:
   - Workspace-Switch bleibt funktionsfähig.
   - Active Marker bleibt konsistent.
   - Occupied bleibt global.

### Ergebnisprotokoll (pro Lauf ausfüllen)
- Testumgebung: `<GPU/Connectoren/Kernel>`
- A Reconfigure: `pass|fail|pending`
- B Remove: `pass|fail|pending`
- C Add: `pass|fail|pending`
- D Panel/Layer-Shell: `pass|fail|pending`
- E Workspace-State: `pass|fail|pending`
- Beobachtete Abweichungen/Risiken: `<kurz>`

### H5d Zwischenergebnis (realer Lauf)
- Initialer DRM Output Add: `pass`
- Layer-Shell Recovery bei initial add: `pass`
- OutputWorkspaceSnapshot bei initial add: `pass`
- Reconfigure: `pending`
- Runtime Remove: `pending`
- Runtime Add: `pending`
- Warnung beobachtet: `Smithay atomic restore previous state failed` mit `EINVAL`
- Stabilität: kein Crash beobachtet

### NVIDIA Passthrough Zwischenergebnis (VM)
- PCI sichtbar:
  - `07:00.0 NVIDIA RTX 4070 SUPER [10de:2783]`
  - `08:00.0 NVIDIA Audio [10de:22bc]`
- DRM-Zuordnung:
  - `card0 = NVIDIA (vendor 0x10de, device 0x2783)`
  - `card1 = virtio-gpu`
- Connector:
  - `card0-HDMI-A-1 connected`
- Meridian:
  - `Frame rendered` auf `3440x1440@60Hz`
  - Panel layer surface `3440x36` sichtbar/gemappt
  - Layer map `surfaces=2`
  - Pointer input auf `drm-0` funktioniert

## Gelöste Bugs
### Panel war unsichtbar
- Status: gelöst.
- Symptom: Panel-Surface wurde erzeugt und war grundsätzlich funktionsfähig, blieb aber unsichtbar.
- Tatsächliche Ursache: falsche Render-Reihenfolge.
- Effekt: Wallpaper wurde nach/über dem Panel gerendert und hat das Panel verdeckt.
- Nicht-Ursache: Layer-Shell, Registry und SCTK-Bindings.
- Fix-Regel: Background/Wallpaper muss vor normalen Fenstern und Layer-Shell-UI gerendert werden.
