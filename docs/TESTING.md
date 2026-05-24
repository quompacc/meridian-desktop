# Testing Guide

## Standardchecks
Diese drei Checks sind die Baseline:
1. `cargo fmt`
2. `cargo test --workspace`
3. `cargo check --workspace`

## Pflicht nach Codeänderungen
- Nach Rust-Codeänderungen: `cargo fmt`
- Nach Rust-Codeänderungen: `cargo check --workspace`
- Bei Test-/Logikänderungen zusätzlich: `cargo test --workspace`

## Wann ausführen
- Vor Commit: immer alle Standardchecks.
- Nach Config-Änderungen: Standardchecks + Config-Parsing/Reload-Tests prüfen.
- Nach IPC-Änderungen: Standardchecks + Snapshot-/IPC-Tests prüfen.
- Nach Shell/Panel-Änderungen: Standardchecks + Shell-State-Tests + manuelle Panel-Checks.
- Nach Render-Z-Order-Änderungen: Standardchecks + manuelle Sichtbarkeits-/Layer-Reihenfolge-Checks.

## Automatisierte Testbereiche
- Config Parsing/Reload:
  - `crates/meridian-config/src/config.rs` (`#[cfg(test)]`)
- Keybinding Parsing/Defaults:
  - `crates/meridian-config/src/keybind/mod.rs` (`#[cfg(test)]`)
- IPC WindowSnapshot:
  - `crates/meridian-ipc/src/lib.rs` (`#[cfg(test)]`)
  - inkl. `LaunchApp` argv-Roundtrip und Legacy-Decode (`command` -> `program`)
- IPC Output-aware Workspace (Phase 4d1):
  - `crates/meridian-ipc/src/lib.rs` (`#[cfg(test)]`)
  - `OutputWorkspaceChanged` roundtrip
  - `OutputWorkspaceSnapshot` roundtrip (mehrere Outputs)
  - Legacy `WorkspaceChanged` roundtrip bleibt stabil
  - `focused_output_id: None` und `output_name: None` bleiben zulässig
- Compositor Output-aware Workspace Snapshot Aufbau (Phase 4d2):
  - `crates/meridian-compositor/src/state/ipc/broadcast.rs` (`#[cfg(test)]`)
  - Snapshot-Aufbau für zwei Outputs (focused/primary/workspace korrekt)
  - Empty-Registry bleibt sicher
  - H3 nutzt diesen bestehenden Snapshot-Build-Pfad nach zentralen Output-State-Änderungen.
- Shell Output-aware Workspace State (Phase 4d3):
  - `crates/meridian-shell/src/wayland/state.rs` (`#[cfg(test)]`)
  - Snapshot mit zwei Outputs wird gespeichert
  - Changed aktualisiert bekannte Outputs
  - Changed für unbekannten Output bleibt sicher (add/reconcile später via Snapshot)
  - Legacy `WorkspaceChanged` bleibt funktionsfähig und überschreibt output-aware Daten nicht
  - Dirty-Flag wird bei output-aware Updates gesetzt
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
- Portal Scaffold:
  - `crates/meridian-portal/src/lib.rs` (`#[cfg(test)]`)
  - D-Bus-Error-Mapping (`NotSupported`/`AccessDenied`) + Bridge-Result-Mapping + Scaffold-Health-State + D-Bus-Konstanten
- Compositor Screenshot-Bridge deny-only:
  - `crates/meridian-compositor/src/state/ipc/screenshot.rs` (`#[cfg(test)]`)
  - invalid request -> `InvalidRequest`, valid request -> `PermissionDenied`, region -> `Unsupported`
- Compositor Screenshot-Policy:
  - `crates/meridian-compositor/src/state/ipc/screenshot_policy.rs` (`#[cfg(test)]`)
  - valid full-output -> `Deny`, region -> `Unsupported`, invalid -> `Invalid`, unknown requester -> `Deny`

Hinweis: `cargo test --workspace` und `cargo check --workspace` enthalten `meridian-portal`.

## Manuelle E2E-Tests (verlinkt)
- Login Realtest:
  - `sudo scripts/test-login-uinput.py --prepare-user --run --lock-user`
  - Nutzt `/dev/uinput` fuer echte Tastatureingabe am DRM-Login, prueft `auth ok`, `compositor spawned`, `ipc handover` und `ipc exit`, startet danach die Loginmaske neu und sperrt den Testnutzer wieder.
  - Login plus Logout-Smoke: `sudo scripts/test-login-uinput.py --prepare-user --run --logout-ipc --lock-user`
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
  - Siehe `docs/XDG_PORTALS.md` (Reihenfolge, Risiken, erster E2E-Backlog)
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
