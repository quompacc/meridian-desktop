# Architecture

## Workspace-Struktur
- `src/main.rs`: Startpunkt (Backend-Wahl, XWayland, Timers, Shell-Watchdog).
- `crates/meridian-compositor`: Wayland-Compositor, Backends, State, Render, Input.
- `crates/meridian-shell`: Panel/Launcher (Layer-Shell Client).
- `crates/meridian-config`: Theme/Keybind-Konfiguration.
- `crates/meridian-ipc`: IPC-Protokoll (`ShellCommand`, `ShellEvent`) + gemeinsame Screenshot-Bridge-Contract-Typen.
- `crates/meridian-wm`: Tiling/Floating-Workspace-Logik.
- `crates/meridian-portal`: separates Portal-Backend-Binary (aktuell Scaffold ohne Featurelogik).

## Modulstruktur (aktuell)
- `meridian-compositor/backend/drm/*`: Init/GPU/Render (inkl. `render/layers.rs`, `render/stack.rs`)
- `meridian-compositor/backend/winit/*`: Winit-Init, Layer-Erfassung, Scene-Komposition
- `meridian-compositor/state/*`: `setup`, `utils`, `layout/*`, `ipc/*`, `handlers/*`
- `meridian-compositor/wallpaper/*`: `manager`, `compose`, `gpu`
- `meridian-shell/wayland/*`: `init`, `render`, `state`, `handlers/*`, `ipc`
- `meridian-shell/draw/*`: `painter`, `text`, `bitmap`, `ft`, `fc`

## Startpfad
1. `src/main.rs`
2. `backend::drm::init_drm` oder `backend::winit::init_winit`
3. `state::MeridianState` + Handler
4. Renderpfad (`backend/drm/render/*` oder `backend/winit/*`)

## Compositor / Shell / IPC
- `meridian-compositor` verwaltet Surfaces, Input, Workspaces, Rendering.
- Output-Metadaten werden zusätzlich zentral in `OutputRegistry` gespiegelt (read-only), während `outputs: Vec<Output>` unverändert bestehen bleibt.
- `meridian-shell` rendert Panel/Launcher als Layer-Surfaces.
- IPC koppelt beide:
  - Compositor -> Shell: Workspace/Window/Focus/ToggleLauncher Events.
  - Shell -> Compositor: SwitchWorkspace/FocusWindow/LaunchApp/ReloadConfig.
- Aktueller Workspace-IPC-Stand:
  - legacy/global: `WorkspaceChanged { workspace }`
  - snapshot-basiert: `WindowSnapshot { active_workspace, windows[] }`
- Geplante Phase 4d (Spezifikation):
  - output-aware Workspace-Kontext zusätzlich zu legacy einführen (Parallelbetrieb).
  - Zielpfad: Shell/Panel lesen per-output Active-Workspace-State aus output-aware Snapshot/Event.
  - Details und Übergangsregeln: `docs/WORKSPACES.md` Abschnitt `Phase 4d IPC-Workspace-Kontext (Spezifikation)`.

## Backends
- `drm`: KMS/GBM/GLES, Hauptpfad für echte Session.
- `winit`: Fallback/Embedded-Session, gleiche State- und Renderprinzipien.

## Wayland-Protokolle
- XDG Shell (+ Popups/Toplevel)
- WLR Layer Shell
- XDG Decoration
- SHM
- Output/XDG-Output
- Data Device / DnD
- XWayland Shell

## Layer-Shell-Orte
- Server-seitig: `crates/meridian-compositor/src/state/handlers/core/layer_shell.rs`
- Client-seitig (Shell): `crates/meridian-shell/src/wayland/handlers/layer.rs`
- Status: Layer-Shell ist funktionsfähig genug für sichtbares Panel und Launcher.

## Render-Layer-Reihenfolge (Korrektheitsregel)
Die visuelle Stapelung ist verbindlich:
1. background / wallpaper
2. bottom layer surfaces
3. normal application windows
4. top layer surfaces / panel
5. overlay surfaces / launcher
6. cursor

## Keybinding-Orte
- Parsing/Defaults: `crates/meridian-config/src/keybind/*`
- Ausführung: `crates/meridian-compositor/src/input/keyboard.rs`
- Runtime-Reload: `crates/meridian-compositor/src/state/ipc/commands.rs`

## Performance-sensitive Bereiche
- `backend/drm/render/*`
- `backend/winit/*`
- `decoration/render/*`
- `wallpaper/*` (CPU compose + GPU upload)
- `state/handlers/*` (Commit/Input-Frequenzpfade)

## XDG-Portal Architektur (Plan)
- Referenz: `docs/XDG_PORTALS.md`
- Empfehlung:
  - eigener Portal-Prozess statt Integration in den Compositor-Renderpfad.
  - Compositor bleibt für Capture-/State-Quellen zuständig, nicht für D-Bus-Policy.
  - Portal-Policy, Prompting und Session-Lifecycle liegen im Portal-Backend.
- Benötigte Meridian-Daten für spätere Portals:
  - Screenshot/ScreenCast: Output-Infos + kontrollierter Framezugriff.
  - Settings/Appearance: read-only Theme/Appearance-Werte.
  - OpenURI/FileChooser: Delegationspfad über Shell/externe Handler.
- Sicherheitsregel:
  - minimale Datenfreigabe pro Request, kein globaler Freigabepfad.
- Aktueller Screenshot-Bridge-Contract:
  - `ScreenshotBridgeRequest` mit `request_id`, `kind=full-output`, optionalem `output`, `include_cursor`, optional `region`.
  - `ScreenshotBridgeError` mit `Unsupported`, `PermissionDenied`, `CompositorUnavailable`, `InvalidRequest`, `Internal`.
  - Vorerst kein Capture-Transport; Portal bleibt deny-only/unsupported.

## Multi-Monitor (Audit)
- Referenz: `docs/MULTI_MONITOR.md`
- Workspace-Policy-Referenz: `docs/WORKSPACES.md`
- Aktueller Kern:
  - Outputs werden in `MeridianState.outputs` gehalten.
  - Rendering läuft pro DRM-Output, Workspace-State ist aktuell noch global aktiv.
  - First-output-Fallback-Pfade in Pointer/Surface/Maximize/Fullscreen/Tiling/Layer-Shell sind auf OutputRegistry-Policy migriert.
- Architekturentscheidung spezifiziert:
  - Zielmodell ist Hybrid (`active_workspace_by_output` + `focused_output`) gemäß `docs/WORKSPACES.md`.
- Hotplug-Policy spezifiziert (noch nicht vollständig implementiert):
  - Output Add/Remove/Reconfigure + Recovery-Regeln sind verbindlich in `docs/WORKSPACES.md` definiert.
  - Implementierung folgt in Phasen H1-H5 (Registry API -> State cleanup -> Snapshot-Broadcast -> Layer/Panel recovery -> Backend-Hotplug).
  - H1-H4 Audit bestätigt die operative Reihenfolge im zentralen State-Pfad:
    1) Registry-Änderung -> 2) WorkspaceOutputState Sync/Fallback -> 3) Layer-Shell-Recovery -> 4) OutputWorkspaceSnapshot Broadcast.
  - H5-Hookpunkte:
    - Winit: `backend/winit/mod.rs` (`WinitEvent::Resized`).
    - DRM: `backend/drm/init.rs` (DRM-Notifier/Connector-Lifecycle, derzeit primär VBlank-Pfad).
