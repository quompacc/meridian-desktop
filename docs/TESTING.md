# Testing Guide

## Standardchecks
Diese Checks sind die Baseline und werden vom pre-push-Hook
(`.githooks/pre-push`, aktivieren mit `git config core.hooksPath .githooks`)
und der GitHub-CI erzwungen:
1. `cargo fmt --all -- --check`
2. `cargo test --workspace`
3. `cargo check --workspace`
4. `cargo clippy --workspace --all-targets -- -D warnings`

## Pflicht nach Codeänderungen
- Nach Rust-Codeänderungen: `cargo fmt`
- Nach Rust-Codeänderungen: `cargo check --workspace`
- Nach Rust-Codeänderungen: `cargo clippy --workspace --all-targets -- -D warnings`
- Bei Test-/Logikänderungen zusätzlich: `cargo test --workspace`

## Wann ausführen
- Vor Commit: immer alle Standardchecks.
- Nach Config-Änderungen: Standardchecks + Config-Parsing/Reload-Tests prüfen.
- Nach IPC-Änderungen: Standardchecks + Snapshot-/IPC-Tests prüfen.
- Nach Shell/Panel-Änderungen: Standardchecks + Shell-State-Tests + manuelle Panel-Checks.
- Nach Login-Änderungen: Standardchecks + Login-Unit-Tests + passenden uinput-Smoke.
- Nach Portal-Änderungen: Standardchecks + Portal-Unit-Tests + D-Bus/Picker-Smoke.
- Nach Render-Z-Order-Änderungen: Standardchecks + manuelle Sichtbarkeits-/Layer-Reihenfolge-Checks.

## Automatisierte Testbereiche
- Config Parsing/Reload:
  - `crates/meridian-config/src/config.rs` (`#[cfg(test)]`)
  - Primary-Output-Writer setzt genau einen `outputs.*.primary = true`,
    ergaenzt fehlende Primary-Zeilen und haengt unbekannte Outputs minimal an.
- Keybinding Parsing/Defaults:
  - `crates/meridian-config/src/keybind/mod.rs` (`#[cfg(test)]`)
- IPC WindowSnapshot:
  - `crates/meridian-ipc/src/lib.rs` (`#[cfg(test)]`)
  - inkl. `LaunchApp` argv-Roundtrip und Legacy-Decode (`command` -> `program`)
- IPC Output-aware Workspace (Phase 4d1):
  - `crates/meridian-ipc/src/lib.rs` (`#[cfg(test)]`)
  - `OutputWorkspaceChanged` roundtrip
  - `OutputWorkspaceSnapshot` roundtrip (mehrere Outputs inkl. Geometry/Scale/
    Transform/Refresh fuer Display-Settings)
  - Legacy-Decode alter `OutputWorkspaceSnapshot`-Payloads ohne Display-Details
    bleibt stabil.
  - Legacy `WorkspaceChanged` roundtrip bleibt stabil
  - `focused_output_id: None` und `output_name: None` bleiben zulässig
- Compositor Output-aware Workspace Snapshot Aufbau (Phase 4d2):
  - `crates/meridian-compositor/src/state/ipc/broadcast.rs` (`#[cfg(test)]`)
  - Snapshot-Aufbau für zwei Outputs (focused/primary/workspace sowie
    Geometry/Scale/Transform/Refresh korrekt)
  - Empty-Registry bleibt sicher
  - H3 nutzt diesen bestehenden Snapshot-Build-Pfad nach zentralen Output-State-Änderungen.
- Shell Output-aware Workspace State (Phase 4d3):
  - `crates/meridian-shell/src/wayland/state.rs` (`#[cfg(test)]`)
- Shell Display Settings:
  - `crates/meridian-shell/src/widget_action.rs` (`#[cfg(test)]`)
  - `display-primary-N` wird als Primary-Output-Aktion erkannt.
  - Snapshot mit zwei Outputs wird gespeichert
  - Changed aktualisiert bekannte Outputs
  - Changed für unbekannten Output bleibt sicher (add/reconcile später via Snapshot)
  - Legacy `WorkspaceChanged` bleibt funktionsfähig und überschreibt output-aware Daten nicht
  - Dirty-Flag wird bei output-aware Updates gesetzt
- Shell Printers Settings:
  - `crates/meridian-shell/src/printers.rs` (`#[cfg(test)]`)
  - `lpstat`-Parser deckt Default-Drucker, enabled/disabled, accepting-state
    und Queue-Zaehler ab.
  - Live-Pfad: `test-login-uinput.py --run --keep-session`, Launcher,
    Settings, `System -> Printers`, danach `--logout-ipc --lock-user`.
- Shell Sound Settings / Panel Audio Tray:
  - `crates/meridian-shell/src/audio.rs` (`#[cfg(test)]`)
  - `crates/meridian-shell/src/audio_popup.rs` (`#[cfg(test)]`)
  - `wpctl status`-Parser deckt Default-Output/Input, Lautstaerke und Mute ab.
  - `panel-sound` wird als Panel-Klickaktion erkannt und toggelt die
    Sound-Karte; der Settings-Link in der Karte oeffnet `System -> Sound`.
  - Live-Pfad: `test-login-uinput.py --run --keep-session`, Klick auf
    Panel-Sound-Chip per
    `test-login-uinput.py --panel-click panel-sound --panel-click-count N`
    mehrfach wiederholen, Settings-Link in der Karte testen, `wpctl status`
    fuer die fakeuser-Session pruefen, danach `--logout-ipc --lock-user`.
- Shell Panel Click Harness:
  - `meridian-shell` schreibt die aktuellen Panel-Click-Zonen nach
    `/run/user/$UID/meridian-panel-click-zones.json`.
  - `test-login-uinput.py --panel-click <id>` klickt die Zone anhand dieser
    Datei und rechnet Panel-lokale Koordinaten auf globale Screen-Koordinaten
    um. Damit keine geratenen Pixel fuer `panel-sound`, `panel-clock`,
    `panel-network` usw. verwenden.
- Shell StatusNotifierItem Watcher:
  - `crates/meridian-shell/src/status_notifier.rs` (`#[cfg(test)]`)
  - Live-Pfad: Login mit `--keep-session`, dann als fakeuser:
    `busctl --user introspect org.kde.StatusNotifierWatcher /StatusNotifierWatcher org.kde.StatusNotifierWatcher`
    und ein Fake-Item auf `org.example.MeridianFakeTray` exportieren, das
    `/StatusNotifierItem` mit `Title` und `IconName` bereitstellt, danach
    `RegisterStatusNotifierItem` beim Watcher aufrufen.
  - Im Shell-Journal muss der Register-Log `title=... icon_name=...`
    enthalten; fehlt der Fake-Service oder die Properties, faellt das Panel
    auf den Service-Namen als Label zurueck.
  - Danach muss `/run/user/$UID/meridian-panel-click-zones.json` eine
    `panel-sni-0` Zone mit `activate-status-notifier-item-0` enthalten;
    Linksklicktest per `test-login-uinput.py --panel-click panel-sni-0`.
  - Fuer Activate/SecondaryActivate/ContextMenu ein Fake-Item mit
    `Activate(i32, i32)`, `SecondaryActivate(i32, i32)` und
    `ContextMenu(i32, i32)` verwenden und die empfangenen Koordinaten pruefen.
    Bei 1280x800 soll ein `panel-sni-0` Klick unten im Panel globale
    Koordinaten um `x=939,y=779` liefern, nicht panel-lokales `y=21`.
    Mittelklicktest per
    `test-login-uinput.py --panel-click panel-sni-0 --panel-click-button middle`,
    Rechtsklicktest per
    `test-login-uinput.py --panel-click panel-sni-0 --panel-click-button right`.
  - Fuer DBusMenu-Probing muss das Fake-Item `Menu=/Menu` anbieten und auf
    `/Menu` die Schnittstelle `com.canonical.dbusmenu` mit `GetLayout`
    exportieren. Beim Rechtsklick muss das Shell-Journal
    `status-notifier: dbusmenu layout fetched` mit Revision, Root-ID und
    Child-Anzahl enthalten.
- Panel Active-Workspace Auswahl (Phase 4e):
  - `crates/meridian-shell/src/wayland/state.rs` (`#[cfg(test)]`)
  - Auswahlreihenfolge: `focused_output_id` -> `focused` -> `primary` -> `first` -> legacy
  - Out-of-range Workspace-Werte werden auf `1..=9` normalisiert
- IPC Screenshot-Bridge-Contract:
  - `crates/meridian-ipc/src/lib.rs` (`#[cfg(test)]`)
  - Full-Output-Request, leere/ungültige Requests, Region-`Unsupported`, Request-Metadaten-Roundtrip, Command/Event-Roundtrip für `ScreenshotBridge`
- Shell Workspace-/Occupied-State:
  - `crates/meridian-shell/src/wayland/state.rs` (`#[cfg(test)]`)
- Launcher Parsing/Filter/Sort:
  - `crates/meridian-shell/src/launcher.rs` (`#[cfg(test)]`)
  - inkl. `OnlyShowIn/NotShowIn`, `Exec`-Bereinigung/argv-Parsing (Fieldcodes/Quotes), `TryExec`-Checks
- Workspace-Switch-Guards (Compositor):
  - `crates/meridian-compositor/src/workspace.rs` (`#[cfg(test)]`)
- Output-Registry (Compositor):
  - `crates/meridian-compositor/src/state/output_registry.rs` (`#[cfg(test)]`)
  - register/list, primary/first fallback, by_id, output_at_point, leere Registry
  - remove_by_id/remove_by_name no-op safety
  - reconfigure behält ID und aktualisiert Geometry/Scale/Transform/Refresh
  - ID-Nicht-Wiederverwendung nach remove+add
- Workspace Output State (Phase 1 Vorbereitung):
  - `crates/meridian-compositor/src/state/workspace_output_state.rs` (`#[cfg(test)]`)
  - focused_output init, per-output workspace mapping, unknown-output fallback, set/get mapping, stale mapping cleanup
  - read-path fallback tests: focused output mapping read, missing focused output fallback, missing mapping fallback
  - invalid target for per-output workspace mapping is ignored
  - Hotplug-State-Recovery (H2): remove focused output -> primary/first fallback, empty registry -> focused `None`, reconfigure (gleiche ID) erhält Fokus/Mapping, add neuer Output nutzt global active mapping
- Pointer absolute output selection:
  - `crates/meridian-compositor/src/input/pointer/mod.rs` (`#[cfg(test)]`)
  - Punkt auf Output 1/2 sowie außerhalb -> `primary` fallback
  - Focus-Update-Kandidat: außerhalb aller Outputs -> kein `focused_output`-Update
- Pointer button output selection:
  - `crates/meridian-compositor/src/input/pointer/button.rs` (`#[cfg(test)]`)
  - Klickpunkt auf Output 1/2, außerhalb -> `primary`, ohne primary -> `first`, leere Registry sicher
- XDG maximize/fullscreen output selection:
  - `crates/meridian-compositor/src/state/handlers/xdg/requests/state.rs` (`#[cfg(test)]`)
  - `primary` fallback, `first` fallback ohne primary, leere Liste sicher
- Tiling output selection:
  - `crates/meridian-compositor/src/state/layout/tiling.rs` (`#[cfg(test)]`)
  - `primary` gewählt, `first` fallback ohne primary, leere Liste sicher
- Layer-Shell output fallback selection:
  - `crates/meridian-compositor/src/state/handlers/core/layer_shell.rs` (`#[cfg(test)]`)
  - explicit output wins, unknown requested output fallback, primary/first fallback, empty registry sicher
  - Recovery-Helfer (H4): lost output -> primary/first fallback, no outputs safe none, reconfigure same output -> keep assignment
- Surface hit-testing output selection:
  - `crates/meridian-compositor/src/state/layout/surface.rs` (`#[cfg(test)]`)
  - Punkt auf Output 1/2, `primary` fallback, `first` fallback ohne primary, leere Registry sicher
- Portal Service:
  - `crates/meridian-portal/src/lib.rs` (`#[cfg(test)]`)
  - D-Bus-Konstanten und Service-Startpfad
- Portal FileChooser:
  - `crates/meridian-portal/src/file_chooser.rs` (`#[cfg(test)]`)
  - `file://` URI-Erzeugung und Percent-Encoding
- Compositor Screenshot-Bridge deny-only:
  - `crates/meridian-compositor/src/state/ipc/screenshot.rs` (`#[cfg(test)]`)
  - invalid request -> `InvalidRequest`, valid request -> `PermissionDenied`, region -> `Unsupported`
- Compositor Screenshot-Policy:
  - `crates/meridian-compositor/src/state/ipc/screenshot_policy.rs` (`#[cfg(test)]`)
  - valid full-output -> `Deny`, region -> `Unsupported`, invalid -> `Invalid`, unknown requester -> `Deny`
- Shell screenshots:
  - `crates/meridian-shell/src/wayland/screencopy.rs` (`#[cfg(test)]`)
  - XRGB->RGB PNG-Encoding und Buffer-Layout-Grenzen
- Shell widget actions:
  - `crates/meridian-shell/src/widget_action.rs` (`#[cfg(test)]`)
  - Launcher-, Settings-, Power- und Pinned-App-Action-Mapping
- Shell UI drawing smoke tests:
  - `crates/meridian-shell/src/panel_view.rs`
  - `crates/meridian-shell/src/context_menu.rs`
  - `crates/meridian-shell/src/ui_preview.rs`
  - weitere View-/Popup-Module nach betroffenem Slice
- Login:
  - `crates/meridian-login/src/main.rs` (`#[cfg(test)]`)
  - Smartcard/YubiKey-Hints, Power-Confirmation, Click-Targets
  - `crates/meridian-login/src/input.rs` fuer evdev/xkb-Input-Helfer
  - `crates/meridian-login/src/session.rs` fuer Session-Fehlerpfade
- Output power:
  - `crates/meridian-compositor/src/state/output_power.rs` (`#[cfg(test)]`)
  - On/Off-State, Last-output-Guard, forget/reproject

Hinweis: `cargo test --workspace` und `cargo check --workspace` enthalten `meridian-portal`.

## Manuelle E2E-Tests (verlinkt)
- Login Realtest:
  - `sudo scripts/test-login-uinput.py --prepare-user --run --lock-user`
  - Nutzt `/dev/uinput` fuer echte Tastatureingabe am DRM-Login, prueft `auth ok`, `compositor spawned`, `ipc handover` und `ipc exit`, startet danach die Loginmaske neu und sperrt den Testnutzer wieder.
  - Login plus Logout-Smoke: `sudo scripts/test-login-uinput.py --prepare-user --run --logout-ipc --lock-user`
  - Login plus UI-Logout-Smoke: `sudo scripts/test-login-uinput.py --prepare-user --run --logout-ui --lock-user`
- ReloadConfig E2E:
  - Siehe `docs/DEBUGGING.md`, Abschnitt `Manueller E2E-Test: ReloadConfig`
- Workspace Switch/Move:
  - Siehe `docs/PROJECT_STATUS.md`, Abschnitt `Manueller Testhinweis (Workspace-Switching)` und `Manueller Testhinweis (Move-to-workspace)`
  - Focused-Output-Semantik (Phase 4b): `docs/DEBUGGING.md`, Abschnitt `Manueller E2E-Test: Focused-Output Workspace Switch (Phase 4b)`
  - Focused-Output Move (Phase 4c): `docs/DEBUGGING.md`, Abschnitt `Manueller E2E-Test: Focused-Output Move-to-Workspace (Phase 4c)`
  - Panel output-aware Marker (Phase 4e): `docs/DEBUGGING.md`, Abschnitt `Manueller E2E-Test: Panel Output-aware Active Workspace (Phase 4e)`
  - Phase-4 Abschlusslauf: `docs/DEBUGGING.md`, Abschnitt `Manueller E2E-Test: Phase-4 Abschluss (Switch/Move/Panel/Fallback)`
- Panel Active/Occupied:
  - Siehe `docs/PROJECT_STATUS.md`, Abschnitt `Manueller Testhinweis (Panel Workspace-Indikator)` und `Manueller Testhinweis (Occupied Workspaces)`
  - Für die finale Produktregel (active output-aware, occupied global, active hat Vorrang): `docs/DEBUGGING.md`, Abschnitt `Manueller E2E-Test: Phase-4 Abschluss (Switch/Move/Panel/Fallback)`
- XDG Portals (Planungsstand):
  - Siehe `docs/XDG_PORTALS.md`.
  - FileChooser ist implementiert und sollte mit `MERIDIAN_FILE_PICKER` gegen einen realen Picker getestet werden.
  - Screenshot/ScreenCast bleiben offen.
- Multi-Monitor Audit:
  - Siehe `docs/MULTI_MONITOR.md` (Ist-Zustand, Zielmodell, Risiken, nächster Slice)
  - NVIDIA VFIO-Hardwarelauf: `docs/NVIDIA_PASSTHROUGH.md`
  - Aktueller NVIDIA-Status: passthrough + DRM/GBM/EGL `pass`, Runtime-hotplug `pending` (siehe `docs/NVIDIA_PASSTHROUGH.md`, Abschnitt `Aktuelles Ergebnis`)
  - NVIDIA Input-Smoke-Test: `docs/NVIDIA_PASSTHROUGH.md`, Abschnitt `NVIDIA Input Smoke-Test (vor Runtime-Hotplug)`
  - DRM Render/Input-Stutter-Messung: `docs/DEBUGGING.md`, Abschnitt `DRM Render/Input Stutter (NVIDIA VM)` (`MERIDIAN_DRM_TIMING=1`)
  - Relative-vs-absolute Pointer-Recheck: `POINTER_MOTION` (USB-Maus) und `POINTER_MOTION_ABSOLUTE` (QEMU tablet) getrennt verifizieren
  - Hotplug-Policy-Spezifikation: `docs/WORKSPACES.md`, Abschnitt `Hotplug-Policy (verbindlich, vor Implementierung)`
  - Manueller Policy-Check (vor Implementierung): `docs/DEBUGGING.md`, Abschnitt `Manueller Test (vor Hotplug-Implementierung): Policy-Validierung`
  - H5a (aktiv): Winit Resize/Reconfigure manuell prüfen (`docs/DEBUGGING.md`, Abschnitt `Manueller Test: H5a Winit Resize/Reconfigure`)
  - H5b (aktiv): DRM Connector-Reconfigure manuell prüfen (`docs/DEBUGGING.md`, Abschnitt `Manueller Test: H5b DRM Connector Reconfigure`)
  - H5c (aktiv): DRM Output-Remove minimal manuell prüfen (`docs/DEBUGGING.md`, Abschnitt `Manueller Test: H5c DRM Output Remove (minimal)`)
  - H5c-add (aktiv): DRM Output-Add minimal manuell prüfen (`docs/DEBUGGING.md`, Abschnitt `Manueller Test: H5c-add DRM Output Add (minimal)`)
  - H5d (dokumentiert): manueller DRM-Hotplug-E2E-Lauf (`docs/DEBUGGING.md`, Abschnitt `Manueller Test: H5d DRM Hotplug E2E (Reconfigure/Remove/Add)`)

## Hinweis
Dieses Dokument ist ein Index. Detailabläufe und erwartete Logs bleiben in den jeweiligen Fachdokumenten.
