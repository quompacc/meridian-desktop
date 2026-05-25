# Architecture

Stand: 2026-05-25, auditiert gegen `master` bei `2e7a2ed`.

## Workspace-Struktur
- `src/main.rs`: Startpunkt, Backend-Wahl, XWayland, IPC-Timer und
  Shell-Watchdog.
- `crates/meridian-compositor`: Wayland-Compositor, Backends, State,
  Rendering, Input, Output-/Workspace-Policy.
- `crates/meridian-shell`: separater Layer-Shell-Client fuer Panel,
  Launcher, Popups, Settings, Notifications und Screenshots.
- `crates/meridian-login`: root-seitiger DRM-Login mit PAM/logind,
  YubiKey/PIN-Flow, Passwort-Fallback und Compositor-Handover.
- `crates/meridian-config`: TOML-Konfiguration, Themes, Wallpaper,
  Keybinds, Outputs und Panel-Pinned-Apps.
- `crates/meridian-ipc`: JSON-Line IPC fuer Shell/Compositor plus
  gemeinsame Screenshot-Bridge-Typen.
- `crates/meridian-portal`: D-Bus Portal-Backend; aktuell FileChooser
  delegiert an einen externen Picker.
- `crates/meridian-wm`: Tiling/Floating-Workspace-Logik.
- `crates/meridian-ui`: gemeinsam nutzbare UI-Primitives.
- `crates/meridian-compass-render`: gemeinsamer Compass-Renderer fuer
  Bootsplash/Login.
- `crates/meridian-boot-common`: Boot-Chain-Helfer fuer Socket-Cleanup,
  sichere Socket-Rechte und Boot-Mode-Auswahl.

## Modulstruktur
- `meridian-compositor/backend/drm/*`: DRM-Init, GPU/Mode-Auswahl, Hotplug,
  Timing-Diagnostik und Render-Pipeline.
- `meridian-compositor/backend/winit/*`: Entwicklungsbackend, Winit-Output
  und Scene-Komposition.
- `meridian-compositor/state/*`: Smithay-State, Setup, Layout, IPC,
  Handler, OutputRegistry, WorkspaceOutputState, Lock/Idle/Output-Power.
- `meridian-compositor/input/*`: Keyboard-/Pointer-Verarbeitung,
  Keybind-Ausfuehrung und Output-Fokuspflege.
- `meridian-compositor/decoration/*`: Server-Side Decorations, Icons,
  Shadow-/Icon-Caches und Hit-Testing.
- `meridian-compositor/wallpaper/*`: Wallpaper-Compose und GPU-Cache.
- `meridian-shell/wayland/*`: Client-Init, State, Render, IPC,
  Screencopy und Wayland-Handler.
- `meridian-shell/icons/*`: Icon-Theme, SVG/RCC-Loader und Cache.
- `meridian-shell/draw/*`: tiny-skia Painter, Text, FreeType/fontconfig.
- `meridian-shell/*_view.rs`, `*_popup.rs`: App-Grid, Settings, Panel,
  Power-Footer, Network/Notification/Thumbnail-Popups.
- `meridian-login/src/*`: Auth, Input, Session-Spawn und DRM/Login-UI.
- `meridian-portal/src/*`: D-Bus Service und FileChooser-Implementierung.

## Startpfade

### Desktop
1. `src/main.rs`
2. `MeridianState::new`
3. `backend::drm::init_drm` ohne Parent-Display, sonst `init_winit`
4. XWayland-Start
5. IPC-Poll-Timer
6. `meridian-shell` Watchdog, ausser `MERIDIAN_DRM_DISABLE_SHELL=1` oder
   `MERIDIAN_NO_SHELL=1`

### Login
1. `meridian-login`
2. Bootsplash-Handover ueber `/run/bootsplash.sock`
3. PAM/logind-Session nach erfolgreichem Smartcard- oder Passwort-Login
4. Compositor-Spawn als User
5. Handover ueber `/run/meridian-login.sock`

## Compositor / Shell / IPC
- `meridian-compositor` verwaltet Surfaces, Focus, Workspaces, Outputs,
  Rendering und Shell-Kommandos.
- `meridian-shell` rendert Panel/Launcher/Popups als Layer-Surfaces.
- IPC-Kommandos Shell -> Compositor:
  - `SwitchWorkspace`
  - `FocusWindow`
  - `LaunchApp`
  - `ReloadConfig`
  - `Quit`
  - `CaptureWindowThumbnail`
- IPC-Events Compositor -> Shell:
  - legacy Workspace- und Window-Events
  - `WindowSnapshot`
  - output-aware Workspace-Events/Snapshots
  - `ConfigReloaded`
  - `ToggleLauncher`
  - `WindowThumbnail`
- Screenshot-Bridge-Messages sind im IPC-Typensystem vorhanden, werden
  compositorseitig aber deny-only behandelt.

## Shell-Oberflaeche
- Panel: Launcher, Workspaces, pinned Apps, Network, Screenshot, Clock.
- Launcher: App-Grid, Kategorien, Suche, Kontextmenues, versteckte Apps,
  Pinned-App-Management.
- Settings: Theme, Cursor-Kategorie, Wallpaper-Auswahl/Picker/Modus,
  Pinned-Apps. Display/Keyboard/Audio sind noch offen.
- Popups: Calendar, Workspace, Network, Notifications, Window-Thumbnails.
- Power-Footer: Poweroff/Reboot/Suspend/Lock via Systemtools, Logout via
  Compositor-IPC.

## Backends
- `drm`: KMS/GBM/GLES, Hauptpfad fuer echte Sessions.
- `winit`: Fallback/Embedded-Session, gleiche State- und Renderprinzipien.

## Wayland-Protokolle
- XDG Shell (+ Popups/Toplevel)
- WLR Layer Shell
- XDG Decoration
- SHM
- Output/XDG-Output
- Data Device / DnD
- XWayland Shell
- linux-dmabuf
- linux-drm-syncobj
- ext-image-copy-capture / ext-image-capture-source
- session-lock
- output-power-management
- idle-inhibit / idle-notify

## Render-Layer-Reihenfolge
Die visuelle Stapelung ist Korrektheit, nicht Stilfrage:
1. background / wallpaper
2. bottom layer surfaces
3. normal application windows
4. top layer surfaces / panel
5. overlay surfaces / launcher / popups
6. cursor

## Keybinding-Orte
- Parsing/Defaults: `crates/meridian-config/src/keybind/*`
- Ausfuehrung: `crates/meridian-compositor/src/input/keyboard.rs`
- Runtime-Reload: `crates/meridian-compositor/src/state/ipc/commands.rs`

## Performance-sensitive Bereiche
- `backend/drm/render/*`
- `backend/winit/*`
- `decoration/render/*`
- `wallpaper/*`
- `state/handlers/*`
- `meridian-shell/wayland/render.rs`
- Shell-Timer und Popups, besonders Notifications/Thumbnails/Screenshots

## XDG-Portals
- `meridian-portal` ist ein separater Prozess.
- Aktuell implementiert: FileChooser-Backend unter
  `org.freedesktop.impl.portal.desktop.meridian`.
- FileChooser delegiert an `MERIDIAN_FILE_PICKER` oder
  `/usr/local/bin/meridian-file-picker`.
- Screenshot/ScreenCast/Settings/OpenURI bleiben offene Portal-Slices.
- Referenz: `docs/XDG_PORTALS.md`.

## Multi-Monitor
- Output-Metadaten werden in `OutputRegistry` gespiegelt.
- Workspace-Zielmodell ist Hybrid:
  `focused_output` plus `active_workspace_by_output`, mit globalem
  `WorkspaceManager.active` als Kompatibilitaets-Shadow.
- Active-Workspace im Panel ist output-aware; Occupied bleibt global.
- Hotplug-Pipeline ist in Code vorbereitet/aktiv bis DRM Add/Remove:
  Registry -> Workspace-State-Sync/Fallback -> Layer-Shell-Recovery ->
  OutputWorkspaceSnapshot Broadcast.
- Runtime-Hotplug braucht weiter reale E2E-Validierung.
